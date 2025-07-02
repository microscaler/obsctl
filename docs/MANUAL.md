# Operator Manual: obsctl

This document provides comprehensive operational and deployment guidance for managing the `obsctl` utility in production environments across **any S3-compatible storage provider**.

---

## Purpose

`obsctl` is an AWS CLI-compliant S3-compatible storage management tool that provides comprehensive bucket and object operations for **any S3-compatible storage service**. Originally designed to solve specific challenges with Cloud.ru OBS, it now supports **AWS S3, MinIO, Ceph, DigitalOcean Spaces, Wasabi, Backblaze B2, and any S3-compatible storage** with advanced features optimized for safety, auditability, and integration with systemd + OpenTelemetry in production environments.

## Supported S3-Compatible Providers

| Provider | Status | Common Use Cases | Endpoint Pattern |
|----------|--------|------------------|------------------|
| **AWS S3** | ✅ Fully Supported | Production cloud storage | `s3.amazonaws.com` |
| **Cloud.ru OBS** | ✅ Fully Supported | Russian cloud services | `obs.ru-moscow-1.hc.sbercloud.ru` |
| **MinIO** | ✅ Fully Supported | Development, testing, private cloud | `localhost:9000` |
| **Ceph RadosGW** | ✅ Fully Supported | Self-hosted object storage | `ceph.example.com` |
| **DigitalOcean Spaces** | ✅ Fully Supported | Simple cloud storage | `nyc3.digitaloceanspaces.com` |
| **Wasabi** | ✅ Fully Supported | Hot cloud storage | `s3.wasabisys.com` |
| **Backblaze B2** | ✅ Fully Supported | Backup and archival | `s3.us-west-000.backblazeb2.com` |

---

## Installation

### Build from Source

```bash
# Clone and build
git clone <repository-url>
cd obsctl
cargo build --release

# Install system-wide
sudo cp target/release/obsctl /usr/local/bin/
sudo chmod +x /usr/local/bin/obsctl
```

### System Dependencies

- **Linux with `/proc`** (for file descriptor checking)
- **systemd** (for service integration)
- **Network connectivity** to S3-compatible endpoints
- **Rust toolchain** (for building from source)

---

## AWS CLI Compliant Commands

### Object Operations

#### List Objects (`ls`)
```bash
# List bucket contents (any S3 provider)
obsctl ls s3://my-bucket/

# List with wildcard patterns (unique to obsctl)
obsctl ls --pattern "*-prod"                    # Production buckets
obsctl ls --pattern "user-[0-9]-*"             # Numbered user buckets

# List with details
obsctl ls s3://my-bucket/path/ --long --human-readable

# Recursive listing
obsctl ls s3://my-bucket/ --recursive
```

#### Copy Objects (`cp`)
```bash
# Upload file (any S3 provider)
obsctl cp ./local-file.txt s3://bucket/remote-file.txt

# Download file
obsctl cp s3://bucket/remote-file.txt ./local-file.txt

# Copy between S3 locations
obsctl cp s3://source-bucket/file s3://dest-bucket/file

# Recursive operations
obsctl cp ./local-dir s3://bucket/remote-dir/ --recursive

# With filtering
obsctl cp ./logs s3://bucket/logs/ --recursive \
  --include "*.log" --exclude "*.tmp"
```

#### Synchronize Directories (`sync`)
```bash
# Basic sync (any S3 provider)
obsctl sync ./local-dir s3://bucket/remote-dir/

# Sync with deletion
obsctl sync ./local-dir s3://bucket/remote-dir/ --delete

# Dry run mode
obsctl sync ./local-dir s3://bucket/remote-dir/ --dryrun
```

#### Remove Objects (`rm`)
```bash
# Remove single object
obsctl rm s3://bucket/file.txt

# Remove recursively
obsctl rm s3://bucket/path/ --recursive

# Dry run mode
obsctl rm s3://bucket/old-data/ --recursive --dryrun
```

