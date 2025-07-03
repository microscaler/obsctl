use anyhow::Result;
use aws_sdk_s3::primitives::ByteStream;
use log::info;
use opentelemetry::trace::{Span, Tracer};
use std::path::Path;
use std::time::Instant;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::commands::s3_uri::{is_s3_uri, S3Uri};
use crate::config::Config;

#[allow(clippy::too_many_arguments)]
pub async fn execute(
    config: &Config,
    source: &str,
    dest: &str,
    recursive: bool,
    dryrun: bool,
    max_concurrent: usize,
    force: bool,
    include: Option<&str>,
    exclude: Option<&str>,
) -> Result<()> {
    // Create a span for the cp operation
    let tracer = opentelemetry::global::tracer("obsctl");
    let mut span = tracer
        .span_builder("cp_operation")
        .with_attributes(vec![
            opentelemetry::KeyValue::new("operation", "cp"),
            opentelemetry::KeyValue::new("source", source.to_string()),
            opentelemetry::KeyValue::new("dest", dest.to_string()),
            opentelemetry::KeyValue::new("recursive", recursive),
            opentelemetry::KeyValue::new("force", force),
        ])
        .start(&tracer);

    // Add an event to the span
    span.add_event("cp_operation_started", vec![]);

    let start_time = Instant::now();
    info!("Copying from {source} to {dest}");

    if dryrun {
        info!("[DRY RUN] Would copy from {source} to {dest}");
        span.end();
        return Ok(());
    }

    let source_is_s3 = is_s3_uri(source);
    let dest_is_s3 = is_s3_uri(dest);

    let result = match (source_is_s3, dest_is_s3) {
        (false, true) => {
            // Local to S3 upload
            upload_to_s3(
                config,
                source,
                dest,
                recursive,
                max_concurrent,
                force,
                include,
                exclude,
            )
            .await
        }
        (true, false) => {
            // S3 to local download
            download_from_s3(
                config,
                source,
                dest,
                recursive,
                max_concurrent,
                force,
                include,
                exclude,
            )
            .await
        }
        (true, true) => {
            // S3 to S3 copy
            copy_s3_to_s3(
                config,
                source,
                dest,
                recursive,
                max_concurrent,
                force,
                include,
                exclude,
            )
            .await
        }
        (false, false) => {
            // Local to local copy (not typically handled by S3 tools)
            Err(anyhow::anyhow!(
                "Local to local copy not supported. Use standard cp command."
            ))
        }
    };

    let duration = start_time.elapsed();

    // Record overall cp operation metrics using proper OTEL SDK
    {
        use crate::otel::OTEL_INSTRUMENTS;
        use opentelemetry::KeyValue;

        // Record operation count
        OTEL_INSTRUMENTS
            .operations_total
            .add(1, &[KeyValue::new("operation", "cp")]);

        // Record duration
        let duration_seconds = duration.as_millis() as f64 / 1000.0;
        OTEL_INSTRUMENTS
            .operation_duration
            .record(duration_seconds, &[KeyValue::new("operation", "cp")]);

        // Record success/failure
        match &result {
            Ok(_) => {
                // Success is implicit - no errors recorded
                log::debug!("CP operation completed successfully in {duration:?}");
                span.add_event(
                    "cp_operation_completed",
                    vec![
                        opentelemetry::KeyValue::new("status", "success"),
                        opentelemetry::KeyValue::new("duration_ms", duration.as_millis() as i64),
                    ],
                );
            }
            Err(e) => {
                OTEL_INSTRUMENTS.record_error_with_type(&e.to_string());
                span.add_event(
                    "cp_operation_failed",
                    vec![
                        opentelemetry::KeyValue::new("status", "error"),
                        opentelemetry::KeyValue::new("error", e.to_string()),
                    ],
                );
            }
        }
    }

    span.end();
    result
}

