# Complete Configuration Integration Test Variations

## Overview

This document outlines the comprehensive test matrix for **ALL configuration handling** in obsctl, including AWS credentials, AWS config, and OTEL configuration. These tests are **ONLY run during release management** due to their extensive scope.

**Total Estimated Tests: 2,000+ test cases**
**Execution: Parallel execution for efficiency**
**Trigger: Release pipeline only**

## Configuration Variables & Sources

### AWS Credentials Variables:
- `aws_access_key_id` (string/missing)
- `aws_secret_access_key` (string/missing) 
- `aws_session_token` (string/missing)

### AWS Config Variables:
- `region` (string/missing)
- `endpoint_url` (URL/missing)
- `output` (json/text/table/missing)

### OTEL Variables:
- `otel_enabled` (true/false/missing)
- `otel_endpoint` (URL/missing)
- `otel_service_name` (string/missing)

### Configuration Sources (in priority order):
1. **Environment Variables** (highest priority)
2. **CLI Arguments** (--endpoint, --region)
3. **`~/.aws/credentials` file** (credentials)
4. **`~/.aws/config` file** (config + OTEL)
5. **`~/.aws/otel` file** (dedicated OTEL)
6. **Default values** (lowest priority)

## Mathematical Calculation

### Base Variables:
- **9 total variables** (3 credentials + 3 config + 3 OTEL)
- **6 sources each** = **6⁹ = 10,077,696 theoretical combinations**

### Realistic Constraints:
- **Credentials** only from credentials file, config file, or env
- **Config** only from config file, CLI args, or env  
- **OTEL** only from otel file, config file, or env
- **Reduced to ~2,000 practical test cases**

## Complete Test Matrix Categories

### Category A: AWS Credentials Tests (216 tests)
Testing every combination of credential sources:

| access_key_id | secret_access_key | session_token | Expected Behavior |
|---------------|-------------------|---------------|-------------------|
| credentials file | credentials file | credentials file | Use credentials file |
| credentials file | config file | missing | Mixed source credentials |
| env var | credentials file | config file | Env overrides for access key |
| missing | credentials file | missing | Partial credentials (should fail) |
| env var | env var | env var | Full env credentials |

*3 sources × 3 sources × 3 sources × 8 value combinations = 216 tests*

### Category B: AWS Config Tests (192 tests)
Testing every combination of config sources:

| region | endpoint_url | output | Expected Behavior |
|--------|--------------|--------|-------------------|
| CLI arg | CLI arg | config file | CLI overrides region/endpoint |
| env var | config file | missing | Mixed sources |
| config file | env var | env var | Env overrides endpoint |
| missing | CLI arg | config file | Partial config |
| default | missing | missing | Use defaults |

*4 sources × 4 sources × 4 sources × 3 value combinations = 192 tests*

### Category C: OTEL Config Tests (216 tests)  
Testing every combination of OTEL sources:

| otel_enabled | otel_endpoint | otel_service_name | Expected Behavior |
|--------------|---------------|-------------------|-------------------|
| otel file | otel file | otel file | Pure otel file config |
| otel file | config file | env var | Mixed OTEL sources |
| env var | otel file | config file | Env overrides enabled |
| missing | otel file | missing | Auto-enable from endpoint |
| config file | env var | missing | Config + env mix |

*4 sources × 4 sources × 4 sources × 3 value combinations = 216 tests*

### Category D: Cross-Configuration Dependencies (144 tests)
Testing how different config types interact:

| AWS Credentials | AWS Config | OTEL Config | Expected Behavior |
|-----------------|------------|-------------|-------------------|
| Valid | Valid | Enabled | Full functionality |
| Valid | Invalid endpoint | Enabled | OTEL works, AWS fails |
| Invalid | Valid | Enabled | AWS fails, OTEL works |
| Missing | Valid | Enabled | AWS fails, OTEL works |
| Valid | Valid | Disabled | AWS works, no OTEL |
| Partial | Partial | Partial | Complex failure modes |

*6 credential states × 6 config states × 4 OTEL states = 144 tests*

### Category E: Profile-Specific Tests (288 tests)
Different AWS profiles affecting all configurations:

| AWS_PROFILE | Credentials Profile | Config Profile | OTEL in Profile | Expected |
|-------------|-------------------|----------------|-----------------|----------|
| default | [default] in credentials | [default] in config | otel_enabled=true | Use default profile |
| prod | [prod] in credentials | [profile prod] in config | otel_enabled=false | Use prod profile |
| dev | Missing from credentials | [profile dev] in config | Missing OTEL | Partial profile |
| staging | [staging] in credentials | Missing from config | otel_enabled=true | Mixed profile sources |

*6 profiles × 6 credential configs × 8 config variations = 288 tests*

## Release Management Integration

### When Tests Run:
- ✅ **Release candidate builds**
- ✅ **Pre-release validation**
- ✅ **Major version releases**
- ❌ **NOT on every commit** (too expensive)
- ❌ **NOT on feature branches** (use subset)

### Parallel Execution Strategy:
- **Test categories run in parallel**
- **Individual tests within categories run in parallel**
- **Isolated test environments** (separate temp directories)
- **Resource pooling** for efficiency

### Performance Targets:
- **Total execution time: < 30 minutes** (with parallelization)
- **Individual test: < 5 seconds**
- **Memory usage: < 2GB total**
- **CPU utilization: All available cores**

## Implementation Strategy

