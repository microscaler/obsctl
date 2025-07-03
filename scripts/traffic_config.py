# Traffic Generator Configuration
# This file contains all the configuration settings for the traffic generator
# Keeping them separate prevents accidental overwrites during code changes

import shutil

# Global Configuration
TEMP_DIR = "/tmp/obsctl-traffic"
OBSCTL_BINARY = "./target/release/obsctl"
MINIO_ENDPOINT = "http://127.0.0.1:9000"
SCRIPT_DURATION_HOURS = 0.167  # 10 minutes
MAX_CONCURRENT_USERS = 10

# 🚀 NEW: Disk Space Monitoring Configuration
DISK_SPACE_CONFIG = {
    'min_free_gb': 10,          # Minimum free disk space (10GB)
    'stop_threshold_gb': 11,    # Stop generation when free space drops to 11GB
    'check_interval_seconds': 30, # Check disk space every 30 seconds
    'emergency_cleanup_gb': 5,  # Emergency cleanup if below 5GB
}

# 🚀 NEW: High-Volume Generation Settings
HIGH_VOLUME_CONFIG = {
    'target_files_per_bucket': 5000,  # Target 5000+ files per bucket
    'use_subfolders': True,           # Enable subfolder organization
    'max_subfolder_depth': 3,         # Up to 3 levels deep
    'files_per_subfolder': 200,       # 200 files per subfolder max
    'small_file_bias': 0.8,           # 80% small files for high object count
}

# 🚀 NEW: Subfolder Structure Templates
SUBFOLDER_TEMPLATES = {
    'alice-dev-workspace': [
        'projects/{project}/src',
        'projects/{project}/tests',
        'projects/{project}/docs',
        'libraries/shared',
        'config/environments/{env}',
        'backups/{date}',
        'temp/builds'
    ],
    'bob-marketing-assets': [
        'campaigns/{campaign}/assets',
        'campaigns/{campaign}/creative',
        'campaigns/{campaign}/reports',
        'brand-assets/logos',
        'brand-assets/templates',
        'social-media/{platform}',
        'presentations/{quarter}'
    ],
    'carol-analytics': [
        'datasets/{dataset}/raw',
        'datasets/{dataset}/processed',
        'datasets/{dataset}/analysis',
        'models/{model}/training',
        'models/{model}/validation',
        'reports/{year}/{month}',
        'experiments/{experiment_id}'
    ],
    'david-backups': [
        'daily/{year}/{month}/{day}',
        'weekly/{year}/week-{week}',
        'monthly/{year}/{month}',
        'systems/{system}/configs',
        'systems/{system}/logs',
        'disaster-recovery/snapshots',
        'archive/{year}'
    ],
    'eve-creative-work': [
        'projects/{project}/mockups',
        'projects/{project}/assets',
        'projects/{project}/finals',
        'resources/stock-photos',
        'resources/icons',
        'templates/{category}',
        'client-work/{client}'
    ],
    'frank-research-data': [
        'papers/{year}/{topic}',
        'data/{study}/raw',
        'data/{study}/processed',
        'analysis/{study}/results',
        'publications/drafts',
        'publications/final',
        'collaboration/{partner}'
    ],
    'grace-sales-materials': [
        'leads/{region}/{quarter}',
        'proposals/{client}',
        'contracts/{year}',
        'presentations/templates',
        'presentations/custom',
        'reports/{quarter}',
        'training-materials'
    ],
    'henry-operations': [
        'infrastructure/{environment}',
        'deployments/{service}/{version}',
        'monitoring/dashboards',
        'monitoring/alerts',
        'scripts/automation',
        'logs/{service}/{date}',
        'security/audits'
    ],
    'iris-content-library': [
        'library/{category}/{subcategory}',
        'workflows/templates',
        'workflows/active',
        'archive/{year}/{quarter}',
        'metadata/schemas',
        'metadata/catalogs',
        'staging/review'
    ],
    'jack-mobile-apps': [
        'apps/{app}/ios/src',
        'apps/{app}/android/src',
        'apps/{app}/shared/assets',
        'libraries/ui-components',
        'libraries/networking',
        'builds/{app}/{version}',
        'testing/{app}/automated'
    ]
}

