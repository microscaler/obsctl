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

* [ ] Update Rust to nightly (to support edition 2024)
* [ ] Replace `aws_smithy_client::timeout::TimeoutConfig` with `aws_smithy_types::timeout::TimeoutConfig`
* [ ] Update `Cargo.toml` with compatible versions for `aws-sdk-s3`, `aws-config`, etc.
* [ ] Ensure all dependencies are compatible with the latest Rust edition

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

* [ ] Dockerfile's export stage should use `COPY --from=builder /.../obsctl /obsctl`

### Task 9: Advanced Date Range Filtering

* [ ] Implement `--created-after` flag for ls command to filter buckets/objects by creation date
* [ ] Implement `--created-before` flag for ls command to filter buckets/objects by creation date
* [ ] Add `--created-between` flag accepting date range (e.g., "2024-01-01,2024-12-31")
* [ ] Support multiple date formats: ISO 8601, relative dates ("7d", "1w", "1m"), human-readable ("yesterday", "last week")
* [ ] Add `--modified-after`, `--modified-before`, `--modified-between` flags for object modification dates
* [ ] Implement date range validation and error handling
* [ ] Add comprehensive tests for date parsing and filtering logic
* [ ] Update documentation with date range filtering examples

### Task 10: Advanced Size Range Filtering

* [ ] Implement `--min-size` flag for ls command to filter objects by minimum size
* [ ] Implement `--max-size` flag for ls command to filter objects by maximum size
* [ ] Add `--size-range` flag accepting size range (e.g., "1MB,100MB" or "1048576,104857600")
* [ ] Support human-readable size units (B, KB, MB, GB, TB, PB) and binary units (KiB, MiB, GiB, etc.)
* [ ] Add size comparison operators (">=1MB", "<100MB", "=0B" for empty files)
* [ ] Implement size filtering for both bucket statistics and individual objects
* [ ] Add bucket-level size filtering (total bucket size, object count ranges)
* [ ] Add comprehensive tests for size parsing and filtering logic
* [ ] Update documentation with size range filtering examples

### Task 11: Combined Filtering Enhancement

* [ ] Allow combining date and size filters with wildcard patterns
* [ ] Implement efficient filtering order (patterns first, then API calls, then date/size filters)
* [ ] Add `--filter-summary` flag to show filtering statistics
* [ ] Implement `--sort-by` flag with options: name, size, date, type
* [ ] Add `--reverse` flag for reverse sorting
* [ ] Optimize filtering for large bucket listings with pagination
* [ ] Add filtering performance metrics to OpenTelemetry traces

---

## ✅ Completed

* [x] Renamed CLI binary from `upload_obs` → `obsctl`
* [x] Added OpenTelemetry support with retry and batching
* [x] Implemented systemd watchdog notifications
* [x] Implemented file descriptor safety check via `/proc`
* [x] Manpage and shell completion scripts
* [x] `.deb` packaging + Justfile + GitLab artifacts
* [x] **Advanced wildcard pattern support** - Implemented comprehensive glob pattern matching for bucket operations
  * [x] Added `--pattern` flag to `ls` and `rb` commands
  * [x] Support for `*`, `?`, `[abc]`, `[a-z]`, `[!abc]` pattern types
  * [x] Built robust wildcard matching engine in `utils.rs`
  * [x] Added safety confirmations for pattern-based bulk deletions
  * [x] Comprehensive test coverage for all pattern types
  * [x] Updated README with prominent wildcard functionality documentation
  * [x] Made OpenTelemetry feature built-in by default with selective usage
  * [x] Configured MinIO to bind to 0.0.0.0 for broader network access

---

## 🔁 Ongoing

* Docker registry name normalization
* CI bootstrapping from image builder
* Migrate `test` and `check` stages into Docker build itself