### Bucket Operations with Pattern Support

#### Create Bucket (`mb`)
```bash
obsctl mb s3://new-bucket-name
```

#### Remove Bucket (`rb`)
```bash
# Remove empty bucket
obsctl rb s3://empty-bucket

# Force remove (deletes all objects first)
obsctl rb s3://bucket-with-objects --force

# Pattern-based bulk removal (unique to obsctl)
obsctl rb --pattern "test-*" --confirm         # Delete all test buckets
obsctl rb --pattern "temp-[0-9]*" --confirm    # Delete numbered temp buckets
```

### Utility Operations

#### Generate Presigned URLs (`presign`)
```bash
# Default 1 hour expiration
obsctl presign s3://bucket/file.txt

# Custom expiration
obsctl presign s3://bucket/file.txt --expires-in 7200
```

#### Object Metadata (`head-object`)
```bash
obsctl head-object --bucket my-bucket --key path/to/file.txt
```

#### Storage Usage (`du`)
```bash
# Show storage usage
obsctl du s3://bucket/path/

# Human readable format
obsctl du s3://bucket/ --human-readable --summarize
```

---

## Production Configuration

### Environment Variables (Universal)

```bash
# AWS Credentials (Required for any S3 provider)
export AWS_ACCESS_KEY_ID="your-access-key"
export AWS_SECRET_ACCESS_KEY="your-secret-key"

# Provider-specific endpoint (Optional)
export AWS_ENDPOINT_URL="https://your-s3-provider.com"

# Region (Optional, defaults to us-east-1)
export AWS_DEFAULT_REGION="us-east-1"

# Observability (Optional)
export OTEL_EXPORTER_OTLP_ENDPOINT="https://otel-receiver.example.com/v1/traces"
export OTEL_SERVICE_NAME="obsctl"

# AWS SDK Logging (Optional)
export AWS_LOG_LEVEL="info"
export AWS_SMITHY_LOG="info"
```

### Global Configuration Options

```bash
obsctl [GLOBAL_OPTIONS] <COMMAND> [COMMAND_OPTIONS]

Global Options:
  --debug <LEVEL>         Set log verbosity [default: info]
                         Values: trace, debug, info, warn, error
  -e, --endpoint <URL>    Custom S3 endpoint URL (any S3-compatible provider)
  -r, --region <REGION>   AWS region [default: us-east-1]
  --timeout <SECONDS>     HTTP timeout [default: 10]
  -h, --help             Print help
  -V, --version          Print version
```

### Provider-Specific Configuration Examples

#### AWS S3
```bash
# Default configuration (no endpoint needed)
obsctl cp ./data s3://my-bucket/backup/ --recursive
```

#### Cloud.ru OBS (Original Use Case)
```bash
# Cloud.ru OBS specific settings
obsctl cp ./data s3://my-bucket/backup/ \
  --endpoint https://obs.ru-moscow-1.hc.sbercloud.ru \
  --region ru-moscow-1 \
  --recursive \
  --max-concurrent 8
```

#### MinIO (Development/Testing)
```bash
# MinIO local development
obsctl cp ./data s3://my-bucket/backup/ \
  --endpoint http://localhost:9000 \
  --region us-east-1 \
  --recursive
```

#### DigitalOcean Spaces
```bash
# DigitalOcean Spaces configuration
obsctl cp ./data s3://my-space/backup/ \
  --endpoint https://nyc3.digitaloceanspaces.com \
  --region nyc3 \
  --recursive
```

#### Wasabi
```bash
# Wasabi hot cloud storage
obsctl cp ./data s3://my-bucket/backup/ \
  --endpoint https://s3.wasabisys.com \
  --region us-east-1 \
  --recursive
```

#### Backblaze B2
```bash
# Backblaze B2 configuration
obsctl cp ./data s3://my-bucket/backup/ \
  --endpoint https://s3.us-west-000.backblazeb2.com \
  --region us-west-000 \
  --recursive
```