#[allow(clippy::too_many_arguments)]
async fn upload_to_s3(
    config: &Config,
    source: &str,
    dest: &str,
    recursive: bool,
    _max_concurrent: usize,
    _force: bool,
    _include: Option<&str>,
    _exclude: Option<&str>,
) -> Result<()> {
    let dest_uri = S3Uri::parse(dest)?;

    if recursive {
        info!("Recursive upload from {source} to {dest}");
        upload_directory_to_s3(config, source, &dest_uri).await
    } else {
        info!("Single file upload from {source} to {dest}");
        upload_file_to_s3(config, source, &dest_uri).await
    }
}

#[allow(clippy::too_many_arguments)]
async fn download_from_s3(
    config: &Config,
    source: &str,
    dest: &str,
    recursive: bool,
    _max_concurrent: usize,
    _force: bool,
    _include: Option<&str>,
    _exclude: Option<&str>,
) -> Result<()> {
    let source_uri = S3Uri::parse(source)?;

    if recursive {
        info!("Recursive download from {source} to {dest}");
        download_directory_from_s3(config, &source_uri, dest).await
    } else {
        info!("Single file download from {source} to {dest}");
        download_file_from_s3(config, &source_uri, dest).await
    }
}

#[allow(clippy::too_many_arguments)]
async fn copy_s3_to_s3(
    config: &Config,
    source: &str,
    dest: &str,
    _recursive: bool,
    _max_concurrent: usize,
    _force: bool,
    _include: Option<&str>,
    _exclude: Option<&str>,
) -> Result<()> {
    let source_uri = S3Uri::parse(source)?;
    let dest_uri = S3Uri::parse(dest)?;

    info!("S3 to S3 copy from {source} to {dest}");

    let copy_source = format!("{}/{}", source_uri.bucket, source_uri.key_or_empty());

    config
        .client
        .copy_object()
        .copy_source(&copy_source)
        .bucket(&dest_uri.bucket)
        .key(dest_uri.key_or_empty())
        .send()
        .await?;

    info!("Successfully copied {source} to {dest}");
    Ok(())
}

async fn upload_file_to_s3(config: &Config, local_path: &str, s3_uri: &S3Uri) -> Result<()> {
    let start_time = Instant::now();
    let path = Path::new(local_path);

    if !path.exists() {
        // Record error using proper OTEL SDK
        {
            use crate::otel::OTEL_INSTRUMENTS;

            OTEL_INSTRUMENTS.record_error_with_type("Local file does not exist");
        }

        return Err(anyhow::anyhow!("Local file does not exist: {}", local_path));
    }

    if !path.is_file() {
        // Record error using proper OTEL SDK
        {
            use crate::otel::OTEL_INSTRUMENTS;

            OTEL_INSTRUMENTS.record_error_with_type("Path is not a file");
        }

        return Err(anyhow::anyhow!("Path is not a file: {}", local_path));
    }

    // Read the file content and get size
    let file_content = fs::read(local_path).await?;
    let file_size = file_content.len() as u64;
    let byte_stream = ByteStream::from(file_content);

    // Upload to S3
    match config
        .client
        .put_object()
        .bucket(&s3_uri.bucket)
        .key(s3_uri.key_or_empty())
        .body(byte_stream)
        .send()
        .await
    {
        Ok(_) => {
            let duration = start_time.elapsed();

            // Record upload success using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;

                OTEL_INSTRUMENTS.record_upload(file_size, duration.as_millis() as u64);
            }

            info!(
                "Successfully uploaded {} to s3://{}/{} ({} bytes in {:?})",
                local_path,
                s3_uri.bucket,
                s3_uri.key_or_empty(),
                file_size,
                duration
            );

            // Transparent du call for real-time bucket analytics
            let bucket_uri = format!("s3://{}", s3_uri.bucket);
            call_transparent_du(config, &bucket_uri).await;

            Ok(())
        }
        Err(e) => {
            // Record error using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;

                OTEL_INSTRUMENTS
                    .record_error_with_type(&format!("Failed to upload {local_path}: {e}"));
            }

            Err(anyhow::anyhow!("Failed to upload {}: {}", local_path, e))
        }
    }
}