# Traffic Volume Settings (operations per minute) - RAMPED UP FOR HIGH VOLUME
PEAK_VOLUME_MIN = 500    # Minimum ops/min during peak hours (5x increase)
PEAK_VOLUME_MAX = 2000   # Maximum ops/min during peak hours (4x increase)
OFF_PEAK_VOLUME_MIN = 100 # Minimum ops/min during off-peak hours (10x increase)
OFF_PEAK_VOLUME_MAX = 500 # Maximum ops/min during off-peak hours (10x increase)

# File TTL Settings (in seconds) - SHORTER FOR HIGH TURNOVER
REGULAR_FILE_TTL = 1 * 3600     # 1 hour for regular files (reduced from 3)
LARGE_FILE_TTL = 30 * 60        # 30 minutes for large files (reduced from 60)
LARGE_FILE_THRESHOLD = 50 * 1024 * 1024  # 50MB threshold (reduced from 100MB)

# User Configurations
USER_CONFIGS = {
    'alice-dev': {
        'description': 'Software Developer - Heavy code and docs',
        'bucket': 'alice-dev-workspace',
        'timezone_offset': 0,  # UTC
        'peak_hours': (9, 17),  # 9 AM to 5 PM
        'activity_multiplier': 2.0,  # Increased for high volume
        'file_preferences': {
            'code': 0.5,      # More code files
            'documents': 0.3,
            'images': 0.1,
            'archives': 0.05,
            'media': 0.05
        },
        'config_method': 'env_vars'  # Use environment variables
    },
    'bob-marketing': {
        'description': 'Marketing Manager - Media and presentations',
        'bucket': 'bob-marketing-assets',
        'timezone_offset': -5,  # EST
        'peak_hours': (8, 16),
        'activity_multiplier': 1.8,  # Increased
        'file_preferences': {
            'media': 0.3,
            'images': 0.4,    # More images
            'documents': 0.2,
            'code': 0.05,
            'archives': 0.05
        },
        'config_method': 'config_file'  # Use config files on disk
    },
    'carol-data': {
        'description': 'Data Scientist - Large datasets and analysis',
        'bucket': 'carol-analytics',
        'timezone_offset': -8,  # PST
        'peak_hours': (10, 18),
        'activity_multiplier': 2.5,  # Highest activity
        'file_preferences': {
            'documents': 0.4,  # More data files
            'archives': 0.3,
            'code': 0.2,
            'images': 0.05,
            'media': 0.05
        },
        'config_method': 'env_vars'  # Use environment variables
    },
    'david-backup': {
        'description': 'IT Admin - Automated backup systems',
        'bucket': 'david-backups',
        'timezone_offset': 0,  # UTC
        'peak_hours': (2, 6),  # Night backup window
        'activity_multiplier': 3.5,  # Highest for backup scenarios
        'file_preferences': {
            'archives': 0.6,
            'documents': 0.2,
            'code': 0.1,
            'images': 0.05,
            'media': 0.05
        },
        'config_method': 'config_file'  # Use config files on disk
    },
    'eve-design': {
        'description': 'Creative Designer - Images and media files',
        'bucket': 'eve-creative-work',
        'timezone_offset': 1,  # CET
        'peak_hours': (9, 17),
        'activity_multiplier': 2.2,  # Increased
        'file_preferences': {
            'images': 0.6,    # Heavy image focus
            'media': 0.2,
            'documents': 0.1,
            'code': 0.05,
            'archives': 0.05
        },
        'config_method': 'env_vars'  # Use environment variables
    },
    'frank-research': {
        'description': 'Research Scientist - Academic papers and data',
        'bucket': 'frank-research-data',
        'timezone_offset': -3,  # BRT
        'peak_hours': (14, 22),  # Afternoon/evening researcher
        'activity_multiplier': 1.9,  # Increased
        'file_preferences': {
            'documents': 0.5,  # Heavy document focus
            'archives': 0.2,
            'code': 0.2,
            'images': 0.05,
            'media': 0.05
        },
        'config_method': 'config_file'  # Use config files on disk
    },
    'grace-sales': {
        'description': 'Sales Manager - Presentations and materials',
        'bucket': 'grace-sales-materials',
        'timezone_offset': -6,  # CST
        'peak_hours': (8, 16),
        'activity_multiplier': 1.7,  # Increased
        'file_preferences': {
            'documents': 0.5,  # Heavy presentations
            'images': 0.3,
            'media': 0.1,
            'code': 0.05,
            'archives': 0.05
        },
        'config_method': 'env_vars'  # Use environment variables
    },
    'henry-ops': {
        'description': 'DevOps Engineer - Infrastructure and configs',
        'bucket': 'henry-operations',
        'timezone_offset': 0,  # UTC
        'peak_hours': (0, 8),  # Night shift operations
        'activity_multiplier': 3.0,  # High for ops
        'file_preferences': {
            'code': 0.5,      # Heavy config files
            'archives': 0.2,
            'documents': 0.2,
            'images': 0.05,
            'media': 0.05
        },
        'config_method': 'config_file'  # Use config files on disk
    },
    'iris-content': {
        'description': 'Content Manager - Digital asset library',
        'bucket': 'iris-content-library',
        'timezone_offset': 9,  # JST
        'peak_hours': (9, 17),
        'activity_multiplier': 2.3,  # Increased
        'file_preferences': {
            'media': 0.4,
            'images': 0.3,
            'documents': 0.2,
            'archives': 0.05,
            'code': 0.05
        },
        'config_method': 'env_vars'  # Use environment variables
    },
    'jack-mobile': {
        'description': 'Mobile Developer - App assets and code',
        'bucket': 'jack-mobile-apps',
        'timezone_offset': 5.5,  # IST
        'peak_hours': (10, 18),
        'activity_multiplier': 2.1,  # Increased
        'file_preferences': {
            'code': 0.5,      # Heavy code focus
            'images': 0.2,
            'media': 0.2,
            'documents': 0.05,
            'archives': 0.05
        },
        'config_method': 'config_file'  # Use config files on disk
    }
}

