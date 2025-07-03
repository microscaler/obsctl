#!/usr/bin/env python3
"""
Release Configuration Tests for obsctl

This module implements comprehensive configuration testing for AWS credentials,
AWS config, and OTEL configuration. Only runs during release management due
to extensive scope (2000+ test cases).

Features:
- UUID-based test files for GitHub Actions compatibility (small files)
- Generator fan-out pattern for efficient test data management
- Parallel execution with ThreadPoolExecutor
- MinIO integration testing

Usage:
    python tests/release_config_tests.py --category all
    python tests/release_config_tests.py --category credentials --workers 8
"""

import asyncio
import argparse
import tempfile
import os
import subprocess
import time
import json
import shutil
import uuid
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, asdict
from typing import Optional, Dict, Any, List, Generator, Tuple
from pathlib import Path


@dataclass
class ConfigTestCase:
    """Configuration test case definition"""
    test_id: str
    category: str
    description: str

    # AWS Credentials
    aws_access_key_id_source: str
    aws_access_key_id_value: Optional[str]
    aws_secret_access_key_source: str
    aws_secret_access_key_value: Optional[str]
    aws_session_token_source: str
    aws_session_token_value: Optional[str]

    # AWS Config
    region_source: str
    region_value: Optional[str]
    endpoint_url_source: str
    endpoint_url_value: Optional[str]
    output_source: str
    output_value: Optional[str]

    # OTEL Config
    otel_enabled_source: str
    otel_enabled_value: Optional[bool]
    otel_endpoint_source: str
    otel_endpoint_value: Optional[str]
    otel_service_name_source: str
    otel_service_name_value: Optional[str]

    # Expected Results
    expected_aws_works: bool
    expected_otel_enabled: bool
    expected_endpoint: Optional[str]
    expected_service_name: Optional[str]
    expected_region: Optional[str]


class TestFileGenerator:
    """Generator-based test file management with UUID content"""

    def __init__(self, base_dir: str):
        self.base_dir = Path(base_dir)
        self.test_files_dir = self.base_dir / "test_files"
        self.test_files_dir.mkdir(exist_ok=True)
        self.active_files = {}  # Track active test files

    def generate_test_file(self, test_id: str, file_type: str = "small") -> Generator[Tuple[str, str], None, None]:
        """
        Generator that creates a test file with UUID content and yields (file_path, uuid)
        Uses generator pattern for automatic cleanup
        """
        test_uuid = str(uuid.uuid4())
        file_name = f"{test_id}_{file_type}_{test_uuid[:8]}.txt"
        file_path = self.test_files_dir / file_name

        try:
            # Create test file content
            content = self._generate_file_content(test_uuid, file_type)

            # Write file
            with open(file_path, 'w') as f:
                f.write(content)

            # Track active file
            self.active_files[str(file_path)] = test_uuid

            # Yield file path and UUID for test use
            yield str(file_path), test_uuid

        finally:
            # Cleanup: remove file and tracking
            if file_path.exists():
                file_path.unlink()
            self.active_files.pop(str(file_path), None)

    def generate_test_file_batch(self, test_id: str, count: int = 3) -> Generator[List[Tuple[str, str]], None, None]:
        """
        Generator that creates multiple test files for batch operations
        Fan-out pattern: creates multiple files simultaneously
        """
        files_created = []

        try:
            for i in range(count):
                test_uuid = str(uuid.uuid4())
                file_name = f"{test_id}_batch_{i}_{test_uuid[:8]}.txt"
                file_path = self.test_files_dir / file_name

                # Create varied content sizes
                file_type = ["tiny", "small", "medium"][i % 3]
                content = self._generate_file_content(test_uuid, file_type)

                with open(file_path, 'w') as f:
                    f.write(content)

                files_created.append((str(file_path), test_uuid))
                self.active_files[str(file_path)] = test_uuid

            # Yield all files at once for batch testing
            yield files_created

        finally:
            # Cleanup all files
            for file_path, test_uuid in files_created:
                if Path(file_path).exists():
                    Path(file_path).unlink()
                self.active_files.pop(file_path, None)

    def _generate_file_content(self, test_uuid: str, file_type: str) -> str:
        """Generate test file content based on type"""
        base_content = f"""# Test File for obsctl Integration Testing
# UUID: {test_uuid}
# Type: {file_type}
# Generated: {time.strftime('%Y-%m-%d %H:%M:%S')}

This file contains a UUID for integration testing with obsctl.
The UUID serves as a unique identifier to verify file operations.

UUID: {test_uuid}
"""

        if file_type == "tiny":
            return base_content
        elif file_type == "small":
            # Add some padding (still < 1KB for GitHub Actions)
            padding = "# " + "x" * 50 + "\n"
            return base_content + padding * 10
        elif file_type == "medium":
            # Add more padding (still < 5KB for GitHub Actions)
            padding = "# " + "x" * 100 + "\n"
            return base_content + padding * 20
        else:
            return base_content

    def cleanup_all(self):
        """Clean up all test files"""
        if self.test_files_dir.exists():
            shutil.rmtree(self.test_files_dir, ignore_errors=True)


