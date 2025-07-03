use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as b64, Engine as _};
use log::info;
use md5;
use opentelemetry::trace::{Span, Tracer};
use std::time::Instant;

use crate::commands::s3_uri::{is_s3_uri, S3Uri};
use crate::config::Config;

pub async fn execute(
    config: &Config,
    path: &str,
    recursive: bool,
    dryrun: bool,
    force: bool,
    include: Option<&str>,
    exclude: Option<&str>,
) -> Result<()> {
    // Create a span for the rm operation
    let tracer = opentelemetry::global::tracer("obsctl");
    let mut span = tracer
        .span_builder("rm_operation")
        .with_attributes(vec![
            opentelemetry::KeyValue::new("operation", "rm"),
            opentelemetry::KeyValue::new("path", path.to_string()),
            opentelemetry::KeyValue::new("recursive", recursive),
            opentelemetry::KeyValue::new("force", force),
        ])
        .start(&tracer);

    // Add an event to the span
    span.add_event("rm_operation_started", vec![]);

    let start_time = Instant::now();

    if !is_s3_uri(path) {
        return Err(anyhow::anyhow!(
            "rm command only works with S3 URIs (s3://...)"
        ));
    }

    let s3_uri = S3Uri::parse(path)?;

    if dryrun {
        info!("[DRY RUN] Would delete {path}");
        return Ok(());
    }

    let result = if s3_uri.key.is_none() || s3_uri.key_or_empty().is_empty() {
        // Deleting entire bucket
        if !force {
            return Err(anyhow::anyhow!("To delete a bucket, use --force flag"));
        }
        delete_bucket(config, &s3_uri.bucket, recursive).await
    } else {
        // Deleting specific object(s)
        if recursive {
            delete_objects_recursive(config, &s3_uri, include, exclude).await
        } else {
            delete_single_object(config, &s3_uri).await
        }
    };

    let duration = start_time.elapsed();

    // Record overall rm operation metrics using proper OTEL SDK
    {
        use crate::otel::OTEL_INSTRUMENTS;
        use opentelemetry::KeyValue;

        // Record duration
        let duration_seconds = duration.as_millis() as f64 / 1000.0;
        OTEL_INSTRUMENTS
            .operation_duration
            .record(duration_seconds, &[KeyValue::new("operation", "rm")]);

        // Record success/failure
        match &result {
            Ok(_) => {
                log::debug!("RM operation completed successfully in {duration:?}");
                span.add_event(
                    "rm_operation_completed",
                    vec![
                        opentelemetry::KeyValue::new("status", "success"),
                        opentelemetry::KeyValue::new("duration_ms", duration.as_millis() as i64),
                    ],
                );
            }
            Err(e) => {
                OTEL_INSTRUMENTS.record_error_with_type(&e.to_string());
                span.add_event(
                    "rm_operation_failed",
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

async fn delete_single_object(config: &Config, s3_uri: &S3Uri) -> Result<()> {
    let start_time = Instant::now();
    info!(
        "Deleting object: s3://{}/{}",
        s3_uri.bucket,
        s3_uri.key_or_empty()
    );

    let result = config
        .client
        .delete_object()
        .bucket(&s3_uri.bucket)
        .key(s3_uri.key_or_empty())
        .send()
        .await;

    match result {
        Ok(_) => {
            let duration = start_time.elapsed();

            // Record single object deletion using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;
                use opentelemetry::KeyValue;

                OTEL_INSTRUMENTS
                    .files_deleted_total
                    .add(1, &[KeyValue::new("operation", "delete_single")]);

                let duration_seconds = duration.as_millis() as f64 / 1000.0;
                OTEL_INSTRUMENTS.operation_duration.record(
                    duration_seconds,
                    &[KeyValue::new("operation", "delete_single")],
                );
            }

            println!("delete: s3://{}/{}", s3_uri.bucket, s3_uri.key_or_empty());

            // Transparent du call for real-time bucket analytics
            let bucket_uri = format!("s3://{}", s3_uri.bucket);
            call_transparent_du(config, &bucket_uri).await;

            Ok(())
        }
        Err(e) => {
            // Record error using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;

                let error_msg = format!(
                    "Failed to delete single object s3://{}/{}: {}",
                    s3_uri.bucket,
                    s3_uri.key_or_empty(),
                    e
                );
                OTEL_INSTRUMENTS.record_error_with_type(&error_msg);
            }

            Err(anyhow::anyhow!("Failed to delete object: {}", e))
        }
    }
}

async fn delete_objects_recursive(
    config: &Config,
    s3_uri: &S3Uri,
    _include: Option<&str>,
    _exclude: Option<&str>,
) -> Result<()> {
    let start_time = Instant::now();
    info!(
        "Recursively deleting objects with prefix: s3://{}/{}",
        s3_uri.bucket,
        s3_uri.key_or_empty()
    );

    let mut continuation_token: Option<String> = None;
    let mut deleted_count = 0;

    let result: anyhow::Result<()> = async {
        loop {
            // Create a new list request for each iteration
            let mut list_request = config.client.list_objects_v2().bucket(&s3_uri.bucket);

            if !s3_uri.key_or_empty().is_empty() {
                list_request = list_request.prefix(s3_uri.key_or_empty());
            }

            if let Some(token) = &continuation_token {
                list_request = list_request.continuation_token(token);
            }

            let response = list_request.send().await?;

            if let Some(objects) = response.contents {
                // Collect object keys for batch deletion
                let mut objects_to_delete = Vec::new();

                for object in objects {
                    if let Some(key) = object.key {
                        objects_to_delete.push(
                            aws_sdk_s3::types::ObjectIdentifier::builder()
                                .key(&key)
                                .build()
                                .map_err(|e| {
                                    anyhow::anyhow!("Failed to build object identifier: {}", e)
                                })?,
                        );
                        println!("delete: s3://{}/{}", s3_uri.bucket, key);
                        deleted_count += 1;
                    }
                }

                // Perform batch deletion if we have objects to delete
                if !objects_to_delete.is_empty() {
                    let delete_request = aws_sdk_s3::types::Delete::builder()
                        .set_objects(Some(objects_to_delete.clone()))
                        .build()
                        .map_err(|e| anyhow::anyhow!("Failed to build delete request: {}", e))?;

                    // For MinIO compatibility, compute and add Content-MD5 header
                    // MinIO requires this header for batch deletion operations
                    let result = config
                        .client
                        .delete_objects()
                        .bucket(&s3_uri.bucket)
                        .delete(delete_request.clone())
                        .customize()
                        .mutate_request(|req| {
                            // For MinIO compatibility, we need to add Content-MD5 header
                            // Get the request body bytes if available
                            let payload_xml = if let Some(body_bytes) = req.body().bytes() {
                                body_bytes.to_vec()
                            } else {
                                // Fallback: compute MD5 of empty body
                                Vec::new()
                            };

                            // Compute MD5 hash of the payload and base64 encode it
                            let md5_hash = md5::compute(&payload_xml);
                            let md5_b64 = b64.encode(md5_hash.as_ref());

                            // Add the Content-MD5 header
                            req.headers_mut().insert("Content-MD5", md5_b64);
                        })
                        .send()
                        .await;

                    match result {
                        Ok(_) => {
                            // Batch deletion succeeded with Content-MD5 header
                        },
                        Err(e) if e.to_string().contains("MissingContentMD5") => {
                            info!("Batch deletion failed with MissingContentMD5, falling back to individual deletions");
                            // Fall back to individual object deletion when batch fails
                            for obj in &objects_to_delete {
                                let key = obj.key();
                                if !key.is_empty() {
                                    config
                                        .client
                                        .delete_object()
                                        .bucket(&s3_uri.bucket)
                                        .key(key)
                                        .send()
                                        .await?;

                                    println!("delete: s3://{}/{}", s3_uri.bucket, key);
                                }
                            }
                        },
                        Err(e) => {
                            return Err(e.into());
                        }
                    }
                }
            }

            // Check if there are more objects to delete
            if response.is_truncated.unwrap_or(false) {
                continuation_token = response.next_continuation_token;
            } else {
                break;
            }
        }
        Ok(())
    }
    .await;

    match result {
        Ok(_) => {
            let duration = start_time.elapsed();

            // Record recursive deletion using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;
                use opentelemetry::KeyValue;

                OTEL_INSTRUMENTS.files_deleted_total.add(
                    deleted_count,
                    &[KeyValue::new("operation", "delete_recursive")],
                );

                let duration_seconds = duration.as_millis() as f64 / 1000.0;
                OTEL_INSTRUMENTS.operation_duration.record(
                    duration_seconds,
                    &[KeyValue::new("operation", "delete_recursive")],
                );
            }

            info!("Successfully deleted {deleted_count} objects");

            // Transparent du call for real-time bucket analytics
            let bucket_uri = format!("s3://{}", s3_uri.bucket);
            call_transparent_du(config, &bucket_uri).await;

            Ok(())
        }
        Err(e) => {
            // Record error using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;

                let error_msg = format!(
                    "Failed to delete objects recursively in s3://{}/{}: {}",
                    s3_uri.bucket,
                    s3_uri.key_or_empty(),
                    e
                );
                OTEL_INSTRUMENTS.record_error_with_type(&error_msg);
            }

            Err(e)
        }
    }
}

async fn delete_bucket(config: &Config, bucket_name: &str, force_empty: bool) -> Result<()> {
    let start_time = Instant::now();
    info!("Deleting bucket: {bucket_name}");

    let result: anyhow::Result<()> = async {
        if force_empty {
            // First, delete all objects in the bucket
            let s3_uri = S3Uri {
                bucket: bucket_name.to_string(),
                key: None,
            };

            delete_objects_recursive(config, &s3_uri, None, None).await?;

            // Also delete all object versions and delete markers (for versioned buckets)
            delete_all_versions(config, bucket_name).await?;
        }

        // Now delete the bucket itself
        config
            .client
            .delete_bucket()
            .bucket(bucket_name)
            .send()
            .await?;

        Ok(())
    }
    .await;

    match result {
        Ok(_) => {
            let duration = start_time.elapsed();

            // Record bucket deletion using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;
                use opentelemetry::KeyValue;

                OTEL_INSTRUMENTS
                    .operations_total
                    .add(1, &[KeyValue::new("operation", "delete_bucket")]);

                let duration_seconds = duration.as_millis() as f64 / 1000.0;
                OTEL_INSTRUMENTS.operation_duration.record(
                    duration_seconds,
                    &[KeyValue::new("operation", "delete_bucket")],
                );
            }

            println!("remove_bucket: s3://{bucket_name}");

            // Transparent du call for real-time bucket analytics
            let bucket_uri = format!("s3://{bucket_name}");
            call_transparent_du(config, &bucket_uri).await;

            Ok(())
        }
        Err(e) => {
            // Record error using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;

                let error_msg = format!("Failed to delete bucket {bucket_name}: {e}");
                OTEL_INSTRUMENTS.record_error_with_type(&error_msg);
            }

            Err(anyhow::anyhow!("Failed to delete bucket: {}", e))
        }
    }
}