---

## Systemd Integration

### Service Unit File (Multi-Provider)

Create `/etc/systemd/system/obsctl-backup.service`:

```ini
[Unit]
Description=Daily backup to S3-compatible storage
Wants=network-online.target
After=network-online.target

[Service]
Type=oneshot
ExecStart=/usr/local/bin/obsctl sync /var/backups/daily s3://backup-bucket/daily/ --delete
Environment=AWS_ACCESS_KEY_ID=your-access-key
Environment=AWS_SECRET_ACCESS_KEY=your-secret-key
Environment=AWS_ENDPOINT_URL=https://your-s3-provider.com
Environment=AWS_DEFAULT_REGION=us-east-1
Environment=OTEL_EXPORTER_OTLP_ENDPOINT=https://otel.example.com/otlp
Environment=OTEL_SERVICE_NAME=obsctl-backup
StandardOutput=journal
StandardError=journal
User=backup
Group=backup

[Install]
WantedBy=multi-user.target
```

### Timer Unit File

Create `/etc/systemd/system/obsctl-backup.timer`:

```ini
[Unit]
Description=Trigger daily S3-compatible storage backup
Requires=obsctl-backup.service

[Timer]
OnCalendar=*-*-* 02:00:00
Persistent=true
Unit=obsctl-backup.service

[Install]
WantedBy=timers.target
```

### Enable and Manage

```bash
# Reload systemd
sudo systemctl daemon-reload

# Enable timer
sudo systemctl enable --now obsctl-backup.timer

# Check status
systemctl status obsctl-backup.timer
systemctl status obsctl-backup.service

# View logs
journalctl -u obsctl-backup.service --since "24 hours ago"

# Manual execution
sudo systemctl start obsctl-backup.service
```

---

## Safety Mechanisms

| Feature | Purpose | Implementation |
|---------|---------|---------------|
| **File Descriptor Check** | Prevents uploading files being written | Scans `/proc/<pid>/fd/` for open handles |
| **Modification Window** | Avoids race conditions | Skips files modified within 2 seconds |
| **Dry Run Mode** | Test operations safely | `--dryrun` flag for all destructive operations |
| **Retry Logic** | Handle transient failures | Exponential backoff with configurable limits |
| **Atomic Operations** | Ensure data consistency | Uses S3 multipart uploads for large files |
| **Pattern Confirmations** | Prevent accidental bulk deletions | `--confirm` flag for pattern-based operations |
| **Systemd Integration** | Service lifecycle management | `READY`/`STOPPING` notifications |

---

## Monitoring and Observability

### OpenTelemetry Integration

```bash
# Enable OTEL tracing (any S3 provider)
export OTEL_EXPORTER_OTLP_ENDPOINT="https://otel-collector.example.com/v1/traces"
export OTEL_SERVICE_NAME="obsctl"

# Operations emit structured traces with provider information
obsctl cp ./large-dataset s3://data-bucket/dataset/ --recursive
```

### Log Analysis

```bash
# View recent operations
journalctl -u obsctl-backup.service --since "1 hour ago"

# Filter by log level
journalctl -u obsctl-backup.service | grep "ERROR\|WARN"

# Follow live logs
journalctl -u obsctl-backup.service -f
```

### Health Checks

```bash
# Test connectivity (any S3 provider)
obsctl ls s3://test-bucket/ --debug debug

# Validate credentials
obsctl head-object --bucket test-bucket --key test-file

# Performance testing
time obsctl du s3://large-bucket/ --summarize
```

---

## Performance Tuning

### Concurrency Settings

```bash
# High-throughput uploads (any S3 provider)
obsctl cp ./data s3://bucket/data/ \
  --recursive \
  --max-concurrent 16

# Bandwidth-limited environments
obsctl sync ./data s3://bucket/data/ \
  --max-concurrent 2
```

### Network Optimization