# File type extensions
FILE_EXTENSIONS = {
    'code': ['.py', '.js', '.html', '.css', '.rs', '.go', '.java', '.cpp', '.c', '.json', '.xml', '.yaml', '.toml'],
    'documents': ['.pdf', '.docx', '.txt', '.md', '.xlsx', '.pptx', '.csv', '.rtf'],
    'images': ['.jpg', '.png', '.gif', '.svg', '.bmp', '.webp', '.tiff'],
    'archives': ['.zip', '.tar.gz', '.rar', '.7z', '.tar', '.gz'],
    'media': ['.mp4', '.avi', '.mov', '.mkv', '.mp3', '.wav', '.flac', '.ogg']
}

# Environment variables for obsctl
OBSCTL_ENV = {
    'OTEL_ENABLED': 'true',
    'OTEL_EXPORTER_OTLP_ENDPOINT': 'http://localhost:4317',
    'JAEGER_ENABLED': 'true',
    'JAEGER_ENDPOINT': 'http://localhost:14250',
    'JAEGER_SERVICE_NAME': 'obsctl',
    'JAEGER_SAMPLING_RATIO': '1.0',
    'AWS_ACCESS_KEY_ID': 'minioadmin',
    'AWS_SECRET_ACCESS_KEY': 'minioadmin123',
    'AWS_ENDPOINT_URL': MINIO_ENDPOINT,
    'AWS_REGION': 'us-east-1'
}

# 🚀 NEW: Helper function for disk space checking
def get_disk_free_space_gb(path="/"):
    """Get free disk space in GB for the given path"""
    try:
        _, _, free_bytes = shutil.disk_usage(path)
        return free_bytes / (1024**3)  # Convert to GB
    except Exception:
        return float('inf')  # If we can't check, assume infinite space

def should_stop_generation():
    """Check if we should stop generation due to low disk space"""
    free_gb = get_disk_free_space_gb()
    return free_gb <= DISK_SPACE_CONFIG['stop_threshold_gb']

def needs_emergency_cleanup():
    """Check if we need emergency cleanup"""
    free_gb = get_disk_free_space_gb()
    return free_gb <= DISK_SPACE_CONFIG['emergency_cleanup_gb']