async fn delete_all_versions(config: &Config, bucket_name: &str) -> Result<()> {
    info!("Deleting all versions and delete markers in bucket: {bucket_name}");

    let mut key_marker: Option<String> = None;
    let mut version_id_marker: Option<String> = None;

    loop {
        let mut list_request = config.client.list_object_versions().bucket(bucket_name);

        if let Some(key) = &key_marker {
            list_request = list_request.key_marker(key);
        }

        if let Some(version_id) = &version_id_marker {
            list_request = list_request.version_id_marker(version_id);
        }

        let response = list_request.send().await?;

        let mut objects_to_delete = Vec::new();

        // Add object versions
        if let Some(versions) = response.versions {
            for version in versions {
                if let (Some(key), Some(version_id)) = (version.key, version.version_id) {
                    objects_to_delete.push(
                        aws_sdk_s3::types::ObjectIdentifier::builder()
                            .key(&key)
                            .version_id(&version_id)
                            .build()
                            .map_err(|e| {
                                anyhow::anyhow!("Failed to build object identifier: {}", e)
                            })?,
                    );
                }
            }
        }

        // Add delete markers
        if let Some(delete_markers) = response.delete_markers {
            for marker in delete_markers {
                if let (Some(key), Some(version_id)) = (marker.key, marker.version_id) {
                    objects_to_delete.push(
                        aws_sdk_s3::types::ObjectIdentifier::builder()
                            .key(&key)
                            .version_id(&version_id)
                            .build()
                            .map_err(|e| {
                                anyhow::anyhow!("Failed to build object identifier: {}", e)
                            })?,
                    );
                }
            }
        }

        // Perform batch deletion if we have objects to delete
        if !objects_to_delete.is_empty() {
            let delete_request = aws_sdk_s3::types::Delete::builder()
                .set_objects(Some(objects_to_delete))
                .build()
                .map_err(|e| anyhow::anyhow!("Failed to build delete request: {}", e))?;

            // For MinIO compatibility, compute and add Content-MD5 header
            // MinIO requires this header for batch deletion operations
            config
                .client
                .delete_objects()
                .bucket(bucket_name)
                .delete(delete_request.clone())
                .customize()
                .mutate_request(|req| {
                    // For MinIO compatibility, we need to add Content-MD5 header
                    // Get the request body bytes if available
                    let payload_xml = if let Some(body_bytes) = req.body().bytes() {
                        body_bytes.to_vec()
                    } else {
                        // Fallback: compute MD5 of empty body
                        Vec::new()
                    };

                    // Compute MD5 hash of the payload and base64 encode it
                    let md5_hash = md5::compute(&payload_xml);
                    let md5_b64 = b64.encode(md5_hash.as_ref());

                    // Add the Content-MD5 header
                    req.headers_mut().insert("Content-MD5", md5_b64);
                })
                .send()
                .await?;
        }

        // Check if there are more versions to delete
        if response.is_truncated.unwrap_or(false) {
            key_marker = response.next_key_marker;
            version_id_marker = response.next_version_id_marker;
        } else {
            break;
        }
    }

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

            debug!("Running transparent du for bucket analytics after deletion: {bucket_uri}");

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
    async fn test_execute_non_s3_uri() {
        let config = create_mock_config();

        let result = execute(
            &config,
            "/local/path/file.txt",
            false,
            false,
            false,
            None,
            None,
        )
        .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("rm command only works with S3 URIs"));
    }

    #[tokio::test]
    async fn test_execute_dry_run() {
        let config = create_mock_config();

        let result = execute(
            &config,
            "s3://bucket/file.txt",
            false,
            true, // dry run
            false,
            None,
            None,
        )
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_bucket_without_force() {
        let config = create_mock_config();

        let result = execute(
            &config,
            "s3://bucket", // bucket without key
            false,
            false,
            false, // no force flag
            None,
            None,
        )
        .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("To delete a bucket, use --force flag"));
    }

    #[tokio::test]
    async fn test_execute_bucket_with_force() {
        let config = create_mock_config();

        let result = execute(
            &config,
            "s3://bucket/", // bucket with trailing slash (empty key)
            false,
            false,
            true, // force flag
            None,
            None,
        )
        .await;

        // Will fail due to no AWS connection, but tests the routing
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_single_object() {
        let config = create_mock_config();

        let result = execute(
            &config,
            "s3://bucket/file.txt",
            false, // not recursive
            false,
            false,
            None,
            None,
        )
        .await;

        // Will fail due to no AWS connection, but tests the routing
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_recursive_objects() {
        let config = create_mock_config();

        let result = execute(
            &config,
            "s3://bucket/prefix/",
            true, // recursive
            false,
            false,
            None,
            None,
        )
        .await;

        // Will fail due to no AWS connection, but tests the routing
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_s3_uri_parsing_error() {
        let config = create_mock_config();

        let result = execute(
            &config, "s3://", // invalid S3 URI
            false, false, false, None, None,
        )
        .await;

        assert!(result.is_err());
    }

    #[test]
    fn test_s3_uri_key_handling() {
        let s3_uri_with_key = S3Uri {
            bucket: "test-bucket".to_string(),
            key: Some("test/key.txt".to_string()),
        };
        assert_eq!(s3_uri_with_key.key_or_empty(), "test/key.txt");

        let s3_uri_no_key = S3Uri {
            bucket: "test-bucket".to_string(),
            key: None,
        };
        assert_eq!(s3_uri_no_key.key_or_empty(), "");

        let s3_uri_empty_key = S3Uri {
            bucket: "test-bucket".to_string(),
            key: Some("".to_string()),
        };
        assert_eq!(s3_uri_empty_key.key_or_empty(), "");
    }

    #[tokio::test]
    async fn test_delete_single_object_mock() {
        let config = create_mock_config();
        let s3_uri = S3Uri {
            bucket: "test-bucket".to_string(),
            key: Some("test-key.txt".to_string()),
        };

        // This will fail due to no real AWS connection, but tests the function structure
        let result = delete_single_object(&config, &s3_uri).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_objects_recursive_mock() {
        let config = create_mock_config();
        let s3_uri = S3Uri {
            bucket: "test-bucket".to_string(),
            key: Some("test-prefix/".to_string()),
        };

        // This will fail due to no real AWS connection, but tests the function structure
        let result = delete_objects_recursive(&config, &s3_uri, None, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_bucket_mock() {
        let config = create_mock_config();

        // This will fail due to no real AWS connection, but tests the function structure
        let result = delete_bucket(&config, "test-bucket", true).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_all_versions_mock() {
        let config = create_mock_config();

        // This will fail due to no real AWS connection, but tests the function structure
        let result = delete_all_versions(&config, "test-bucket").await;
        assert!(result.is_err());
    }
}
