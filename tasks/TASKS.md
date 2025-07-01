# Codex Task Log: Fix and Stabilize obsctl Codebase

This document tracks all outstanding and completed tasks required to bring the `obsctl` project to a compiling, test-passing, production-ready state.

---

## 🛠️ Outstanding Tasks

### Task 1: Fix duplicate imports

* [x] Remove duplicate `use aws_sdk_s3::operation::put_object::PutObjectError;`

### Task 2: Platform-specific logging

* [ ] Wrap `systemd_journal_logger` with `#[cfg(target_os = "linux")]`
* [ ] Fallback to `TermLogger` for non-Linux systems

### Task 3: Version compatibility fixes

* [ ] Update Rust to 1.78+ (to support edition 2024)
* [ ] Replace `aws_smithy_client::timeout::TimeoutConfig` with `aws_smithy_types::timeout::TimeoutConfig`
* [ ] Update `Cargo.toml` with compatible versions for `aws-sdk-s3`, `aws-config`, etc.

### Task 4: Resolve `?` mismatches

* [ ] Refactor `upload_file()` to return `anyhow::Result<()>`
* [ ] Replace `?` usages on `File::open()` and `read_to_end()` with `.context(...)` calls

### Task 5: Fix `.set_time_to_local()`

* [ ] Replace with `set_time_offset_to_local().unwrap()` in log config

### Task 6: Fix broken async return flows

* [ ] Ensure all `async fn` blocks that use `?` return `Result<...>`
* [ ] Add `Ok(())` at the end of main async block and each job runner

### Task 7: Simplify build logic

* [ ] Remove `cargo build` from GitLab if binary is copied from Docker container

### Task 8: Ensure Dockerfile produces `/obsctl`

* [ ] Dockerfile’s export stage should use `COPY --from=builder /.../obsctl /obsctl`

---

## ✅ Completed

* [x] Renamed CLI binary from `upload_obs` → `obsctl`
* [x] Added OpenTelemetry support with retry and batching
* [x] Implemented systemd watchdog notifications
* [x] Implemented file descriptor safety check via `/proc`
* [x] Manpage and shell completion scripts
* [x] `.deb` packaging + Justfile + GitLab artifacts

---

## 🔁 Ongoing

* Docker registry name normalization
* CI bootstrapping from image builder
* Migrate `test` and `check` stages into Docker build itself