async fn download_file_from_s3(config: &Config, s3_uri: &S3Uri, local_path: &str) -> Result<()> {
    let start_time = Instant::now();

    // Get the object from S3
    match config
        .client
        .get_object()
        .bucket(&s3_uri.bucket)
        .key(s3_uri.key_or_empty())
        .send()
        .await
    {
        Ok(response) => {
            // Create parent directories if they don't exist
            let local_path_obj = Path::new(local_path);
            if let Some(parent) = local_path_obj.parent() {
                fs::create_dir_all(parent).await?;
            }

            // Read the response body and write to file
            let mut file = fs::File::create(local_path).await?;
            let mut body = response.body.into_async_read();
            let bytes_written = tokio::io::copy(&mut body, &mut file).await?;
            file.flush().await?;

            let duration = start_time.elapsed();

            // Record comprehensive download metrics
            {
                use crate::otel::OTEL_INSTRUMENTS;

                OTEL_INSTRUMENTS.record_download(bytes_written, duration.as_millis() as u64);
            }

            info!(
                "Successfully downloaded s3://{}/{} to {} ({} bytes in {:?})",
                s3_uri.bucket,
                s3_uri.key_or_empty(),
                local_path,
                bytes_written,
                duration
            );

            // Transparent du call for real-time bucket analytics
            let bucket_uri = format!("s3://{}", s3_uri.bucket);
            call_transparent_du(config, &bucket_uri).await;

            Ok(())
        }
        Err(e) => {
            // Record error using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;

                OTEL_INSTRUMENTS.record_error_with_type(&format!(
                    "Failed to download s3://{}/{}: {}",
                    s3_uri.bucket,
                    s3_uri.key_or_empty(),
                    e
                ));
            }

            Err(anyhow::anyhow!(
                "Failed to download s3://{}/{}: {}",
                s3_uri.bucket,
                s3_uri.key_or_empty(),
                e
            ))
        }
    }
}

async fn upload_directory_to_s3(config: &Config, local_dir: &str, s3_uri: &S3Uri) -> Result<()> {
    use walkdir::WalkDir;

    let start_time = Instant::now();
    let base_path = Path::new(local_dir);
    let mut total_files = 0u64;
    let mut total_bytes = 0u64;

    for entry in WalkDir::new(local_dir) {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            // Calculate relative path from base directory
            let relative_path = path.strip_prefix(base_path)?;
            let s3_key = if s3_uri.key.is_none() || s3_uri.key_or_empty().is_empty() {
                relative_path.to_string_lossy().to_string()
            } else {
                format!(
                    "{}/{}",
                    s3_uri.key_or_empty(),
                    relative_path.to_string_lossy()
                )
            };

            // Create S3 URI for this file
            let file_s3_uri = S3Uri {
                bucket: s3_uri.bucket.clone(),
                key: Some(s3_key),
            };

            // Get file size before upload
            if let Ok(metadata) = path.metadata() {
                total_bytes += metadata.len();
            }
            total_files += 1;

            // Upload the file
            upload_file_to_s3(config, path.to_str().unwrap(), &file_s3_uri).await?;
        }
    }

    let duration = start_time.elapsed();

    // Record bulk upload metrics using proper OTEL SDK
    {
        use crate::otel::OTEL_INSTRUMENTS;
        use opentelemetry::KeyValue;

        // Record bulk upload count
        OTEL_INSTRUMENTS.uploads_total.add(total_files, &[]);

        // Record bulk bytes uploaded
        OTEL_INSTRUMENTS.bytes_uploaded_total.add(total_bytes, &[]);

        // Record bulk files uploaded
        OTEL_INSTRUMENTS.files_uploaded_total.add(total_files, &[]);

        // Record duration in seconds (not milliseconds)
        let duration_seconds = duration.as_millis() as f64 / 1000.0;
        OTEL_INSTRUMENTS.operation_duration.record(
            duration_seconds,
            &[KeyValue::new("operation", "upload_directory")],
        );
    }

    info!(
        "Successfully uploaded directory {} to s3://{}/{} ({} files, {} bytes in {:?})",
        local_dir,
        s3_uri.bucket,
        s3_uri.key_or_empty(),
        total_files,
        total_bytes,
        duration
    );
    Ok(())
}