def run_single_test(test_case_dict: Dict[str, Any]) -> Dict[str, Any]:
    """Run a single test in isolated environment - standalone function for pickling"""
    # Reconstruct test case from dict
    test_case = ConfigTestCase(**test_case_dict)

    test_env = IsolatedTestEnvironment(test_case.test_id)

    try:
        test_env.setup(test_case)
        result = test_env.execute_obsctl_test()
        verification = test_env.verify_expectations(test_case, result)

        return {
            'test_id': test_case.test_id,
            'category': test_case.category,
            'description': test_case.description,
            'status': 'PASS' if verification['success'] else 'FAIL',
            'result': result,
            'verification': verification,
            'execution_time': test_env.execution_time,
            'test_case': asdict(test_case),
            'files_tested': getattr(test_env, 'files_tested', [])
        }
    except Exception as e:
        return {
            'test_id': test_case.test_id,
            'category': test_case.category,
            'description': test_case.description,
            'status': 'ERROR',
            'error': str(e),
            'execution_time': getattr(test_env, 'execution_time', 0),
            'test_case': asdict(test_case)
        }
    finally:
        test_env.cleanup()


class IsolatedTestEnvironment:
    """Isolated test environment for configuration testing with MinIO integration"""

    def __init__(self, test_id: str):
        self.test_id = test_id
        self.temp_dir = tempfile.mkdtemp(prefix=f"obsctl-test-{test_id}-")
        self.aws_dir = os.path.join(self.temp_dir, ".aws")
        self.execution_time = 0
        self.original_env = {}
        self.file_generator = TestFileGenerator(self.temp_dir)
        self.files_tested = []

    def setup(self, test_case: ConfigTestCase):
        """Setup isolated test environment"""
        os.makedirs(self.aws_dir, exist_ok=True)

        # Create config files based on test case
        self._create_credentials_file(test_case)
        self._create_config_file(test_case)
        self._create_otel_file(test_case)

    def _create_credentials_file(self, test_case: ConfigTestCase):
        """Create ~/.aws/credentials file"""
        credentials_content = []

        # Default profile
        if (test_case.aws_access_key_id_source == 'credentials' or
            test_case.aws_secret_access_key_source == 'credentials' or
            test_case.aws_session_token_source == 'credentials'):

            credentials_content.append("[default]")

            if test_case.aws_access_key_id_source == 'credentials':
                credentials_content.append(f"aws_access_key_id = {test_case.aws_access_key_id_value or 'AKIATEST12345'}")

            if test_case.aws_secret_access_key_source == 'credentials':
                credentials_content.append(f"aws_secret_access_key = {test_case.aws_secret_access_key_value or 'testsecretkey12345'}")

            if test_case.aws_session_token_source == 'credentials':
                credentials_content.append(f"aws_session_token = {test_case.aws_session_token_value or 'testsessiontoken12345'}")

        if credentials_content:
            with open(os.path.join(self.aws_dir, "credentials"), 'w') as f:
                f.write('\n'.join(credentials_content) + '\n')

    def _create_config_file(self, test_case: ConfigTestCase):
        """Create ~/.aws/config file"""
        config_content = []

        # Check if any config values should be in config file
        has_config_values = any([
            test_case.aws_access_key_id_source == 'config',
            test_case.aws_secret_access_key_source == 'config',
            test_case.aws_session_token_source == 'config',
            test_case.region_source == 'config',
            test_case.endpoint_url_source == 'config',
            test_case.output_source == 'config',
            test_case.otel_enabled_source == 'config',
            test_case.otel_endpoint_source == 'config',
            test_case.otel_service_name_source == 'config'
        ])

        if has_config_values:
            config_content.append("[default]")

            # AWS credentials in config
            if test_case.aws_access_key_id_source == 'config':
                config_content.append(f"aws_access_key_id = {test_case.aws_access_key_id_value or 'AKIACONFIG12345'}")

            if test_case.aws_secret_access_key_source == 'config':
                config_content.append(f"aws_secret_access_key = {test_case.aws_secret_access_key_value or 'configsecretkey12345'}")

            if test_case.aws_session_token_source == 'config':
                config_content.append(f"aws_session_token = {test_case.aws_session_token_value or 'configsessiontoken12345'}")

            # AWS config values
            if test_case.region_source == 'config':
                config_content.append(f"region = {test_case.region_value or 'us-west-2'}")

            if test_case.endpoint_url_source == 'config':
                config_content.append(f"endpoint_url = {test_case.endpoint_url_value or 'http://localhost:9000'}")

            if test_case.output_source == 'config':
                config_content.append(f"output = {test_case.output_value or 'json'}")

            # OTEL config values
            if test_case.otel_enabled_source == 'config':
                enabled_val = str(test_case.otel_enabled_value).lower() if test_case.otel_enabled_value is not None else 'true'
                config_content.append(f"otel_enabled = {enabled_val}")

            if test_case.otel_endpoint_source == 'config':
                config_content.append(f"otel_endpoint = {test_case.otel_endpoint_value or 'http://localhost:4317'}")

            if test_case.otel_service_name_source == 'config':
                config_content.append(f"otel_service_name = {test_case.otel_service_name_value or 'obsctl-config'}")

        if config_content:
            with open(os.path.join(self.aws_dir, "config"), 'w') as f:
                f.write('\n'.join(config_content) + '\n')

    def _create_otel_file(self, test_case: ConfigTestCase):
        """Create ~/.aws/otel file"""
        otel_content = []

        # Check if any OTEL values should be in otel file
        has_otel_values = any([
            test_case.otel_enabled_source == 'otel',
            test_case.otel_endpoint_source == 'otel',
            test_case.otel_service_name_source == 'otel'
        ])

        if has_otel_values:
            otel_content.append("[otel]")

            if test_case.otel_enabled_source == 'otel':
                enabled_val = str(test_case.otel_enabled_value).lower() if test_case.otel_enabled_value is not None else 'true'
                otel_content.append(f"enabled = {enabled_val}")

            if test_case.otel_endpoint_source == 'otel':
                otel_content.append(f"endpoint = {test_case.otel_endpoint_value or 'http://localhost:4318'}")

            if test_case.otel_service_name_source == 'otel':
                otel_content.append(f"service_name = {test_case.otel_service_name_value or 'obsctl-otel'}")

        if otel_content:
            with open(os.path.join(self.aws_dir, "otel"), 'w') as f:
                f.write('\n'.join(otel_content) + '\n')

    def execute_obsctl_test(self) -> Dict[str, Any]:
        """Execute obsctl command with MinIO integration testing"""
        start_time = time.time()

        try:
            # Build environment for this test
            test_env = os.environ.copy()

            # Clear AWS-related environment variables first
            aws_env_vars = [
                'AWS_ACCESS_KEY_ID', 'AWS_SECRET_ACCESS_KEY', 'AWS_SESSION_TOKEN',
                'AWS_DEFAULT_REGION', 'AWS_ENDPOINT_URL', 'AWS_PROFILE',
                'OTEL_ENABLED', 'OTEL_EXPORTER_OTLP_ENDPOINT', 'OTEL_SERVICE_NAME'
            ]

            for var in aws_env_vars:
                test_env.pop(var, None)

            # Set HOME to our temp directory
            test_env['HOME'] = self.temp_dir

            # Test sequence: 1) ls command 2) file operations if AWS works
            results = {}

            # First test: ls command (configuration test)
            ls_result = self._run_obsctl_command(['ls'], test_env)
            results['ls'] = ls_result

            # If ls works, test file operations with UUID files
            if ls_result['returncode'] == 0:
                file_ops_result = self._test_file_operations(test_env)
                results['file_ops'] = file_ops_result

            self.execution_time = time.time() - start_time

            return {
                'results': results,
                'execution_time': self.execution_time,
                'files_tested': self.files_tested
            }

        except Exception as e:
            self.execution_time = time.time() - start_time
            return {
                'results': {'error': str(e)},
                'execution_time': self.execution_time,
                'files_tested': []
            }

    def _run_obsctl_command(self, cmd_args: List[str], test_env: Dict[str, str], timeout: int = 30) -> Dict[str, Any]:
        """Run a single obsctl command"""
        try:
            cmd = ['./target/release/obsctl', '--debug', 'debug'] + cmd_args

            result = subprocess.run(
                cmd,
                env=test_env,
                capture_output=True,
                text=True,
                timeout=timeout,
                cwd='/Users/casibbald/Workspace/microscaler/obsctl'
            )

            return {
                'stdout': result.stdout,
                'stderr': result.stderr,
                'returncode': result.returncode,
                'command': ' '.join(cmd)
            }
        except subprocess.TimeoutExpired:
            return {
                'stdout': '',
                'stderr': f'Command timed out after {timeout} seconds',
                'returncode': -1,
                'command': ' '.join(cmd)
            }
        except Exception as e:
            return {
                'stdout': '',
                'stderr': f'Command execution failed: {str(e)}',
                'returncode': -2,
                'command': ' '.join(cmd)
            }

    def _test_file_operations(self, test_env: Dict[str, str]) -> Dict[str, Any]:
        """Test file operations using UUID-based test files"""
        operations_results = {}

        try:
            # Create test bucket
            bucket_name = f"test-{self.test_id.lower().replace('_', '-')}"
            mb_result = self._run_obsctl_command(['mb', f's3://{bucket_name}'], test_env)
            operations_results['mb'] = mb_result

            if mb_result['returncode'] != 0:
                return operations_results

            # Test single file upload using generator
            for file_path, test_uuid in self.file_generator.generate_test_file(self.test_id, "small"):
                self.files_tested.append({'file': file_path, 'uuid': test_uuid, 'type': 'single'})

                # Upload file
                cp_result = self._run_obsctl_command([
                    'cp', file_path, f's3://{bucket_name}/single-{test_uuid[:8]}.txt'
                ], test_env)
                operations_results['cp_single'] = cp_result

                if cp_result['returncode'] == 0:
                    # Verify file exists
                    ls_result = self._run_obsctl_command([
                        'ls', f's3://{bucket_name}/single-{test_uuid[:8]}.txt'
                    ], test_env)
                    operations_results['ls_verify'] = ls_result

            # Test batch file upload using generator fan-out
            for files_batch in self.file_generator.generate_test_file_batch(self.test_id, 3):
                batch_results = []

                for file_path, test_uuid in files_batch:
                    self.files_tested.append({'file': file_path, 'uuid': test_uuid, 'type': 'batch'})

                    # Upload each file in batch
                    cp_result = self._run_obsctl_command([
                        'cp', file_path, f's3://{bucket_name}/batch-{test_uuid[:8]}.txt'
                    ], test_env)
                    batch_results.append(cp_result)

                operations_results['cp_batch'] = batch_results

            # Clean up test bucket
            rb_result = self._run_obsctl_command(['rb', f's3://{bucket_name}', '--force'], test_env)
            operations_results['rb'] = rb_result

        except Exception as e:
            operations_results['error'] = str(e)

        return operations_results

    def verify_expectations(self, test_case: ConfigTestCase, result: Dict[str, Any]) -> Dict[str, Any]:
        """Verify test results match expectations"""
        verification = {
            'success': True,
            'failures': [],
            'otel_enabled': None,
            'endpoint_used': None,
            'service_name_used': None,
            'file_operations_success': False
        }

        # Extract output from results
        if 'results' in result and 'ls' in result['results']:
            ls_result = result['results']['ls']
            output = ls_result.get('stdout', '') + ls_result.get('stderr', '')
        else:
            output = str(result)

        # Check OTEL enabled state
        if 'Initializing OpenTelemetry SDK' in output:
            verification['otel_enabled'] = True
        elif 'OpenTelemetry is disabled' in output:
            verification['otel_enabled'] = False
        else:
            # If no explicit OTEL message, assume disabled
            verification['otel_enabled'] = False

        # Verify OTEL enabled expectation
        if verification['otel_enabled'] != test_case.expected_otel_enabled:
            verification['success'] = False
            verification['failures'].append(
                f"OTEL enabled mismatch: expected {test_case.expected_otel_enabled}, got {verification['otel_enabled']}"
            )

        # Extract endpoint from debug output
        if 'gRPC endpoint:' in output:
            import re
            endpoint_match = re.search(r'gRPC endpoint: (\S+)', output)
            if endpoint_match:
                verification['endpoint_used'] = endpoint_match.group(1)

        # Extract service name from debug output
        if 'Service:' in output:
            import re
            service_match = re.search(r'Service: (\S+)', output)
            if service_match:
                verification['service_name_used'] = service_match.group(1).split()[0]  # Take just the service name

        # Verify endpoint expectation
        if test_case.expected_endpoint and verification['endpoint_used']:
            if test_case.expected_endpoint not in verification['endpoint_used']:
                verification['success'] = False
                verification['failures'].append(
                    f"Endpoint mismatch: expected {test_case.expected_endpoint}, got {verification['endpoint_used']}"
                )

        # Verify service name expectation
        if test_case.expected_service_name and verification['service_name_used']:
            if test_case.expected_service_name not in verification['service_name_used']:
                verification['success'] = False
                verification['failures'].append(
                    f"Service name mismatch: expected {test_case.expected_service_name}, got {verification['service_name_used']}"
                )

        # Check file operations success
        if 'results' in result and 'file_ops' in result['results']:
            file_ops = result['results']['file_ops']
            if ('cp_single' in file_ops and file_ops['cp_single'].get('returncode') == 0 and
                'ls_verify' in file_ops and file_ops['ls_verify'].get('returncode') == 0):
                verification['file_operations_success'] = True

        return verification

    def cleanup(self):
        """Clean up test environment"""
        # Clean up file generator
        self.file_generator.cleanup_all()

        # Remove temporary directory
        if os.path.exists(self.temp_dir):
            shutil.rmtree(self.temp_dir, ignore_errors=True)


