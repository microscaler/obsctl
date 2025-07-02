# obsctl Integration Testing Suite

A comprehensive, modular integration testing framework for obsctl with full OpenTelemetry observability support.

## 🎯 Quick Start

```bash
# Run comprehensive tests (default)
./tests/integration/run_tests.sh

# Run with verbose output
./tests/integration/run_tests.sh --verbose

# Run specific test type
./tests/integration/run_tests.sh basic
```

## 📁 Architecture

```
tests/integration/
├── run_tests.sh          # 🎯 MAIN ENTRY POINT
├── README.md            # This documentation
└── scripts/             # Modular test implementations
    ├── common.sh        # Shared utilities and functions
    ├── test_basic.sh    # Basic S3 operations
    ├── test_comprehensive.sh  # Full feature testing
    ├── test_performance.sh    # Performance benchmarks
    ├── test_observability.sh  # OTEL/metrics validation
    ├── test_concurrent.sh     # Concurrent operations
    └── test_error_handling.sh # Error scenarios
```

## 🚀 Usage

### Basic Usage

```bash
# Default comprehensive test
./tests/integration/run_tests.sh

# Show help
./tests/integration/run_tests.sh --help
```

### Test Types

| Test Type | Description | Use Case |
|-----------|-------------|----------|
| `basic` | Basic S3 operations (upload, download, list) | Quick validation |
| `comprehensive` | Full feature testing (default) | Complete validation |
| `performance` | Performance benchmarks and timing | Performance analysis |
| `observability` | OTEL metrics and tracing validation | Monitoring setup |
| `concurrent` | Concurrent operations testing | Stress testing |
| `error-handling` | Error scenarios and edge cases | Robustness testing |
| `all` | Run all test types sequentially | Full regression testing |

### Examples

```bash
# Run specific test types
./tests/integration/run_tests.sh basic --verbose
./tests/integration/run_tests.sh performance --no-cleanup
./tests/integration/run_tests.sh observability --otel true

# Run all tests with custom endpoint
./tests/integration/run_tests.sh all \
  --endpoint http://localhost:9000 \
  --region us-east-1

# Dry run to see what would be executed
./tests/integration/run_tests.sh --dry-run comprehensive

# Debug mode with no cleanup
./tests/integration/run_tests.sh basic \
  --verbose \
  --no-cleanup
```

## ⚙️ Configuration Options

### Command Line Arguments

| Option | Description | Default |
|--------|-------------|---------|
| `-e, --endpoint URL` | MinIO endpoint URL | `http://localhost:9000` |
| `-r, --region REGION` | AWS region | `us-east-1` |
| `-o, --otel BOOL` | Enable OpenTelemetry | `true` |
| `--no-otel` | Disable OpenTelemetry | - |
| `-c, --cleanup BOOL` | Cleanup after tests | `true` |
| `--no-cleanup` | Skip cleanup (for debugging) | - |
| `-v, --verbose` | Enable verbose output | `false` |
| `-n, --dry-run` | Show what would be executed | `false` |
| `-h, --help` | Show help message | - |

### Environment Variables

The test suite respects these environment variables:

```bash
# AWS Configuration
export AWS_ACCESS_KEY_ID="minioadmin"
export AWS_SECRET_ACCESS_KEY="minioadmin123"
export AWS_DEFAULT_REGION="us-east-1"

# OpenTelemetry Configuration
export OTEL_ENABLED="true"
export OTEL_EXPORTER_OTLP_ENDPOINT="http://localhost:4317"
export OTEL_SERVICE_NAME="obsctl-integration-test"
export OTEL_SERVICE_VERSION="0.1.0"
```

## 🔧 Prerequisites

### Required Infrastructure

1. **Docker Compose Stack** (must be running):
   ```bash
   docker compose up -d
   ```

2. **obsctl Binary** (with OTEL features):
   ```bash
   cargo build --features otel
   ```

### Infrastructure Services

