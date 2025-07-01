# Operator Manual: upload\_obs

This document provides operational and deployment guidance for managing the `obsctl` utility in production environments.

---

## Purpose

`obsctl` reliably transfers files from a local directory to a remote S3-compatible object store. It is optimized for safety, auditability, and integration with systemd + OpenTelemetry.

---

## Installation

### Build from source:

```sh
cargo build --release
sudo cp target/release/obsctl /usr/local/bin/
```

### System dependencies:

* Linux with `/proc`
* systemd (for service watchdog integration)
* Environment variables for AWS credentials

---

## Running Manually

```sh
export AWS_ACCESS_KEY_ID="014f1de034145f:XXXXXXX"
export AWS_SECRET_ACCESS_KEY="YYYYYYY"
export OTEL_EXPORTER_OTLP_ENDPOINT="https://otel.example.com/otlp"

obsctl \
  --source /var/uploads/export \
  --bucket backup-target \
  --prefix daily/ \
  --endpoint https://obs.example.com \
  --region ru-moscow-1 \
  --http-timeout 10 \
  --max-concurrent 4 \
  --debug info
```

---

## Safety Mechanisms

| Feature              | Purpose                                                                |
| -------------------- | ---------------------------------------------------------------------- |
| **FD Check**         | Skips files with open file descriptors (detected via `/proc/<pid>/fd`) |
| **Timestamp Delay**  | Skips files modified in the last 2 seconds                             |
| **Retry Logic**      | Retries failed uploads with exponential backoff                        |
| **Systemd Watchdog** | Emits `READY` and `STOPPING` states                                    |

---

## 🩺 Health + Logging

### Journald:

```
journalctl -u obsctl.service
```

### Systemd Sample Unit:

```ini
[Unit]
Description=Upload daily backups to OBS
After=network-online.target

[Service]
ExecStart=/usr/local/bin/obsctl --source /data/export --bucket logs --endpoint https://obs.example.com --prefix nightly/
Environment=AWS_ACCESS_KEY_ID=...
Environment=AWS_SECRET_ACCESS_KEY=...
Environment=OTEL_EXPORTER_OTLP_ENDPOINT=https://otel.local/otlp
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

### Timer:

```ini
[Timer]
OnCalendar=*-*-* 00:01:00
Persistent=true
Unit=obsctl.service
```

---

## Telemetry (OTEL)

* File-level `upload_success` events
* Final summary event
* Retries up to 3 times if OTEL send fails

Set with:

```sh
export OTEL_EXPORTER_OTLP_ENDPOINT=https://otel-receiver.domain.com/v1/traces
```

---

## Exit Codes

* `0` = all uploads succeeded or were safely skipped
* `1` = one or more uploads failed after max retries

---

## Systemd Integration

### Service Unit File

Create a systemd service unit at `/etc/systemd/system/obsctl.service`:

```ini
[Unit]
Description=Upload directory to Cloud.ru OBS
Wants=network-online.target
After=network-online.target

[Service]
Type=oneshot
ExecStart=/usr/local/bin/obsctl \
  --source /var/data/export \
  --bucket backup-bucket \
  --endpoint https://obs.ru-moscow-1.hc.sbercloud.ru \
  --prefix daily/ \
  --region ru-moscow-1 \
  --http-timeout 10 \
  --max-concurrent 4 \
  --debug info

Environment=AWS_ACCESS_KEY_ID=tenant:key
Environment=AWS_SECRET_ACCESS_KEY=secret
Environment=OTEL_EXPORTER_OTLP_ENDPOINT=https://otel.example.com/otlp

StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

### Timer Unit File

Create a timer unit at `/etc/systemd/system/obsctl.timer`:

```ini
[Unit]
Description=Trigger OBS uploader daily

[Timer]
OnCalendar=*-*-* 00:01:00
Persistent=true
Unit=obsctl.service

[Install]
WantedBy=timers.target
```

### Enable and Start

```sh
sudo systemctl daemon-reexec
sudo systemctl enable --now obsctl.timer
```

### Status Checks

```sh
systemctl status obsctl.service
journalctl -u obsctl.service --since "1 hour ago"
```

## 🧪 Testing

```sh
cargo test
```

Includes:

* File open/FD detection
* CLI parsing
* Key generation correctness

---

## 🛠️ Maintenance

* Rotate logs via systemd or journald config
* Monitor OTEL receiver to ensure traces arrive
* Upgrade with `cargo install --path . --force`

---

## 📝 Authors

* Developed by Charles Sibbald