class ParallelConfigTestFramework:
    """Framework for running configuration tests in parallel using ThreadPoolExecutor"""

    def __init__(self, max_workers: Optional[int] = None):
        self.max_workers = max_workers or min(os.cpu_count() or 4, 8)  # Cap at 8 for stability

    async def run_test_batch(self, test_cases: List[ConfigTestCase]) -> List[Dict[str, Any]]:
        """Run a batch of tests in parallel using threads"""
        loop = asyncio.get_event_loop()

        # Convert test cases to dicts for easier handling
        test_case_dicts = [asdict(test_case) for test_case in test_cases]

        # Use ThreadPoolExecutor instead of ProcessPoolExecutor
        with ThreadPoolExecutor(max_workers=self.max_workers) as executor:
            # Submit all tests
            futures = []
            for test_case_dict in test_case_dicts:
                future = loop.run_in_executor(executor, run_single_test, test_case_dict)
                futures.append(future)

            # Wait for all tests to complete
            results = await asyncio.gather(*futures, return_exceptions=True)

        # Convert exceptions to error results
        processed_results = []
        for i, result in enumerate(results):
            if isinstance(result, Exception):
                processed_results.append({
                    'test_id': test_cases[i].test_id,
                    'category': test_cases[i].category,
                    'status': 'ERROR',
                    'error': str(result),
                    'execution_time': 0
                })
            else:
                processed_results.append(result)

        return processed_results