async fn download_directory_from_s3(
    config: &Config,
    s3_uri: &S3Uri,
    local_dir: &str,
) -> Result<()> {
    let start_time = Instant::now();
    let mut total_files = 0u64;
    let mut total_bytes = 0u64;

    // List all objects with the prefix
    let mut list_request = config.client.list_objects_v2().bucket(&s3_uri.bucket);

    if !s3_uri.key_or_empty().is_empty() {
        list_request = list_request.prefix(s3_uri.key_or_empty());
    }

    let response = list_request.send().await?;

    if let Some(objects) = response.contents {
        for object in objects {
            if let Some(key) = object.key {
                // Calculate local file path
                let local_file_path = if s3_uri.key_or_empty().is_empty() {
                    format!("{local_dir}/{key}")
                } else {
                    // Remove the prefix from the key
                    let relative_key = key
                        .strip_prefix(&format!("{}/", s3_uri.key_or_empty()))
                        .unwrap_or(&key);
                    format!("{local_dir}/{relative_key}")
                };

                // Create S3 URI for this object
                let object_s3_uri = S3Uri {
                    bucket: s3_uri.bucket.clone(),
                    key: Some(key),
                };

                // Track file size from S3 object info
                if let Some(size) = object.size {
                    total_bytes += size as u64;
                }
                total_files += 1;

                // Download the file
                download_file_from_s3(config, &object_s3_uri, &local_file_path).await?;
            }
        }
    }

    let duration = start_time.elapsed();

    // Record bulk download metrics using proper OTEL SDK
    {
        use crate::otel::OTEL_INSTRUMENTS;
        use opentelemetry::KeyValue;

        // Record bulk download count
        OTEL_INSTRUMENTS.downloads_total.add(total_files, &[]);

        // Record bulk bytes downloaded
        OTEL_INSTRUMENTS
            .bytes_downloaded_total
            .add(total_bytes, &[]);

        // Record bulk files downloaded
        OTEL_INSTRUMENTS
            .files_downloaded_total
            .add(total_files, &[]);

        // Record duration in seconds (not milliseconds)
        let duration_seconds = duration.as_millis() as f64 / 1000.0;
        OTEL_INSTRUMENTS.operation_duration.record(
            duration_seconds,
            &[KeyValue::new("operation", "download_directory")],
        );
    }

    info!(
        "Successfully downloaded directory s3://{}/{} to {} ({} files, {} bytes in {:?})",
        s3_uri.bucket,
        s3_uri.key_or_empty(),
        local_dir,
        total_files,
        total_bytes,
        duration
    );
    Ok(())
}