| Service | URL | Credentials |
|---------|-----|-------------|
| MinIO | http://localhost:9000 | minioadmin/minioadmin123 |
| MinIO Console | http://localhost:9001 | minioadmin/minioadmin123 |
| Grafana | http://localhost:3000 | admin/admin |
| Prometheus | http://localhost:9090 | - |
| Jaeger | http://localhost:16686 | - |
| OTEL Collector | localhost:4317 | - |

### Verification

```bash
# Check if infrastructure is ready
curl -s http://localhost:9000 >/dev/null && echo "✅ MinIO ready"
curl -s http://localhost:3000/api/health >/dev/null && echo "✅ Grafana ready"
curl -s http://localhost:9090/-/ready >/dev/null && echo "✅ Prometheus ready"
```

## 📊 Test Descriptions

### Basic Tests (`test_basic.sh`)
- **Duration**: ~30 seconds
- **Operations**: Upload, download, list, metadata
- **Files**: Small text files (KB range)
- **Use Case**: Quick smoke testing

### Comprehensive Tests (`test_comprehensive.sh`)
- **Duration**: ~2-5 minutes
- **Operations**: All obsctl commands
- **Files**: Various sizes (KB to MB)
- **Features**: Recursive operations, sync, presigned URLs
- **Use Case**: Full regression testing

### Performance Tests (`test_performance.sh`)
- **Duration**: ~1-3 minutes
- **Focus**: Timing measurements, throughput
- **Files**: Large files (MB to GB range)
- **Metrics**: Upload/download speeds, latency
- **Use Case**: Performance benchmarking

### Observability Tests (`test_observability.sh`)
- **Duration**: ~1-2 minutes
- **Focus**: OTEL metrics and traces
- **Validation**: Prometheus metrics, Jaeger traces
- **Dashboard**: Grafana dashboard verification
- **Use Case**: Monitoring setup validation

### Concurrent Tests (`test_concurrent.sh`)
- **Duration**: ~1-2 minutes
- **Focus**: Parallel operations
- **Operations**: Multiple simultaneous uploads/downloads
- **Use Case**: Stress testing, race condition detection

### Error Handling Tests (`test_error_handling.sh`)
- **Duration**: ~30 seconds
- **Focus**: Error scenarios and edge cases
- **Scenarios**: Invalid buckets, missing files, network errors
- **Use Case**: Robustness validation

## 🔍 Observability Integration

### OpenTelemetry Metrics

When OTEL is enabled, tests generate metrics visible in:

