# obsctl Traffic Generator

## Advanced Concurrent Traffic Generator

This directory contains the advanced Python-based traffic generator for obsctl that simulates realistic multi-user S3 operations.

### Features

- **10 Concurrent User Threads**: Each user runs independently with their own behavior patterns
- **Realistic User Profiles**: Different users with specific file type preferences and activity patterns
- **High-Volume Traffic**: 25-2000 operations per minute with peak/off-peak cycles
- **TTL-Based Cleanup**: Automatic file lifecycle management (3 hours regular, 60 minutes large files)
- **Business Metrics**: Comprehensive OpenTelemetry metrics for monitoring

### User Profiles

| User | Role | Bucket | File Types | Peak Hours |
|------|------|--------|------------|------------|
| alice-dev | Software Developer | alice-development | Code, Documents | 9-17 UTC |
| bob-marketing | Marketing Manager | bob-marketing-assets | Images, Media | 8-16 UTC |
| carol-data | Data Scientist | carol-analytics | Documents, Archives | 10-18 UTC |
| david-backup | IT Admin | david-backups | Archives, Documents | 22-6 UTC |
| eve-design | Creative Designer | eve-creative-work | Images, Media | 9-17 UTC |
| frank-research | Research Scientist | frank-research-data | Documents, Code | 8-20 UTC |
| grace-sales | Sales Manager | grace-sales-materials | Documents, Images | 7-15 UTC |
| henry-ops | DevOps Engineer | henry-operations | Code, Archives | 24/7 |
| iris-content | Content Manager | iris-content-library | Images, Documents | 8-16 UTC |
| jack-mobile | Mobile Developer | jack-mobile-apps | Code, Images | 10-18 UTC |

### Usage

#### Prerequisites

1. **MinIO Running**: Ensure MinIO is running with proper resource allocation
   ```bash
   docker compose up -d minio
   ```

2. **AWS Credentials Configured**: MinIO credentials must be set up
   ```bash
   # ~/.aws/credentials should contain:
   [default]
   aws_access_key_id = minioadmin
   aws_secret_access_key = minioadmin123
   
   # ~/.aws/config should contain:
   [default]
   region = us-east-1
   endpoint_url = http://localhost:9000
   s3 =
     addressing_style = virtual
     preferred_transfer_client = classic
   ```

3. **obsctl Binary Built**: Ensure obsctl is compiled
   ```bash
   cargo build --release
   ```

#### Running the Traffic Generator

**Direct Python Execution** (Recommended):
```bash
python3 scripts/generate_traffic.py
```

**Key Features:**
- Runs for 24 hours by default
- 10 concurrent user threads
- Real-time statistics every 5 minutes
- Graceful shutdown with Ctrl+C
- Comprehensive logging to `traffic_generator.log`

#### Monitoring

- **Logs**: `tail -f traffic_generator.log`
- **Grafana Dashboard**: http://localhost:3000 (admin/admin)
- **Prometheus Metrics**: http://localhost:9090
- **Jaeger Tracing**: http://localhost:16686

#### Configuration

Key settings in `generate_traffic.py`:

```python
SCRIPT_DURATION_HOURS = 24          # Total runtime
MAX_CONCURRENT_USERS = 10           # Thread pool size
TTL_CONFIG = {
    'regular_files_hours': 3,        # TTL for files < 100MB
    'large_files_minutes': 60,       # TTL for files > 100MB
    'large_file_threshold_mb': 100
}
```

### Traffic Patterns

The generator creates realistic traffic with:

- **Variable Activity**: Users have different activity multipliers and peak hours
- **File Type Distribution**: Each user prefers specific file types based on their role
- **Size Variation**: From small 1KB files to large 1GB+ datasets
- **Operation Mix**: 80% uploads, 20% downloads
- **Error Simulation**: Realistic error rates for comprehensive testing

### Example Output

```
2025-07-02 00:26:52,956 - INFO - [alice-dev] Uploaded alice-dev_documents_1751405212.pdf (55911 bytes)
2025-07-02 00:26:53,311 - INFO - [henry-ops] Uploaded henry-ops_code_1751405212.html (91840 bytes)
2025-07-02 00:26:53,365 - INFO - [frank-research] Uploaded frank-research_documents_1751405212.txt (3795 bytes)
2025-07-02 00:26:53,608 - INFO - [eve-design] Uploaded eve-design_images_1751405212.svg (35670 bytes)
```

This shows true concurrent operation with multiple users uploading simultaneously! 