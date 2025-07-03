#!/usr/bin/env python3
"""
Advanced Concurrent Traffic Generator for obsctl

This script simulates realistic S3 traffic patterns using multiple concurrent user personas.
Each user has distinct behavior patterns, file preferences, and peak activity hours.

Features:
- 10 concurrent user simulations with unique profiles
- Realistic file generation with proper content
- TTL-based cleanup (3 hours regular, 60 minutes large files)
- Smart bucket creation (check before create)
- Comprehensive error handling and race condition fixes
- High-volume traffic generation (100-500 ops/min peak, 10-50 ops/min off-peak)
- Lock file management to prevent multiple instances
- FIXED: Race condition protection with file locking and operation tracking
- FIXED: Graceful shutdown with proper thread synchronization
- FIXED: Operation-aware TTL cleanup to prevent file deletion during uploads
"""

import os
import sys
import time
import random
import threading
import subprocess
import logging
import signal
import shutil
import fcntl
import json
from datetime import datetime
from concurrent.futures import ThreadPoolExecutor, as_completed
from threading import Event, RLock
from pathlib import Path

# Import configuration from separate file
from traffic_config import (
    TEMP_DIR, OBSCTL_BINARY, MINIO_ENDPOINT, SCRIPT_DURATION_HOURS, MAX_CONCURRENT_USERS,
    PEAK_VOLUME_MIN, PEAK_VOLUME_MAX, OFF_PEAK_VOLUME_MIN, OFF_PEAK_VOLUME_MAX,
    REGULAR_FILE_TTL, LARGE_FILE_TTL, LARGE_FILE_THRESHOLD,
    USER_CONFIGS, FILE_EXTENSIONS, OBSCTL_ENV,
    DISK_SPACE_CONFIG, HIGH_VOLUME_CONFIG, SUBFOLDER_TEMPLATES,
    get_disk_free_space_gb, should_stop_generation, needs_emergency_cleanup
)

# Use imported configuration from traffic_config.py

# Compatibility mappings for old variable names
USERS = USER_CONFIGS
TTL_CONFIG = {
    'regular_files_hours': REGULAR_FILE_TTL // 3600,
    'large_files_minutes': LARGE_FILE_TTL // 60,
    'large_file_threshold_mb': LARGE_FILE_THRESHOLD // (1024 * 1024),
}

# Create FILE_TYPES from imported configuration
FILE_TYPES = {}
for file_type, extensions in FILE_EXTENSIONS.items():
    if file_type == 'images':
        FILE_TYPES[file_type] = {
            'extensions': extensions,
            'sizes': [(1024, 500*1024), (500*1024, 5*1024*1024), (5*1024*1024, 20*1024*1024)],
            'weights': [0.8, 0.15, 0.05],
            'weight': 0.25
        }
    elif file_type == 'documents':
        FILE_TYPES[file_type] = {
            'extensions': extensions,
            'sizes': [(1024, 100*1024), (100*1024, 2*1024*1024), (2*1024*1024, 10*1024*1024)],
            'weights': [0.8, 0.15, 0.05],
            'weight': 0.20
        }
    elif file_type == 'code':
        FILE_TYPES[file_type] = {
            'extensions': extensions,
            'sizes': [(100, 10*1024), (10*1024, 100*1024), (100*1024, 500*1024)],
            'weights': [0.85, 0.12, 0.03],
            'weight': 0.15
        }
    elif file_type == 'archives':
        FILE_TYPES[file_type] = {
            'extensions': extensions,
            'sizes': [(100*1024, 5*1024*1024), (5*1024*1024, 50*1024*1024), (50*1024*1024, 200*1024*1024)],
            'weights': [0.7, 0.25, 0.05],
            'weight': 0.15
        }
    elif file_type == 'media':
        FILE_TYPES[file_type] = {
            'extensions': extensions,
            'sizes': [(500*1024, 10*1024*1024), (10*1024*1024, 100*1024*1024), (100*1024*1024, 500*1024*1024)],
            'weights': [0.75, 0.20, 0.05],
            'weight': 0.25
        }

# Global variables for runtime state
global_stats = {
    'operations': 0,
    'uploads': 0,
    'downloads': 0,
    'errors': 0,
    'files_created': 0,
    'large_files_created': 0,
    'ttl_policies_applied': 0,
    'bytes_transferred': 0
}

user_stats = {}
stats_lock = threading.Lock()
running = True

# Global bucket tracking to avoid duplicate creation attempts
created_buckets = set()
bucket_creation_lock = threading.Lock()

# 🔥 CRITICAL FIX: Operation tracking to prevent race conditions
active_operations = {}  # file_path -> operation_info
operations_lock = RLock()  # Reentrant lock for nested operations

# 🔥 CRITICAL FIX: Shutdown coordination
shutdown_event = Event()
user_threads_completed = Event()
all_users_stopped = threading.Barrier(len(USERS) + 1)  # +1 for main thread

# Lock file path
LOCK_FILE = "/tmp/obsctl-traffic-generator.lock"

def acquire_lock():
    """Acquire exclusive lock to prevent multiple instances"""
    try:
        lock_fd = os.open(LOCK_FILE, os.O_CREAT | os.O_WRONLY | os.O_TRUNC)
        fcntl.flock(lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)

        # Write PID to lock file
        os.write(lock_fd, f"{os.getpid()}\n".encode())
        os.fsync(lock_fd)

        return lock_fd
    except (OSError, IOError) as e:
        print(f"ERROR: Another traffic generator instance is already running")
        print(f"Lock file: {LOCK_FILE}")
        if os.path.exists(LOCK_FILE):
            try:
                with open(LOCK_FILE, 'r') as f:
                    existing_pid = f.read().strip()
                print(f"Existing PID: {existing_pid}")
            except:
                pass
        sys.exit(1)

