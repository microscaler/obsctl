# Documentation: `obsctl`

## Overview
`obsctl` is a comprehensive Rust-based CLI tool for **any S3-compatible object storage** with advanced features including wildcard pattern matching, telemetry, systemd integration, and production-grade safety features. Originally designed for Cloud.ru OBS, it now supports **AWS S3, MinIO, Ceph, DigitalOcean Spaces, Wasabi, Backblaze B2, and any S3-compatible storage**.

## Supported S3-Compatible Providers

| Provider | Status | Common Use Cases |
|----------|--------|------------------|
| **AWS S3** | ✅ Fully Supported | Production cloud storage |
| **Cloud.ru OBS** | ✅ Fully Supported | Russian cloud services |
| **MinIO** | ✅ Fully Supported | Development, testing, private cloud |
| **Ceph RadosGW** | ✅ Fully Supported | Self-hosted object storage |
| **DigitalOcean Spaces** | ✅ Fully Supported | Simple cloud storage |
| **Wasabi** | ✅ Fully Supported | Hot cloud storage |
| **Backblaze B2** | ✅ Fully Supported | Backup and archival |

---

## AWS CLI Compatible Commands

| Command | Description | AWS Equivalent |
|---------|-------------|----------------|
| `obsctl ls` | List objects/buckets with pattern support | `aws s3 ls` |
| `obsctl cp` | Copy files/objects | `aws s3 cp` |
| `obsctl sync` | Sync directories | `aws s3 sync` |
| `obsctl rm` | Remove objects | `aws s3 rm` |
| `obsctl mb` | Create buckets | `aws s3 mb` |
| `obsctl rb` | Remove buckets with patterns | `aws s3 rb` |
| `obsctl presign` | Generate presigned URLs | `aws s3 presign` |
| `obsctl head-object` | Show object metadata | `aws s3api head-object` |
| `obsctl du` | Storage usage statistics | Custom extension |

---

## Command Line Options

### Global Options
| Flag | Description |
|------|-------------|
| `--debug <level>` | Logging level: trace, debug, info, warn, error |
| `--endpoint <url>` | S3-compatible endpoint (any provider) |
| `--region <name>` | AWS region (default: us-east-1) |
| `--timeout <sec>` | HTTP timeout (default: 10) |

### Pattern Matching Options
| Flag | Description |
|------|-------------|
| `--pattern <glob>` | Wildcard pattern for bucket operations |
| `--confirm` | Confirm pattern-based bulk operations |

### Transfer Options
| Flag | Description |
|------|-------------|
| `--recursive` | Recursive operation |
| `--max-concurrent <n>` | Parallel operations (default: 4) |
| `--max-retries <n>` | Retries per operation (default: 3) |
| `--dry-run` | Simulate without executing |

---

## Environment Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `AWS_ACCESS_KEY_ID` | S3 access key | `AKIAIOSFODNN7EXAMPLE` |
| `AWS_SECRET_ACCESS_KEY` | S3 secret key | `wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY` |
| `AWS_ENDPOINT_URL` | S3 endpoint URL | `https://s3.wasabisys.com` |
| `AWS_DEFAULT_REGION` | Default region | `us-east-1` |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | OTEL telemetry endpoint | `http://localhost:4317` |
| `OTEL_SERVICE_NAME` | Service name for telemetry | `obsctl` |

---

## Provider-Specific Examples

### AWS S3
```bash
obsctl cp ./data s3://my-bucket/data/ --recursive
```

### Cloud.ru OBS (Original Use Case)
```bash
obsctl cp ./data s3://my-bucket/data/ \
  --endpoint https://obs.ru-moscow-1.hc.sbercloud.ru \
  --region ru-moscow-1 \
  --recursive
```

### MinIO (Development/Testing)
```bash
obsctl cp ./data s3://my-bucket/data/ \
  --endpoint http://localhost:9000 \
  --region us-east-1 \
  --recursive
```

### DigitalOcean Spaces
```bash
obsctl cp ./data s3://my-space/data/ \
  --endpoint https://nyc3.digitaloceanspaces.com \
  --region nyc3 \
  --recursive
```

### Wasabi
```bash
obsctl cp ./data s3://my-bucket/data/ \
  --endpoint https://s3.wasabisys.com \
  --region us-east-1 \
  --recursive
```

---

## OTEL JSON Schema

### Per-operation span:
```json
{
  "event": "operation_success",
  "operation": "cp|sync|ls|rm|mb|rb",
  "source": "local/path/or/s3://bucket/key",
  "destination": "s3://bucket/key/or/local/path",
  "timestamp": "2025-07-01T00:01:02Z",
  "provider": "aws|cloudru|minio|wasabi|etc"
}
```

### Final summary event:
```json
{
  "service": "obsctl",
  "status": "ok" | "failed",
  "operations_total": 22,
  "operations_failed": 2,
  "bytes_transferred": 1048576,
  "timestamp": "2025-07-01T00:01:30Z",
  "provider": "aws|cloudru|minio|wasabi|etc"
}
```

---

## Shell Completion

### Bash:
Installed via `packaging/obsctl.bash-completion`

### Zsh:
Add to `~/.zshrc`:
```zsh
compdef _obsctl obsctl
_obsctl() {
  local -a opts
  opts=(--debug --endpoint --region --timeout --pattern --confirm --recursive --max-concurrent --max-retries --dry-run)
  _arguments "*: :->opts" && _values "flags" $opts
}
```

### Fish:
```fish
complete -c obsctl -l debug -d 'Log verbosity'
complete -c obsctl -l endpoint -d 'S3-compatible endpoint URL'
complete -c obsctl -l region -d 'AWS region'
complete -c obsctl -l timeout -d 'HTTP timeout (sec)'
complete -c obsctl -l pattern -d 'Wildcard pattern'
complete -c obsctl -l confirm -d 'Confirm bulk operations'
complete -c obsctl -l recursive -d 'Recursive operation'
complete -c obsctl -l max-concurrent -d 'Parallel operations'
complete -c obsctl -l max-retries -d 'Retries per operation'
complete -c obsctl -l dry-run -d 'Dry run mode'
```

