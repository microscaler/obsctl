# Documentation: `obsctl`

## Overview
`obsctl` is a Rust-based CLI tool to safely upload files to S3-compatible OBS with telemetry, systemd integration, and open file protection.

---

## Command Line Options

| Flag | Description |
|------|-------------|
| `--source <path>` | Source directory to upload |
| `--bucket <name>` | Target S3 bucket |
| `--prefix <key>` | Path prefix inside bucket |
| `--endpoint <url>` | S3-compatible endpoint (e.g. Cloud.ru OBS) |
| `--region <name>` | AWS region for signature (default: ru-moscow-1) |
| `--http-timeout <sec>` | Shared timeout for S3 and OTEL requests (default: 10) |
| `--max-concurrent <n>` | Number of parallel uploads (default: 4) |
| `--max-retries <n>` | Retries per file (default: 3) |
| `--debug <level>` | Logging level: trace, debug, info, warn, error |
| `--dry-run` | Simulate the upload without sending |

---

## Environment Variables

| Variable | Description |
|----------|-------------|
| `AWS_ACCESS_KEY_ID` | S3 access key (e.g. `tenant:key`) |
| `AWS_SECRET_ACCESS_KEY` | S3 secret key |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | HTTP endpoint to send OTEL telemetry traces |

---

## OTEL JSON Schema

### Per-file upload span:
```json
{
  "event": "upload_success",
  "file": "prefix/filename.txt",
  "timestamp": "2025-07-01T00:01:02Z"
}
```

### Final summary event:
```json
{
  "service": "obsctl",
  "status": "ok" | "failed",
  "files_total": 22,
  "files_failed": 2,
  "timestamp": "2025-07-01T00:01:30Z"
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
  opts=(--source --bucket --prefix --endpoint --region --http-timeout --max-concurrent --max-retries --debug --dry-run)
  _arguments "*: :->opts" && _values "flags" $opts
}
```

### Fish:
```fish
complete -c obsctl -l source -d 'Source directory'
complete -c obsctl -l bucket -d 'S3 bucket name'
complete -c obsctl -l prefix -d 'Key prefix'
complete -c obsctl -l endpoint -d 'OBS endpoint URL'
complete -c obsctl -l region -d 'AWS region'
complete -c obsctl -l http-timeout -d 'HTTP timeout (sec)'
complete -c obsctl -l max-concurrent -d 'Parallel uploads'
complete -c obsctl -l max-retries -d 'Retries per file'
complete -c obsctl -l debug -d 'Log verbosity'
complete -c obsctl -l dry-run -d 'Dry run mode'
```