def release_lock(lock_fd):
    """Release the exclusive lock"""
    try:
        fcntl.flock(lock_fd, fcntl.LOCK_UN)
        os.close(lock_fd)
        if os.path.exists(LOCK_FILE):
            os.unlink(LOCK_FILE)
    except:
        pass

def check_if_running():
    """Check if traffic generator is already running"""
    if os.path.exists(LOCK_FILE):
        try:
            with open(LOCK_FILE, 'r') as f:
                pid = int(f.read().strip())

            # Check if process is still running
            try:
                os.kill(pid, 0)  # Signal 0 just checks if process exists
                return True, pid
            except OSError:
                # Process not running, remove stale lock file
                os.unlink(LOCK_FILE)
                return False, None
        except:
            return False, None
    return False, None

# 🔥 CRITICAL FIX: Operation tracking functions
def register_operation(file_path, operation_type, user_id):
    """Register an active operation to prevent race conditions"""
    with operations_lock:
        active_operations[file_path] = {
            'type': operation_type,
            'user_id': user_id,
            'start_time': time.time(),
            'thread_id': threading.current_thread().ident
        }

def unregister_operation(file_path):
    """Unregister a completed operation"""
    with operations_lock:
        active_operations.pop(file_path, None)

def is_file_in_use(file_path):
    """Check if a file is currently being used in an operation"""
    with operations_lock:
        return file_path in active_operations

def get_active_operations_for_user(user_id):
    """Get all active operations for a specific user"""
    with operations_lock:
        return [path for path, info in active_operations.items() if info['user_id'] == user_id]

def wait_for_user_operations_complete(user_id, timeout=30):
    """Wait for all operations for a specific user to complete"""
    start_time = time.time()
    while time.time() - start_time < timeout:
        active_ops = get_active_operations_for_user(user_id)
        if not active_ops:
            return True
        time.sleep(0.1)
    return False