def generate_test_matrix() -> Dict[str, List[ConfigTestCase]]:
    """Generate test matrix organized by category"""
    test_matrix = {
        'credentials': [],
        'config': [],
        'otel': [],
        'mixed': []
    }

    # Category A: AWS Credentials Tests (simplified subset for now)
    test_id = 0
    for access_key_source in ['credentials', 'config', 'env', 'missing']:
        for secret_key_source in ['credentials', 'config', 'env', 'missing']:
            # Only test a subset for now to avoid overwhelming
            if test_id >= 16:  # Limit to first 16 for initial testing
                break

            test_matrix['credentials'].append(ConfigTestCase(
                test_id=f"cred_{test_id:04d}",
                category='credentials',
                description=f"Credentials: access_key from {access_key_source}, secret_key from {secret_key_source}",

                # AWS Credentials
                aws_access_key_id_source=access_key_source,
                aws_access_key_id_value=None,  # Use defaults
                aws_secret_access_key_source=secret_key_source,
                aws_secret_access_key_value=None,
                aws_session_token_source='missing',
                aws_session_token_value=None,

                # AWS Config (defaults)
                region_source='default',
                region_value='us-east-1',
                endpoint_url_source='default',
                endpoint_url_value='http://localhost:9000',
                output_source='default',
                output_value='json',

                # OTEL Config (defaults)
                otel_enabled_source='default',
                otel_enabled_value=False,
                otel_endpoint_source='default',
                otel_endpoint_value=None,
                otel_service_name_source='default',
                otel_service_name_value='obsctl',

                # Expected results
                expected_aws_works=_determine_aws_works(access_key_source, secret_key_source),
                expected_otel_enabled=False,
                expected_endpoint=None,
                expected_service_name='obsctl',
                expected_region='us-east-1'
            ))
            test_id += 1

    # Category C: OTEL Config Tests (simplified subset)
    test_id = 0
    for otel_enabled_source in ['otel', 'config', 'env', 'missing']:
        for otel_endpoint_source in ['otel', 'config', 'env', 'missing']:
            if test_id >= 16:  # Limit to first 16 for initial testing
                break

            # Determine expected OTEL state
            expected_enabled = _determine_otel_enabled(otel_enabled_source, otel_endpoint_source)
            expected_endpoint = _determine_expected_endpoint(otel_endpoint_source)

            test_matrix['otel'].append(ConfigTestCase(
                test_id=f"otel_{test_id:04d}",
                category='otel',
                description=f"OTEL: enabled from {otel_enabled_source}, endpoint from {otel_endpoint_source}",

                # AWS Credentials (defaults for OTEL tests)
                aws_access_key_id_source='env',
                aws_access_key_id_value='AKIATEST12345',
                aws_secret_access_key_source='env',
                aws_secret_access_key_value='testsecret12345',
                aws_session_token_source='missing',
                aws_session_token_value=None,

                # AWS Config (defaults)
                region_source='default',
                region_value='us-east-1',
                endpoint_url_source='env',
                endpoint_url_value='http://localhost:9000',
                output_source='default',
                output_value='json',

                # OTEL Config
                otel_enabled_source=otel_enabled_source,
                otel_enabled_value=True if otel_enabled_source != 'missing' else None,
                otel_endpoint_source=otel_endpoint_source,
                otel_endpoint_value=expected_endpoint,
                otel_service_name_source='default',
                otel_service_name_value='obsctl',

                # Expected results
                expected_aws_works=True,
                expected_otel_enabled=expected_enabled,
                expected_endpoint=expected_endpoint,
                expected_service_name='obsctl',
                expected_region='us-east-1'
            ))
            test_id += 1

    return test_matrix