```bash
# Increase timeout for slow connections
obsctl cp large-file.zip s3://bucket/files/ \
  --timeout 300

# Regional endpoint optimization (provider-specific)
obsctl cp ./data s3://bucket/data/ \
  --endpoint https://region.your-provider.com \
  --region your-region
```

---

## Troubleshooting

### Common Issues

#### Authentication Errors
```bash
# Verify credentials (any S3 provider)
echo $AWS_ACCESS_KEY_ID
echo $AWS_SECRET_ACCESS_KEY
echo $AWS_ENDPOINT_URL

# Test with verbose logging
obsctl ls s3://test-bucket/ --debug trace
```

#### Network Connectivity
```bash
# Test endpoint connectivity (provider-specific)
curl -I https://your-s3-provider.com

# Check DNS resolution
nslookup your-s3-provider.com
```

#### File Permission Issues
```bash
# Check file permissions
ls -la /path/to/files/

# Verify process can read files
sudo -u backup obsctl ls s3://test-bucket/
```

### Exit Codes

| Code | Description |
|------|-------------|
| `0` | Success - all operations completed |
| `1` | Failure - one or more operations failed |

### Debug Mode

```bash
# Maximum verbosity (any S3 provider)
obsctl cp ./data s3://bucket/data/ --debug trace

# AWS SDK debugging
export AWS_LOG_LEVEL=debug
export AWS_SMITHY_LOG=debug
obsctl ls s3://bucket/
```

---

## Security Best Practices

### Credential Management

```bash
# Use environment files (any S3 provider)
sudo mkdir -p /etc/obsctl
sudo tee /etc/obsctl/credentials << EOF
AWS_ACCESS_KEY_ID=your-access-key
AWS_SECRET_ACCESS_KEY=your-secret-key
AWS_ENDPOINT_URL=https://your-s3-provider.com
EOF
sudo chmod 600 /etc/obsctl/credentials

# Reference in systemd unit
EnvironmentFile=/etc/obsctl/credentials
```

### Network Security

```bash
# Use HTTPS endpoints only (any S3 provider)
obsctl cp ./data s3://bucket/data/ \
  --endpoint https://secure.your-provider.com

# Validate SSL certificates (default behavior)
```

### Access Control

```bash
# Run as dedicated user
sudo useradd -r -s /bin/false obsctl-user
sudo systemctl edit obsctl-backup.service
# Add: User=obsctl-user
```

---

## Maintenance

### Log Rotation

```bash
# Configure journald retention
sudo tee /etc/systemd/journald.conf.d/obsctl.conf << EOF
[Journal]
SystemMaxUse=1G
SystemMaxFileSize=100M
MaxRetentionSec=30day
EOF

sudo systemctl restart systemd-journald
```

### Updates

```bash
# Update from source
cd /path/to/obsctl
git pull
cargo build --release
sudo cp target/release/obsctl /usr/local/bin/
sudo systemctl restart obsctl-backup.service
```

### Backup Verification

```bash
# Verify backup integrity (any S3 provider)
obsctl ls s3://backup-bucket/daily/ --long

# Compare local and remote
obsctl sync /var/backups/daily s3://backup-bucket/daily/ --dryrun
```

---

## Testing

### Unit Tests

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Test specific modules
cargo test s3_uri
cargo test commands
```

### Integration Testing

```bash
# Test with any S3-compatible endpoint
export AWS_ACCESS_KEY_ID="test-key"
export AWS_SECRET_ACCESS_KEY="test-secret"
export AWS_ENDPOINT_URL="https://your-test-provider.com"

# Create test bucket
obsctl mb s3://test-bucket-$(date +%s)

# Run operations
obsctl cp README.md s3://test-bucket-$(date +%s)/test-file
obsctl ls s3://test-bucket-$(date +%s)/
obsctl rm s3://test-bucket-$(date +%s)/test-file
obsctl rb s3://test-bucket-$(date +%s)
```

---

## 📝 Authors

- Developed by Charles Sibbald
- Contributions welcome via GitHub