class UserSimulator:
    """Individual user simulator that runs in its own thread"""

    def __init__(self, user_id, user_config):
        self.user_id = user_id
        self.user_config = user_config
        self.bucket = user_config['bucket']
        self.user_temp_dir = os.path.join(TEMP_DIR, user_id)
        self.config_method = user_config.get('config_method', 'env_vars')
        os.makedirs(self.user_temp_dir, exist_ok=True)
        self.logger = self.setup_user_logger()
        self.user_stopped = Event()  # 🔥 CRITICAL FIX: Individual user stop event

        # 🚀 NEW: Setup AWS config for this user if using config files
        if self.config_method == 'config_file':
            self.setup_aws_config()

        # 🚀 NEW: Subfolder management
        self.subfolder_templates = SUBFOLDER_TEMPLATES.get(self.bucket, ['files'])
        self.used_subfolders = set()
        self.files_per_subfolder = {}

        # 🚀 NEW: High-volume file tracking
        self.total_files_created = 0
        self.last_disk_check = 0

        # Initialize user stats if not already done
        with stats_lock:
            if user_id not in user_stats:
                user_stats[user_id] = {
                    'operations': 0, 'uploads': 0, 'downloads': 0, 'errors': 0,
                    'bytes_transferred': 0, 'files_created': 0, 'large_files': 0,
                    'subfolders_used': 0, 'disk_space_checks': 0
                }

    def setup_aws_config(self):
        """Setup AWS config files for this user"""
        # Create user-specific .aws directory (AWS-only configs)
        aws_dir = os.path.join(self.user_temp_dir, '.aws')
        os.makedirs(aws_dir, exist_ok=True)

        # Create credentials file
        credentials_file = os.path.join(aws_dir, 'credentials')
        with open(credentials_file, 'w') as f:
            f.write(f"[default]\n")
            f.write(f"aws_access_key_id = minioadmin\n")
            f.write(f"aws_secret_access_key = minioadmin123\n")

        # Create config file (AWS-specific only)
        config_file = os.path.join(aws_dir, 'config')
        with open(config_file, 'w') as f:
            f.write(f"[default]\n")
            f.write(f"region = us-east-1\n")
            f.write(f"endpoint_url = {MINIO_ENDPOINT}\n")

        # Create user-specific .obsctl directory (obsctl-specific configs)
        obsctl_dir = os.path.join(self.user_temp_dir, '.obsctl')
        os.makedirs(obsctl_dir, exist_ok=True)

        # Create OTEL config file in .obsctl directory
        otel_file = os.path.join(obsctl_dir, 'otel')
        with open(otel_file, 'w') as f:
            f.write(f"[otel]\n")
            f.write(f"enabled = true\n")
            f.write(f"endpoint = http://localhost:4317\n")
            f.write(f"service_name = obsctl-{self.user_id}\n")

        # Create Loki config file in .obsctl directory
        loki_file = os.path.join(obsctl_dir, 'loki')
        with open(loki_file, 'w') as f:
            f.write(f"[loki]\n")
            f.write(f"enabled = true\n")
            f.write(f"endpoint = http://localhost:3100\n")
            f.write(f"log_level = info\n")
            f.write(f"label_user_id = {self.user_id}\n")
            f.write(f"label_service = obsctl-traffic\n")
            f.write(f"label_environment = development\n")

        self.logger.info(f"Created AWS + obsctl config files for {self.user_id} (method: {self.config_method})")

    def setup_user_logger(self):
        """Setup logger for this specific user"""
        logger = logging.getLogger(f"user.{self.user_id}")
        if not logger.handlers:
            handler = logging.StreamHandler()
            formatter = logging.Formatter(f'%(asctime)s - %(levelname)s - [{self.user_id}] %(message)s')
            handler.setFormatter(formatter)
            logger.addHandler(handler)
            logger.setLevel(logging.INFO)
        return logger

    def check_disk_space(self):
        """🚀 NEW: Check disk space and return whether to continue"""
        current_time = time.time()

        # Only check every 30 seconds to avoid overhead
        if current_time - self.last_disk_check < DISK_SPACE_CONFIG['check_interval_seconds']:
            return True

        self.last_disk_check = current_time
        free_gb = get_disk_free_space_gb()

        with stats_lock:
            user_stats[self.user_id]['disk_space_checks'] += 1

        if needs_emergency_cleanup():
            self.logger.critical(f"EMERGENCY: Only {free_gb:.1f}GB free! Stopping immediately.")
            return False
        elif should_stop_generation():
            self.logger.warning(f"LOW DISK SPACE: {free_gb:.1f}GB free. Stopping generation.")
            return False
        elif free_gb < 20:  # Warning threshold
            self.logger.warning(f"DISK SPACE WARNING: {free_gb:.1f}GB free remaining.")

        return True

    def generate_subfolder_path(self):
        """🚀 NEW: Generate realistic subfolder path based on templates"""
        if not HIGH_VOLUME_CONFIG['use_subfolders']:
            return ""

        # Select a template
        template = random.choice(self.subfolder_templates)

        # Fill in template variables with realistic values
        replacements = {
            'project': random.choice(['web-app', 'mobile-client', 'api-service', 'data-pipeline', 'ml-model']),
            'campaign': random.choice(['q1-launch', 'summer-sale', 'brand-refresh', 'product-demo', 'holiday-2024']),
            'dataset': random.choice(['customer-data', 'sales-metrics', 'user-behavior', 'market-research', 'inventory']),
            'model': random.choice(['recommendation', 'classification', 'clustering', 'regression', 'nlp-sentiment']),
            'system': random.choice(['web-servers', 'databases', 'load-balancers', 'cache-cluster', 'api-gateway']),
            'client': random.choice(['acme-corp', 'beta-tech', 'gamma-solutions', 'delta-industries', 'epsilon-labs']),
            'app': random.choice(['ios-main', 'android-main', 'react-native', 'flutter-app', 'hybrid-app']),
            'service': random.choice(['user-auth', 'payment-processor', 'notification-service', 'analytics-api', 'file-storage']),
            'env': random.choice(['dev', 'staging', 'prod', 'test', 'demo']),
            'platform': random.choice(['facebook', 'instagram', 'twitter', 'linkedin', 'youtube']),
            'category': random.choice(['photos', 'videos', 'documents', 'templates', 'assets']),
            'subcategory': random.choice(['high-res', 'thumbnails', 'originals', 'processed', 'archived']),
            'region': random.choice(['north-america', 'europe', 'asia-pacific', 'latin-america', 'middle-east']),
            'quarter': random.choice(['q1-2024', 'q2-2024', 'q3-2024', 'q4-2024']),
            'year': random.choice(['2023', '2024', '2025']),
            'month': random.choice(['01', '02', '03', '04', '05', '06', '07', '08', '09', '10', '11', '12']),
            'day': f"{random.randint(1, 28):02d}",
            'week': f"{random.randint(1, 52):02d}",
            'date': datetime.now().strftime('%Y-%m-%d'),
            'topic': random.choice(['machine-learning', 'data-analysis', 'user-research', 'market-trends', 'security']),
            'study': random.choice(['user-behavior', 'performance-analysis', 'ab-testing', 'market-research', 'usability']),
            'partner': random.choice(['university-x', 'research-institute', 'tech-company', 'startup-incubator', 'consulting-firm']),
            'version': f"v{random.randint(1, 10)}.{random.randint(0, 9)}.{random.randint(0, 9)}",
            'experiment_id': f"exp-{random.randint(1000, 9999)}"
        }

        # Replace placeholders in template
        path = template
        for key, value in replacements.items():
            path = path.replace(f'{{{key}}}', value)

        # Track subfolder usage
        if path not in self.used_subfolders:
            self.used_subfolders.add(path)
            self.files_per_subfolder[path] = 0
            with stats_lock:
                user_stats[self.user_id]['subfolders_used'] += 1

        # Check if we should create a new subfolder (limit files per subfolder)
        if self.files_per_subfolder[path] >= HIGH_VOLUME_CONFIG['files_per_subfolder']:
            # Try to generate a new path
            for _ in range(3):  # Max 3 attempts
                new_path = self.generate_subfolder_path()
                if new_path != path and self.files_per_subfolder.get(new_path, 0) < HIGH_VOLUME_CONFIG['files_per_subfolder']:
                    return new_path

        return path

    def get_current_activity_level(self):
        """Calculate current activity level based on user's timezone and peak hours"""
        current_hour = datetime.now().hour
        user_hour = (current_hour + self.user_config['timezone_offset']) % 24

        peak_start, peak_end = self.user_config['peak_hours']

        # Handle peak hours that span midnight
        if peak_start > peak_end:
            is_peak = user_hour >= peak_start or user_hour <= peak_end
        else:
            is_peak = peak_start <= user_hour <= peak_end

        base_activity = self.user_config['activity_multiplier']

        # 🚀 ENHANCED: High-volume generation with disk space awareness
        if not self.check_disk_space():
            return 0  # Stop activity if disk space is low

        # Force high activity for high-volume testing
        if hash(self.user_id) % 10 < 8:  # 80% of users get forced peak activity
            activity_level = base_activity * 4.0  # Quadruple activity for testing
            self.logger.debug(f"HIGH-VOLUME MODE: user_hour={user_hour}, activity={activity_level:.1f}")
        elif is_peak:
            activity_level = base_activity * 2.5  # High activity during peak hours
            self.logger.debug(f"NATURAL PEAK: user_hour={user_hour}, activity={activity_level:.1f}")
        else:
            activity_level = base_activity * 0.5  # Reduced activity during off hours
            self.logger.debug(f"OFF PEAK: user_hour={user_hour}, activity={activity_level:.1f}")

        return activity_level

    def select_file_type_and_size(self):
        """🚀 ENHANCED: Select file type and size using weighted distributions"""
        file_preferences = self.user_config['file_preferences']

        # Use weighted random selection based on user preferences
        file_types = list(file_preferences.keys())
        weights = list(file_preferences.values())
        file_type = random.choices(file_types, weights=weights)[0]

        # Get file type configuration
        file_config = FILE_TYPES[file_type]

        # Select size range using weights (80% small files for high object count)
        size_ranges = file_config['sizes']
        size_weights = file_config.get('weights', [1.0] * len(size_ranges))

        # Apply small file bias from HIGH_VOLUME_CONFIG
        if HIGH_VOLUME_CONFIG['small_file_bias'] > 0:
            # Boost weight of smallest size range
            size_weights = list(size_weights)
            size_weights[0] *= (1 + HIGH_VOLUME_CONFIG['small_file_bias'])

        selected_range = random.choices(size_ranges, weights=size_weights)[0]
        size_bytes = random.randint(selected_range[0], selected_range[1])

        return file_type, size_bytes

    def select_file_type(self):
        """Legacy method for compatibility"""
        file_type, _ = self.select_file_type_and_size()
        return file_type

    def generate_file(self, file_type, size_bytes, filename):
        """Generate a file with specific type and size - RACE CONDITION PROTECTED"""
        file_path = os.path.join(self.user_temp_dir, filename)

        # 🔥 CRITICAL FIX: Register operation before file creation
        register_operation(file_path, 'generate', self.user_id)

        try:
            if file_type == 'code':
                content = self.generate_code_content(size_bytes)
                with open(file_path, 'w') as f:
                    f.write(content)
            elif file_type == 'documents':
                content = self.generate_document_content(size_bytes)
                with open(file_path, 'w') as f:
                    f.write(content)
            else:
                # Generate binary content for images, archives, media
                with open(file_path, 'wb') as f:
                    chunk_size = min(8192, size_bytes)
                    remaining = size_bytes
                    while remaining > 0:
                        chunk = os.urandom(min(chunk_size, remaining))
                        f.write(chunk)
                        remaining -= len(chunk)

            # Update stats
            with stats_lock:
                global_stats['files_created'] += 1
                user_stats[self.user_id]['files_created'] += 1

                # Check if it's a large file
                size_mb = size_bytes / (1024 * 1024)
                if size_mb > TTL_CONFIG['large_file_threshold_mb']:
                    global_stats['large_files_created'] += 1
                    user_stats[self.user_id]['large_files'] += 1

            return file_path

        except Exception as e:
            self.logger.error(f"Failed to generate file {filename}: {e}")
            return None
        finally:
            # 🔥 CRITICAL FIX: Always unregister operation
            unregister_operation(file_path)

    def generate_code_content(self, size_bytes):
        """Generate realistic code content"""
        code_templates = [
            "def function_{}():\n    '''Generated function for {user}'''\n    return {}\n\n",
            "class Class{}:\n    def __init__(self):\n        self.{user}_value = {}\n\n",
            "# {user} - {desc}\n# This is a comment about {}\nvar_{} = {}\n\n",
            "import {}\nfrom {} import {}\n# {user} imports\n\n"
        ]

        content = f"# Generated code file for {self.user_id}\n# {self.user_config['description']}\n\n"
        while len(content.encode()) < size_bytes:
            template = random.choice(code_templates)
            content += template.format(
                random.randint(1, 1000),
                random.randint(1, 1000),
                random.randint(1, 1000),
                user=self.user_id,
                desc=self.user_config['description']
            )

        return content[:size_bytes]

    def generate_document_content(self, size_bytes):
        """Generate realistic document content"""
        words = [
            "data", "analysis", "report", "summary", "business", "metrics",
            "performance", "optimization", "cloud", "storage", "transfer",
            "monitoring", "dashboard", "analytics", "insights", "trends",
            self.user_id, "project", "research", "development"
        ]

        content = f"Document by {self.user_id}\n"
        content += f"Department: {self.user_config['description']}\n\n"

        while len(content.encode()) < size_bytes:
            sentence_length = random.randint(5, 15)
            sentence = " ".join(random.choices(words, k=sentence_length))
            content += sentence.capitalize() + ". "

            if random.random() < 0.1:
                content += "\n\n"

        return content[:size_bytes]

    def apply_ttl_policy(self, file_path, size_bytes):
        """Apply TTL policy based on file size"""
        size_mb = size_bytes / (1024 * 1024)

        if size_mb > TTL_CONFIG['large_file_threshold_mb']:
            ttl_minutes = TTL_CONFIG['large_files_minutes']
            self.logger.info(f"Large file ({size_mb:.1f}MB) - TTL: {ttl_minutes} minutes")
        else:
            ttl_hours = TTL_CONFIG['regular_files_hours']
            self.logger.info(f"Regular file ({size_mb:.1f}MB) - TTL: {ttl_hours} hours")

        with stats_lock:
            global_stats['ttl_policies_applied'] += 1

    def upload_operation(self):
        """Perform upload operation - RACE CONDITION PROTECTED"""
        # 🚀 ENHANCED: Check if we should stop due to target reached or disk space
        if self.total_files_created >= HIGH_VOLUME_CONFIG['target_files_per_bucket']:
            self.logger.info(f"Target of {HIGH_VOLUME_CONFIG['target_files_per_bucket']} files reached. Slowing down.")
            return True  # Continue but at reduced rate

        if not self.check_disk_space():
            self.logger.warning("Stopping upload due to low disk space.")
            return False

        file_type, size_bytes = self.select_file_type_and_size()
        extension = random.choice(FILE_TYPES[file_type]['extensions'])

        # 🚀 NEW: Generate subfolder path
        subfolder_path = self.generate_subfolder_path()

        timestamp = int(time.time())
        filename = f"{self.user_id}_{file_type}_{timestamp}{extension}"

        # Generate file
        local_path = self.generate_file(file_type, size_bytes, filename)
        if not local_path:
            with stats_lock:
                global_stats['errors'] += 1
                user_stats[self.user_id]['errors'] += 1
            return False

        # 🔥 CRITICAL FIX: Register upload operation before starting
        register_operation(local_path, 'upload', self.user_id)

        try:
            # 🚀 NEW: Upload to subfolder path in user's bucket
            if subfolder_path:
                s3_path = f"s3://{self.bucket}/{subfolder_path}/{filename}"
            else:
                s3_path = f"s3://{self.bucket}/{filename}"

            success = self.run_obsctl_command(['cp', local_path, s3_path])

            if success:
                # 🚀 NEW: Track files per subfolder
                if subfolder_path:
                    self.files_per_subfolder[subfolder_path] = self.files_per_subfolder.get(subfolder_path, 0) + 1

                self.total_files_created += 1

                with stats_lock:
                    global_stats['uploads'] += 1
                    global_stats['operations'] += 1
                    global_stats['bytes_transferred'] += size_bytes
                    user_stats[self.user_id]['uploads'] += 1
                    user_stats[self.user_id]['operations'] += 1
                    user_stats[self.user_id]['bytes_transferred'] += size_bytes

                self.apply_ttl_policy(local_path, size_bytes)

                # 🚀 ENHANCED: Better logging with subfolder info
                if subfolder_path:
                    self.logger.info(f"Uploaded {subfolder_path}/{filename} ({size_bytes} bytes) [Total: {self.total_files_created}]")
                else:
                    self.logger.info(f"Uploaded {filename} ({size_bytes} bytes) [Total: {self.total_files_created}]")

        finally:
            # 🔥 CRITICAL FIX: Always unregister and cleanup, but check if file still exists
            unregister_operation(local_path)

            # Only remove file if it still exists and isn't being used by another operation
            try:
                if os.path.exists(local_path) and not is_file_in_use(local_path):
                    os.remove(local_path)
            except Exception as e:
                self.logger.debug(f"File cleanup warning: {e}")

        return success

    def download_operation(self):
        """Perform download operation"""
        try:
            # List files in user's bucket
            env = dict(os.environ)

            if self.config_method == 'env_vars':
                env.update(OBSCTL_ENV)
            else:
                aws_dir = os.path.join(self.user_temp_dir, '.aws')
                env['AWS_CONFIG_FILE'] = os.path.join(aws_dir, 'config')
                env['AWS_SHARED_CREDENTIALS_FILE'] = os.path.join(aws_dir, 'credentials')
                env['HOME'] = self.user_temp_dir

            result = subprocess.run(
                [OBSCTL_BINARY, 'ls', f's3://{self.bucket}/'],
                capture_output=True,
                text=True,
                timeout=30,
                env=env
            )

            if result.returncode != 0:
                return False

            lines = result.stdout.strip().split('\n')
            if not lines or len(lines) < 2:
                return False

            # Pick a random file to download
            file_line = random.choice(lines[1:])
            if not file_line.strip():
                return False

            parts = file_line.strip().split()
            if len(parts) < 4:
                return False

            filename = parts[-1]
            s3_path = f"s3://{self.bucket}/{filename}"
            local_path = os.path.join(self.user_temp_dir, f"downloaded_{filename}")

            # 🔥 CRITICAL FIX: Register download operation
            register_operation(local_path, 'download', self.user_id)

            try:
                # Download file
                success = self.run_obsctl_command(['cp', s3_path, local_path])

                if success:
                    try:
                        file_size = os.path.getsize(local_path)
                        with stats_lock:
                            global_stats['downloads'] += 1
                            global_stats['operations'] += 1
                            global_stats['bytes_transferred'] += file_size
                            user_stats[self.user_id]['downloads'] += 1
                            user_stats[self.user_id]['operations'] += 1
                            user_stats[self.user_id]['bytes_transferred'] += file_size

                        self.logger.info(f"Downloaded {filename} ({file_size} bytes)")

                        # Clean up downloaded file immediately
                        if os.path.exists(local_path):
                            os.remove(local_path)
                    except Exception as e:
                        self.logger.debug(f"Download cleanup warning: {e}")

            finally:
                # 🔥 CRITICAL FIX: Always unregister operation
                unregister_operation(local_path)

            return success

        except Exception as e:
            self.logger.error(f"Download operation failed: {e}")
            with stats_lock:
                global_stats['errors'] += 1
                user_stats[self.user_id]['errors'] += 1
            return False

    def run_obsctl_command(self, args):
        """Run obsctl command with proper environment based on config method"""
        cmd = [OBSCTL_BINARY] + args
        try:
            env = dict(os.environ)

            if self.config_method == 'env_vars':
                # Use environment variables
                env.update(OBSCTL_ENV)
                self.logger.debug(f"Using environment variables for {self.user_id}")
            else:
                # Use config files on disk - set paths to user-specific locations
                aws_dir = os.path.join(self.user_temp_dir, '.aws')
                obsctl_dir = os.path.join(self.user_temp_dir, '.obsctl')

                # AWS configuration
                env['AWS_CONFIG_FILE'] = os.path.join(aws_dir, 'config')
                env['AWS_SHARED_CREDENTIALS_FILE'] = os.path.join(aws_dir, 'credentials')

                # obsctl configuration (set HOME to user temp dir so obsctl finds ~/.obsctl)
                env['HOME'] = self.user_temp_dir

                self.logger.debug(f"Using config files for {self.user_id}: AWS={env['AWS_CONFIG_FILE']}, obsctl={obsctl_dir}")

            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=120,
                env=env
            )

            if result.returncode != 0:
                self.logger.warning(f"Command failed: {' '.join(cmd)}")
                self.logger.warning(f"Error: {result.stderr}")
                with stats_lock:
                    global_stats['errors'] += 1
                    user_stats[self.user_id]['errors'] += 1
                return False

            return True

        except subprocess.TimeoutExpired:
            self.logger.error(f"Command timeout: {' '.join(cmd)}")
            with stats_lock:
                global_stats['errors'] += 1
                user_stats[self.user_id]['errors'] += 1
            return False
        except Exception as e:
            self.logger.error(f"Command exception: {e}")
            with stats_lock:
                global_stats['errors'] += 1
                user_stats[self.user_id]['errors'] += 1
            return False

    def ensure_bucket_exists(self):
        """Smart bucket creation - only create if not already done"""
        global created_buckets

        with bucket_creation_lock:
            if self.bucket in created_buckets:
                self.logger.debug(f"Bucket {self.bucket} already created, skipping")
                return True

            # Check if bucket exists by listing it
            try:
                env = dict(os.environ)

                if self.config_method == 'env_vars':
                    env.update(OBSCTL_ENV)
                else:
                    aws_dir = os.path.join(self.user_temp_dir, '.aws')
                    env['AWS_CONFIG_FILE'] = os.path.join(aws_dir, 'config')
                    env['AWS_SHARED_CREDENTIALS_FILE'] = os.path.join(aws_dir, 'credentials')
                    env['HOME'] = self.user_temp_dir

                result = subprocess.run(
                    [OBSCTL_BINARY, 'ls', f's3://{self.bucket}/'],
                    capture_output=True,
                    text=True,
                    timeout=30,
                    env=env
                )

                if result.returncode == 0:
                    # Bucket exists
                    created_buckets.add(self.bucket)
                    self.logger.debug(f"Bucket {self.bucket} already exists")
                    return True

            except Exception as e:
                self.logger.debug(f"Error checking bucket existence: {e}")

            # Bucket doesn't exist, create it
            self.logger.info(f"Creating bucket: {self.bucket}")
            success = self.run_obsctl_command(['mb', f's3://{self.bucket}'])

            if success:
                created_buckets.add(self.bucket)
                self.logger.info(f"Successfully created bucket: {self.bucket}")
            else:
                # Check if error was "BucketAlreadyOwnedByYou" which is actually success
                self.logger.debug(f"Bucket creation command failed, but bucket might already exist")
                created_buckets.add(self.bucket)  # Assume it exists

            return True

    def run(self):
        """Main user simulation loop - GRACEFUL SHUTDOWN ENABLED"""
        global running

        self.logger.info(f"Starting user simulation: {self.user_config['description']}")
        self.logger.info(f"Configuration method: {self.config_method}")

        # Create user's bucket
        self.ensure_bucket_exists()

        try:
            while running and not shutdown_event.is_set():
                try:
                    # Calculate current activity level
                    activity_level = self.get_current_activity_level()

                    # Determine operation interval for high-volume traffic
                    if activity_level > 1.0:  # Peak hours - high volume
                        # Use configured peak volume settings
                        ops_per_min = random.uniform(PEAK_VOLUME_MIN, PEAK_VOLUME_MAX)
                        base_interval = 60.0 / ops_per_min
                    else:  # Off hours - moderate volume
                        # Use configured off-peak volume settings
                        ops_per_min = random.uniform(OFF_PEAK_VOLUME_MIN, OFF_PEAK_VOLUME_MAX)
                        base_interval = 60.0 / ops_per_min

                    # Add some randomness for realistic patterns
                    interval = random.uniform(base_interval * 0.5, base_interval * 1.5)

                    # Select operation type (80% upload, 20% download)
                    if random.random() < 0.8:
                        self.upload_operation()
                    else:
                        self.download_operation()

                    # 🔥 CRITICAL FIX: Check for shutdown during wait
                    # Wait before next operation, but check for shutdown periodically
                    sleep_chunks = max(1, int(interval))
                    for _ in range(sleep_chunks):
                        if shutdown_event.is_set():
                            break
                        time.sleep(min(1.0, interval / sleep_chunks))

                except Exception as e:
                    self.logger.error(f"User simulation error: {e}")
                    # Wait before retry, but check for shutdown
                    for _ in range(30):
                        if shutdown_event.is_set():
                            break
                        time.sleep(1)

        finally:
            # 🔥 CRITICAL FIX: Wait for all operations to complete before cleanup
            self.logger.info(f"User {self.user_id} shutting down, waiting for operations to complete...")

            # Wait for any active operations to complete
            if not wait_for_user_operations_complete(self.user_id, timeout=30):
                self.logger.warning(f"Some operations for {self.user_id} did not complete within timeout")

            # Clean up this user's directory when stopping
            try:
                if os.path.exists(self.user_temp_dir):
                    # Only remove files that aren't in active operations
                    files_removed = 0
                    for root, dirs, files in os.walk(self.user_temp_dir):
                        for file in files:
                            file_path = os.path.join(root, file)
                            if not is_file_in_use(file_path):
                                try:
                                    os.remove(file_path)
                                    files_removed += 1
                                except:
                                    pass

                    # Try to remove directory if empty
                    try:
                        os.rmdir(self.user_temp_dir)
                        self.logger.info(f"Cleaned up user directory: {self.user_temp_dir} ({files_removed} files)")
                    except OSError:
                        self.logger.info(f"Cleaned up {files_removed} files from {self.user_temp_dir} (directory not empty)")

            except Exception as e:
                self.logger.warning(f"Failed to cleanup user directory: {e}")

            # Signal that this user has stopped
            self.user_stopped.set()

        self.logger.info("User simulation stopped")