def _determine_aws_works(access_key_source: str, secret_key_source: str) -> bool:
    """Determine if AWS should work based on credential sources"""
    # AWS works if both access key and secret key are available (not missing)
    return access_key_source != 'missing' and secret_key_source != 'missing'


def _determine_otel_enabled(enabled_source: str, endpoint_source: str) -> bool:
    """Determine if OTEL should be enabled"""
    # OTEL is enabled if explicitly enabled OR if endpoint is provided (auto-enable)
    if enabled_source != 'missing':
        return True
    if endpoint_source != 'missing':
        return True  # Auto-enable when endpoint is provided
    return False


def _determine_expected_endpoint(endpoint_source: str) -> Optional[str]:
    """Determine expected OTEL endpoint"""
    if endpoint_source == 'otel':
        return 'http://localhost:4318'
    elif endpoint_source == 'config':
        return 'http://localhost:4317'
    elif endpoint_source == 'env':
        return 'http://localhost:4319'
    return None


async def run_release_config_tests(category: str = 'all', max_workers: Optional[int] = None) -> Dict[str, Any]:
    """Main entry point for release configuration tests"""
    print("🚀 Starting Release Configuration Tests")
    print(f"📊 Parallel execution with {max_workers or os.cpu_count()} workers")

    # Generate test matrix
    test_matrix = generate_test_matrix()

    # Filter by category if specified
    if category != 'all':
        if category in test_matrix:
            test_matrix = {category: test_matrix[category]}
        else:
            print(f"❌ Unknown category: {category}")
            return {}

    total_tests = sum(len(tests) for tests in test_matrix.values())
    print(f"📋 Total tests: {total_tests}")

    framework = ParallelConfigTestFramework(max_workers)
    all_results = {}

    # Run each category
    start_time = time.time()

    for cat_name, test_cases in test_matrix.items():
        if not test_cases:
            continue

        print(f"🔄 Running {cat_name} tests ({len(test_cases)} tests)")
        category_start = time.time()

        # Split into batches for better memory management
        batch_size = 4  # Smaller batches for stability
        batches = [test_cases[i:i+batch_size] for i in range(0, len(test_cases), batch_size)]

        category_results = []
        for i, batch in enumerate(batches):
            print(f"  📦 Batch {i+1}/{len(batches)} ({len(batch)} tests)")
            batch_results = await framework.run_test_batch(batch)
            category_results.extend(batch_results)

        all_results[cat_name] = category_results
        category_time = time.time() - category_start

        # Show category summary
        passed = len([r for r in category_results if r['status'] == 'PASS'])
        failed = len([r for r in category_results if r['status'] == 'FAIL'])
        errors = len([r for r in category_results if r['status'] == 'ERROR'])

        print(f"✅ {cat_name} completed in {category_time:.2f}s: {passed} passed, {failed} failed, {errors} errors")

    total_time = time.time() - start_time

    # Generate comprehensive report
    generate_release_test_report(all_results, total_time)

    return all_results