### Python Framework for Parallel Execution
```python
import asyncio
import tempfile
import os
import subprocess
from concurrent.futures import ProcessPoolExecutor
from dataclasses import dataclass
from typing import Optional, Dict, Any, List

@dataclass
class ConfigTestCase:
    test_id: str
    category: str
    
    # AWS Credentials
    aws_access_key_id_source: str
    aws_secret_access_key_source: str
    aws_session_token_source: str
    
    # AWS Config
    region_source: str
    endpoint_url_source: str
    output_source: str
    
    # OTEL Config
    otel_enabled_source: str
    otel_endpoint_source: str
    otel_service_name_source: str
    
    # Expected Results
    expected_aws_works: bool
    expected_otel_enabled: bool
    expected_endpoint: Optional[str]
    expected_service_name: Optional[str]

class ParallelConfigTestFramework:
    def __init__(self, max_workers: int = None):
        self.max_workers = max_workers or os.cpu_count()
        self.executor = ProcessPoolExecutor(max_workers=self.max_workers)
        
    async def run_test_batch(self, test_cases: List[ConfigTestCase]) -> List[Dict[str, Any]]:
        """Run a batch of tests in parallel"""
        loop = asyncio.get_event_loop()
        
        # Submit all tests to process pool
        futures = []
        for test_case in test_cases:
            future = loop.run_in_executor(
                self.executor, 
                self.run_single_test, 
                test_case
            )
            futures.append(future)
        
        # Wait for all tests to complete
        results = await asyncio.gather(*futures, return_exceptions=True)
        return results
    
    def run_single_test(self, test_case: ConfigTestCase) -> Dict[str, Any]:
        """Run a single test in isolated environment"""
        test_env = IsolatedTestEnvironment(test_case.test_id)
        
        try:
            test_env.setup(test_case)
            result = test_env.execute_obsctl_test()
            verification = test_env.verify_expectations(test_case, result)
            
            return {
                'test_id': test_case.test_id,
                'category': test_case.category,
                'status': 'PASS' if verification['success'] else 'FAIL',
                'result': result,
                'verification': verification,
                'execution_time': test_env.execution_time
            }
        except Exception as e:
            return {
                'test_id': test_case.test_id,
                'category': test_case.category,
                'status': 'ERROR',
                'error': str(e),
                'execution_time': test_env.execution_time
            }
        finally:
            test_env.cleanup()

# Main execution for release tests
async def run_release_config_tests():
    """Main entry point for release configuration tests"""
    print("🚀 Starting Release Configuration Tests")
    print(f"📊 Parallel execution with {os.cpu_count()} workers")
    
    # Generate all test cases
    test_matrix = generate_test_matrix()
    total_tests = sum(len(tests) for tests in test_matrix.values())
    print(f"📋 Total tests: {total_tests}")
    
    framework = ParallelConfigTestFramework()
    all_results = {}
    
    # Run each category in parallel
    start_time = time.time()
    
    for category, test_cases in test_matrix.items():
        print(f"🔄 Running {category} tests ({len(test_cases)} tests)")
        category_start = time.time()
        
        # Split into batches for better memory management
        batch_size = 50
        batches = [test_cases[i:i+batch_size] for i in range(0, len(test_cases), batch_size)]
        
        category_results = []
        for batch in batches:
            batch_results = await framework.run_test_batch(batch)
            category_results.extend(batch_results)
        
        all_results[category] = category_results
        category_time = time.time() - category_start
        print(f"✅ {category} completed in {category_time:.2f}s")
    
    total_time = time.time() - start_time
    
    # Generate comprehensive report
    generate_release_test_report(all_results, total_time)
    
    framework.executor.shutdown(wait=True)
    return all_results
```

### GitHub Actions Integration
```yaml
name: Release Configuration Tests

on:
  push:
    tags: ['v*']
  workflow_dispatch:
    inputs:
      category:
        description: 'Test category to run'
        required: false
        type: choice
        options:
          - all
          - credentials
          - config
          - otel
          - profiles
          - edge_cases

jobs:
  config-tests:
    runs-on: ubuntu-latest
    timeout-minutes: 45
    
    strategy:
      matrix:
        category: [credentials, config, otel, dependencies, profiles, filesystem, environment, real_world, edge_cases]
      fail-fast: false
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        
      - name: Build obsctl
        run: cargo build --release
        
      - name: Setup Python
        uses: actions/setup-python@v4
        with:
          python-version: '3.11'
          
      - name: Install Python dependencies
        run: |
          pip install pytest pytest-asyncio pytest-xdist
          
      - name: Run Configuration Tests
        run: |
          python tests/release_config_tests.py --category ${{ matrix.category }} --workers 4
          
      - name: Upload Test Results
        uses: actions/upload-artifact@v3
        if: always()
        with:
          name: config-test-results-${{ matrix.category }}
          path: release_config_test_report.json
```

## Success Criteria for Release

### All 2,000+ tests must pass with:
- ✅ **100% pass rate** (zero tolerance for config failures)
- ✅ **< 30 minutes total execution time**
- ✅ **< 2GB memory usage**
- ✅ **Proper parallel execution**
- ✅ **Comprehensive error reporting**

### Release Blocking Conditions:
- ❌ **Any credential resolution failure**
- ❌ **Any OTEL configuration regression**
- ❌ **Any profile handling issue**
- ❌ **Performance degradation > 20%**

**This comprehensive test suite ensures bulletproof configuration handling for every obsctl release.** 