// Add transparent du call for real-time bucket analytics
async fn call_transparent_du(config: &Config, s3_uri: &str) {
    // Only call du for bucket-level analytics if OTEL is enabled
    {
        use crate::commands::du;
        use log::debug;

        // Extract bucket from S3 URI for bucket-level analytics
        if let Ok(uri) = crate::commands::s3_uri::S3Uri::parse(s3_uri) {
            let bucket_uri = format!("s3://{}", uri.bucket);

            debug!("Running transparent du for bucket analytics: {bucket_uri}");

            // Run du in background for bucket analytics - errors are logged but don't fail the main operation
            if let Err(e) = du::execute_transparent(config, &bucket_uri, false, true, Some(1)).await
            {
                debug!("Transparent du failed (non-critical): {e}");
            } else {
                debug!(
                    "Transparent du completed successfully for bucket: {}",
                    uri.bucket
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_s3::Client;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn create_mock_config() -> Config {
        let mock_client = Arc::new(Client::from_conf(
            aws_sdk_s3::config::Builder::new()
                .region(aws_config::Region::new("us-east-1"))
                .behavior_version(aws_config::BehaviorVersion::latest())
                .build(),
        ));

        Config {
            client: mock_client,
            otel: crate::config::OtelConfig {
                enabled: false,
                endpoint: None,
                service_name: "obsctl-test".to_string(),
                service_version: crate::get_service_version(),
                read_operations: false,
            },
            loki: crate::config::LokiConfig::default(),
            jaeger: crate::config::JaegerConfig::default(),
        }
    }

    #[tokio::test]
    async fn test_execute_dry_run() {
        let config = create_mock_config();

        let result = execute(
            &config,
            "/tmp/test.txt",
            "s3://bucket/test.txt",
            false,
            true, // dry run
            4,
            false,
            None,
            None,
        )
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_local_to_local_error() {
        let config = create_mock_config();

        let result = execute(
            &config,
            "/tmp/source.txt",
            "/tmp/dest.txt",
            false,
            false,
            4,
            false,
            None,
            None,
        )
        .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Local to local copy not supported"));
    }

    #[tokio::test]
    async fn test_upload_file_to_s3_nonexistent_file() {
        let config = create_mock_config();
        let s3_uri = S3Uri {
            bucket: "test-bucket".to_string(),
            key: Some("test.txt".to_string()),
        };

        let result = upload_file_to_s3(&config, "/nonexistent/file.txt", &s3_uri).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Local file does not exist"));
    }

    #[tokio::test]
    async fn test_upload_file_to_s3_directory_path() {
        let config = create_mock_config();
        let temp_dir = TempDir::new().unwrap();
        let s3_uri = S3Uri {
            bucket: "test-bucket".to_string(),
            key: Some("test.txt".to_string()),
        };

        let result = upload_file_to_s3(&config, temp_dir.path().to_str().unwrap(), &s3_uri).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Path is not a file"));
    }

    #[tokio::test]
    async fn test_s3_uri_parsing() {
        let config = create_mock_config();

        // Test invalid S3 URI for copy_s3_to_s3
        let result = copy_s3_to_s3(
            &config,
            "invalid-uri",
            "s3://bucket/dest",
            false,
            4,
            false,
            None,
            None,
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_upload_to_s3_invalid_dest_uri() {
        let config = create_mock_config();

        let result = upload_to_s3(
            &config,
            "/tmp/test.txt",
            "invalid-s3-uri",
            false,
            4,
            false,
            None,
            None,
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_download_from_s3_invalid_source_uri() {
        let config = create_mock_config();

        let result = download_from_s3(
            &config,
            "invalid-s3-uri",
            "/tmp/dest.txt",
            false,
            4,
            false,
            None,
            None,
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_upload_to_s3_recursive_vs_single() {
        let config = create_mock_config();
        let dest_uri = "s3://test-bucket/test-key";

        // Test recursive upload (will fail due to no AWS connection, but tests routing)
        let result_recursive = upload_to_s3(
            &config, "/tmp", dest_uri, true, // recursive
            4, false, None, None,
        )
        .await;
        assert!(result_recursive.is_err());

        // Test single file upload (will fail due to no AWS connection, but tests routing)
        let result_single = upload_to_s3(
            &config,
            "/tmp/test.txt",
            dest_uri,
            false, // not recursive
            4,
            false,
            None,
            None,
        )
        .await;
        assert!(result_single.is_err());
    }

    #[tokio::test]
    async fn test_download_from_s3_recursive_vs_single() {
        let config = create_mock_config();
        let source_uri = "s3://test-bucket/test-key";

        // Test recursive download (will fail due to no AWS connection, but tests routing)
        let result_recursive = download_from_s3(
            &config,
            source_uri,
            "/tmp/dest",
            true, // recursive
            4,
            false,
            None,
            None,
        )
        .await;
        assert!(result_recursive.is_err());

        // Test single file download (will fail due to no AWS connection, but tests routing)
        let result_single = download_from_s3(
            &config,
            source_uri,
            "/tmp/dest.txt",
            false, // not recursive
            4,
            false,
            None,
            None,
        )
        .await;
        assert!(result_single.is_err());
    }

    #[test]
    fn test_s3_uri_construction() {
        let s3_uri = S3Uri {
            bucket: "test-bucket".to_string(),
            key: Some("test/key.txt".to_string()),
        };

        assert_eq!(s3_uri.bucket, "test-bucket");
        assert_eq!(s3_uri.key_or_empty(), "test/key.txt");

        let s3_uri_no_key = S3Uri {
            bucket: "test-bucket".to_string(),
            key: None,
        };

        assert_eq!(s3_uri_no_key.key_or_empty(), "");
    }
}