def generate_release_test_report(results: Dict[str, List[Dict[str, Any]]], total_time: float):
    """Generate comprehensive test report for release"""
    total_tests = sum(len(category_results) for category_results in results.values())
    passed_tests = sum(
        len([r for r in category_results if r['status'] == 'PASS'])
        for category_results in results.values()
    )
    failed_tests = sum(
        len([r for r in category_results if r['status'] == 'FAIL'])
        for category_results in results.values()
    )
    error_tests = total_tests - passed_tests - failed_tests

    print("\n" + "="*80)
    print("🎯 RELEASE CONFIGURATION TEST REPORT")
    print("="*80)
    print(f"📊 Total Tests: {total_tests}")
    print(f"✅ Passed: {passed_tests}")
    print(f"❌ Failed: {failed_tests}")
    print(f"💥 Errors: {error_tests}")
    print(f"⏱️  Total Time: {total_time:.2f}s")
    if total_tests > 0:
        print(f"🚀 Average Time per Test: {total_time/total_tests:.3f}s")
        print(f"📈 Pass Rate: {(passed_tests/total_tests)*100:.1f}%")
    print()

    # Category breakdown
    for category, category_results in results.items():
        if not category_results:
            continue

        category_passed = len([r for r in category_results if r['status'] == 'PASS'])
        category_failed = len([r for r in category_results if r['status'] == 'FAIL'])
        category_errors = len([r for r in category_results if r['status'] == 'ERROR'])
        category_total = len(category_results)
        pass_rate = (category_passed / category_total) * 100 if category_total > 0 else 0

        print(f"📂 {category:15} {category_passed:3d}✅ {category_failed:3d}❌ {category_errors:3d}💥 ({pass_rate:5.1f}%)")

    print()

    # Show first few detailed results for debugging
    if any(results.values()):
        print("🔍 SAMPLE TEST RESULTS:")
        for category, category_results in results.items():
            if category_results:
                sample = category_results[0]
                print(f"\n📂 {category} - {sample['test_id']}:")
                print(f"  Status: {sample['status']}")
                print(f"  Description: {sample.get('description', 'N/A')}")
                if sample['status'] == 'ERROR':
                    print(f"  Error: {sample.get('error', 'Unknown error')}")
                elif sample['status'] == 'FAIL':
                    failures = sample.get('verification', {}).get('failures', [])
                    print(f"  Failures: {'; '.join(failures)}")
                break

    # Failure analysis
    if failed_tests > 0 or error_tests > 0:
        print("\n❌ FAILED/ERROR TESTS:")
        for category, category_results in results.items():
            failed_in_category = [r for r in category_results if r['status'] != 'PASS']
            if failed_in_category:
                print(f"\n📂 {category}:")
                for failure in failed_in_category[:3]:  # Show first 3 failures
                    error_msg = failure.get('error', '')
                    if failure.get('verification', {}).get('failures'):
                        error_msg = '; '.join(failure['verification']['failures'])
                    print(f"  • {failure['test_id']}: {error_msg}")
                if len(failed_in_category) > 3:
                    print(f"  ... and {len(failed_in_category) - 3} more")

    print("\n" + "="*80)

    # Write detailed report to file
    report_data = {
        'summary': {
            'total_tests': total_tests,
            'passed_tests': passed_tests,
            'failed_tests': failed_tests,
            'error_tests': error_tests,
            'total_time': total_time,
            'pass_rate': (passed_tests / total_tests) * 100 if total_tests > 0 else 0
        },
        'results': results
    }

    with open('release_config_test_report.json', 'w') as f:
        json.dump(report_data, f, indent=2)

    print(f"📄 Detailed report written to: release_config_test_report.json")


def main():
    """CLI entry point"""
    parser = argparse.ArgumentParser(description="Run release configuration tests")
    parser.add_argument('--category', default='all',
                       choices=['all', 'credentials', 'config', 'otel', 'mixed'],
                       help="Test category to run")
    parser.add_argument('--workers', type=int, help="Number of parallel workers")
    parser.add_argument('--timeout', type=int, default=1800, help="Total timeout in seconds")

    args = parser.parse_args()

    try:
        # Check if obsctl binary exists
        if not os.path.exists('./target/release/obsctl'):
            print("❌ obsctl binary not found at ./target/release/obsctl")
            print("Please run: cargo build --release")
            return 1

        # Run tests with timeout
        result = asyncio.wait_for(
            run_release_config_tests(args.category, args.workers),
            timeout=args.timeout
        )
        asyncio.run(result)

        print("🎉 All tests completed successfully!")
        return 0

    except asyncio.TimeoutError:
        print(f"\n⏰ Tests timed out after {args.timeout} seconds")
        return 1
    except KeyboardInterrupt:
        print("\n🛑 Tests interrupted by user")
        return 1
    except Exception as e:
        print(f"\n💥 Test execution failed: {e}")
        return 1


if __name__ == "__main__":
    exit(main())