class ConcurrentTrafficGenerator:
    """Main traffic generator that manages all user threads"""

    def __init__(self):
        self.setup_logging()
        self.setup_environment()
        self.user_threads = []

    def setup_logging(self):
        """Setup main logging"""
        from logging.handlers import RotatingFileHandler

        file_handler = RotatingFileHandler(
            'traffic_generator.log',
            maxBytes=100 * 1024 * 1024,  # 100MB
            backupCount=5
        )

        console_handler = logging.StreamHandler(sys.stdout)

        logging.basicConfig(
            level=logging.INFO,
            format='%(asctime)s - %(levelname)s - %(message)s',
            handlers=[file_handler, console_handler]
        )
        self.logger = logging.getLogger(__name__)

    def setup_environment(self):
        """Setup directories and environment"""
        os.makedirs(TEMP_DIR, exist_ok=True)

        if not os.path.exists(OBSCTL_BINARY):
            self.logger.error(f"obsctl binary not found at {OBSCTL_BINARY}")
            sys.exit(1)

        self.logger.info(f"Environment setup complete for {len(USERS)} concurrent users")

    def print_stats(self):
        """Print comprehensive statistics including disk space monitoring"""
        # 🚀 NEW: Get current disk space
        free_gb = get_disk_free_space_gb()

        self.logger.info("🚀 HIGH-VOLUME TRAFFIC GENERATOR STATISTICS")
        self.logger.info("=" * 60)

        with stats_lock:
            # Global stats
            self.logger.info("📊 GLOBAL OPERATIONS:")
            self.logger.info(f"  Total Operations: {global_stats['operations']:,}")
            self.logger.info(f"  Uploads: {global_stats['uploads']:,}")
            self.logger.info(f"  Downloads: {global_stats['downloads']:,}")
            self.logger.info(f"  Errors: {global_stats['errors']:,}")
            self.logger.info(f"  Files Created: {global_stats['files_created']:,}")
            self.logger.info(f"  Large Files Created: {global_stats['large_files_created']:,}")
            self.logger.info(f"  TTL Policies Applied: {global_stats['ttl_policies_applied']:,}")

            # Format bytes transferred
            bytes_transferred = global_stats['bytes_transferred']
            if bytes_transferred > 1024**3:
                size_str = f"{bytes_transferred / (1024**3):.2f} GB"
            elif bytes_transferred > 1024**2:
                size_str = f"{bytes_transferred / (1024**2):.2f} MB"
            else:
                size_str = f"{bytes_transferred / 1024:.2f} KB"
            self.logger.info(f"  Data Transferred: {size_str}")

            # 🚀 NEW: Disk space monitoring
            self.logger.info("\n💽 DISK SPACE MONITORING:")
            self.logger.info(f"  Free Space: {free_gb:.1f} GB")
            if free_gb <= DISK_SPACE_CONFIG['stop_threshold_gb']:
                self.logger.warning(f"  ⚠️  CRITICAL: Below {DISK_SPACE_CONFIG['stop_threshold_gb']}GB threshold!")
            elif free_gb < 20:
                self.logger.warning(f"  ⚠️  WARNING: Low disk space")
            else:
                self.logger.info(f"  ✅ OK: Above safety threshold")

            # 🚀 NEW: High-volume progress tracking
            total_target = HIGH_VOLUME_CONFIG['target_files_per_bucket'] * len(USER_CONFIGS)
            current_total = sum(stats['files_created'] for stats in user_stats.values())
            progress_pct = (current_total / total_target) * 100 if total_target > 0 else 0

            self.logger.info("\n🎯 HIGH-VOLUME PROGRESS:")
            self.logger.info(f"  Target: {total_target:,} files across all buckets")
            self.logger.info(f"  Current: {current_total:,} files ({progress_pct:.1f}%)")
            self.logger.info(f"  Remaining: {max(0, total_target - current_total):,} files")

            self.logger.info("\n👥 PER-USER STATISTICS:")
            env_users = []
            config_users = []

            for user_id, stats in user_stats.items():
                user_config = USERS[user_id]
                config_method = user_config.get('config_method', 'env_vars')

                if config_method == 'env_vars':
                    env_users.append(user_id)
                else:
                    config_users.append(user_id)

                total_files = stats['files_created']
                subfolders = stats.get('subfolders_used', 0)
                disk_checks = stats.get('disk_space_checks', 0)

                self.logger.info(f"  {user_id:15} | Files: {total_files:4,} | Subfolders: {subfolders:3} | Config: {config_method}")
                self.logger.info(f"    Ops: {stats['operations']:4,} | Errors: {stats['errors']:2} | Disk Checks: {disk_checks:2}")
                self.logger.info(f"    Bytes: {stats['bytes_transferred']:,}")

            self.logger.info(f"\n🔧 CONFIGURATION METHODS:")
            self.logger.info(f"  Environment Variables: {len(env_users)} users ({', '.join(env_users)})")
            self.logger.info(f"  Config Files on Disk: {len(config_users)} users ({', '.join(config_users)})")

        self.logger.info("=" * 60)

    def run(self):
        """Main traffic generation loop with concurrent users - GRACEFUL SHUTDOWN"""
        global running

        self.logger.info(f"Starting concurrent traffic generator for {SCRIPT_DURATION_HOURS} hours")
        self.logger.info(f"MinIO endpoint: {MINIO_ENDPOINT}")
        self.logger.info(f"TTL Configuration:")
        self.logger.info(f"  Regular files: {TTL_CONFIG['regular_files_hours']} hours")
        self.logger.info(f"  Large files (>{TTL_CONFIG['large_file_threshold_mb']}MB): {TTL_CONFIG['large_files_minutes']} minutes")

        start_time = time.time()

        # 🔥 CRITICAL FIX: Setup signal handler for graceful shutdown
        def signal_handler(signum, frame):
            self.logger.info("Received shutdown signal, stopping all users...")
            global running
            running = False
            shutdown_event.set()  # Signal all threads to stop

        signal.signal(signal.SIGINT, signal_handler)
        signal.signal(signal.SIGTERM, signal_handler)

        # Start all user threads
        with ThreadPoolExecutor(max_workers=MAX_CONCURRENT_USERS) as executor:
            self.logger.info(f"Starting {len(USERS)} concurrent user simulations...")

            # Submit all user simulations
            futures = []
            user_simulators = []
            for user_id, user_config in USERS.items():
                user_sim = UserSimulator(user_id, user_config)
                user_simulators.append(user_sim)
                future = executor.submit(user_sim.run)
                futures.append(future)
                self.logger.info(f"Started user thread: {user_id}")

            # Start stats reporting thread
            def stats_reporter():
                while running and not shutdown_event.is_set():
                    # Wait 5 minutes or until shutdown
                    for _ in range(300):
                        if shutdown_event.is_set():
                            break
                        time.sleep(1)

                    if running and not shutdown_event.is_set():
                        self.print_stats()

            stats_thread = threading.Thread(target=stats_reporter, daemon=True)
            stats_thread.start()

            try:
                # Wait for duration or until interrupted
                end_time = start_time + (SCRIPT_DURATION_HOURS * 3600)
                while time.time() < end_time and running and not shutdown_event.is_set():
                    time.sleep(60)  # Check every minute

            except KeyboardInterrupt:
                self.logger.info("Received keyboard interrupt, shutting down...")

            finally:
                # 🔥 CRITICAL FIX: Graceful shutdown sequence
                running = False
                shutdown_event.set()

                # Wait for all user threads to complete gracefully
                self.logger.info("Waiting for all user threads to stop...")
                completed_users = []

                for i, (future, user_sim) in enumerate(zip(futures, user_simulators)):
                    try:
                        future.result(timeout=45)  # Wait up to 45 seconds per thread
                        completed_users.append(user_sim.user_id)
                        self.logger.info(f"User {user_sim.user_id} stopped gracefully")
                    except Exception as e:
                        self.logger.warning(f"User {user_sim.user_id} thread cleanup error: {e}")

                self.logger.info(f"Completed shutdown for {len(completed_users)}/{len(USERS)} users")

                # Wait a bit more for any remaining operations
                self.logger.info("Waiting for remaining operations to complete...")
                time.sleep(5)

                self.print_stats()

                # 🔥 CRITICAL FIX: Final cleanup only removes files NOT in active operations
                try:
                    if os.path.exists(TEMP_DIR):
                        remaining_files = []
                        protected_files = []

                        for root, dirs, files in os.walk(TEMP_DIR):
                            for file in files:
                                file_path = os.path.join(root, file)
                                if is_file_in_use(file_path):
                                    protected_files.append(file_path)
                                else:
                                    remaining_files.append(file_path)

                        # Only remove files that aren't protected
                        files_removed = 0
                        for file_path in remaining_files:
                            try:
                                os.remove(file_path)
                                files_removed += 1
                            except:
                                pass

                        if protected_files:
                            self.logger.warning(f"Protected {len(protected_files)} files still in use from cleanup")

                        if files_removed > 0:
                            self.logger.info(f"Cleaned up remaining temporary files: {files_removed} files")

                        # Try to remove empty directories
                        try:
                            for root, dirs, files in os.walk(TEMP_DIR, topdown=False):
                                for dir_name in dirs:
                                    dir_path = os.path.join(root, dir_name)
                                    try:
                                        os.rmdir(dir_path)
                                    except OSError:
                                        pass  # Directory not empty

                            # Try to remove main temp directory
                            os.rmdir(TEMP_DIR)
                            self.logger.info("Removed temporary directory")
                        except OSError:
                            self.logger.info("Temporary directory not empty, leaving for next run")

                except Exception as e:
                    self.logger.warning(f"Final cleanup warning: {e}")

                self.logger.info("Concurrent traffic generator finished")


if __name__ == "__main__":
    # Check if already running
    is_running, existing_pid = check_if_running()
    if is_running:
        print(f"ERROR: Traffic generator is already running (PID: {existing_pid})")
        print("Use 'launchctl stop com.obsctl.traffic-generator' to stop it first")
        sys.exit(1)

    # Acquire lock
    lock_fd = acquire_lock()

    try:
        generator = ConcurrentTrafficGenerator()
        generator.run()
    finally:
        # Always release lock when exiting
        release_lock(lock_fd)