**Prometheus Metrics** (http://localhost:9090):
```
obsctl_operations_total
obsctl_bytes_uploaded_total
obsctl_files_uploaded_total
obsctl_operation_duration_seconds
obsctl_transfer_rate_kbps
obsctl_files_small_total
obsctl_files_medium_total
obsctl_files_large_total
```

**Grafana Dashboard** (http://localhost:3000):
- Unified obsctl dashboard with real-time metrics
- Operations overview and performance panels
- Error tracking and bucket analytics

**Jaeger Traces** (http://localhost:16686):
- Distributed tracing for each operation
- Service name: `obsctl-integration-test`
- Detailed operation timing and dependencies

### Metrics Validation

The observability tests automatically verify:
- ✅ Metrics appear in Prometheus
- ✅ Grafana dashboard loads correctly
- ✅ Traces are generated in Jaeger
- ✅ OTEL collector is receiving data

## 🐛 Troubleshooting

### Common Issues

#### 1. MinIO Not Running
```bash
Error: MinIO not accessible at http://localhost:9000
```
**Solution**: Start Docker Compose stack
```bash
docker compose up -d
```

#### 2. obsctl Binary Missing
```bash
Error: obsctl binary not found at ./target/debug/obsctl
```
**Solution**: Build with OTEL features
```bash
cargo build --features otel
```

#### 3. Permission Denied
```bash
Error: AWS credentials not configured
```
**Solution**: Check environment variables
```bash
export AWS_ACCESS_KEY_ID="minioadmin"
export AWS_SECRET_ACCESS_KEY="minioadmin123"
```

#### 4. OTEL Not Working
```bash
Warning: No metrics found in Prometheus
```
**Solution**: Verify OTEL collector is running
```bash
docker logs obsctl-otel-collector
```

### Debug Mode

For debugging test failures:

```bash
# Run with verbose output and no cleanup
./tests/integration/run_tests.sh basic \
  --verbose \
  --no-cleanup

# Check what files remain after test
ls -la /tmp/obsctl-test-*

# Dry run to see commands
./tests/integration/run_tests.sh --dry-run comprehensive
```

### Log Analysis

Test logs include:
- **Operation timing** for performance analysis
- **File checksums** for integrity verification
- **Error details** for debugging failures
- **OTEL status** for observability validation

## 📈 Performance Expectations

### Typical Performance (Local MinIO)

| Operation | File Size | Expected Time | Throughput |
|-----------|-----------|---------------|------------|
| Upload | 1KB | <10ms | - |
| Upload | 1MB | <100ms | >10 MB/s |
| Upload | 10MB | <1s | >10 MB/s |
| Download | 1KB | <10ms | - |
| Download | 1MB | <50ms | >20 MB/s |
| List | 100 objects | <100ms | - |
| Metadata | Single object | <10ms | - |

### Performance Factors

- **Network latency** to MinIO endpoint
- **Disk I/O** for local file operations
- **OTEL overhead** (~1-5% additional time)
- **Concurrent operations** may reduce individual throughput

## 🔄 Continuous Integration

### CI/CD Integration

```yaml
# Example GitHub Actions workflow
- name: Run Integration Tests
  run: |
    docker compose up -d
    sleep 30  # Wait for services
    cargo build --features otel
    ./tests/integration/run_tests.sh all --no-cleanup
    
- name: Collect Test Artifacts
  if: failure()
  run: |
    docker logs obsctl-minio > minio.log
    docker logs obsctl-otel-collector > otel.log
```

### Test Matrix

For comprehensive CI testing:

```bash
# Quick validation
./tests/integration/run_tests.sh basic

# Full regression (nightly)
./tests/integration/run_tests.sh all

# Performance benchmarking (weekly)
./tests/integration/run_tests.sh performance --verbose
```

## 🤝 Contributing

### Adding New Tests

1. **Create new test script** in `scripts/` directory:
   ```bash
   cp scripts/test_basic.sh scripts/test_myfeature.sh
   ```

2. **Implement test function**:
   ```bash
   run_myfeature_tests() {
       print_step "Testing my feature"
       # Test implementation
       print_success "My feature tests completed"
   }
   ```

3. **Update main runner** in `run_tests.sh`:
   - Add to `required_scripts` array
   - Add case in `run_test_type` function
   - Update help text

### Test Guidelines

- **Use descriptive names** for test functions
- **Include timing measurements** for performance-sensitive operations
- **Verify file integrity** with checksums
- **Clean up resources** in test cleanup
- **Add verbose logging** for debugging
- **Handle errors gracefully** with proper exit codes

## 📚 Related Documentation

- [obsctl README](../../README.md) - Main project documentation
- [OTEL Migration Guide](../../tasks/OTEL_SDK_MIGRATION.md) - OpenTelemetry implementation
- [Docker Compose Setup](../../docker-compose.yml) - Infrastructure configuration
- [Grafana Dashboards](../../.docker/grafana/dashboards/) - Observability dashboards

## 🏆 Success Criteria

A successful test run should show:
- ✅ All operations complete without errors
- ✅ File integrity verified with checksums
- ✅ Performance within expected ranges
- ✅ OTEL metrics flowing to Prometheus
- ✅ Grafana dashboard displaying data
- ✅ Jaeger traces captured
- ✅ Clean resource cleanup

---

**Happy Testing!** 🚀

For issues or questions, check the troubleshooting section above or review the test logs with `--verbose` mode. 