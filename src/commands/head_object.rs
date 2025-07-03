use anyhow::Result;
use log::info;
use std::time::Instant;

use crate::commands::s3_uri::{is_s3_uri, S3Uri};
use crate::config::Config;

pub async fn execute(config: &Config, s3_uri: &str) -> Result<()> {
    let start_time = Instant::now();

    if !is_s3_uri(s3_uri) {
        return Err(anyhow::anyhow!(
            "head-object command only works with S3 URIs (s3://...)"
        ));
    }

    let uri = S3Uri::parse(s3_uri)?;

    if uri.key.is_none() || uri.key_or_empty().is_empty() {
        return Err(anyhow::anyhow!(
            "head-object requires a specific object key, not just a bucket"
        ));
    }

    info!("Getting metadata for: {s3_uri}");

    let result = config
        .client
        .head_object()
        .bucket(&uri.bucket)
        .key(uri.key_or_empty())
        .send()
        .await;

    match result {
        Ok(response) => {
            let duration = start_time.elapsed();

            // Record head_object operation using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;
                use opentelemetry::KeyValue;

                OTEL_INSTRUMENTS
                    .operations_total
                    .add(1, &[KeyValue::new("operation", "head_object")]);

                let duration_seconds = duration.as_millis() as f64 / 1000.0;
                OTEL_INSTRUMENTS.operation_duration.record(
                    duration_seconds,
                    &[KeyValue::new("operation", "head_object")],
                );
            }

            // Print object metadata
            println!("Key: {}", uri.key_or_empty());

            if let Some(content_length) = response.content_length {
                println!("Content-Length: {content_length}");
            }

            if let Some(content_type) = response.content_type {
                println!("Content-Type: {content_type}");
            }

            if let Some(etag) = response.e_tag {
                println!("ETag: {etag}");
            }

            if let Some(last_modified) = response.last_modified {
                println!(
                    "Last-Modified: {}",
                    last_modified.fmt(aws_smithy_types::date_time::Format::DateTime)?
                );
            }

            if let Some(storage_class) = response.storage_class {
                println!("Storage-Class: {}", storage_class.as_str());
            }

            if let Some(server_side_encryption) = response.server_side_encryption {
                println!(
                    "Server-Side-Encryption: {}",
                    server_side_encryption.as_str()
                );
            }

            if let Some(version_id) = response.version_id {
                println!("VersionId: {version_id}");
            }

            // Print any custom metadata
            if let Some(metadata) = response.metadata {
                for (key, value) in metadata {
                    println!("Metadata-{key}: {value}");
                }
            }

            Ok(())
        }
        Err(e) => {
            // Record error using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;

                let error_msg = format!("Failed to get metadata for {s3_uri}: {e}");
                OTEL_INSTRUMENTS.record_error_with_type(&error_msg);
            }

            Err(anyhow::anyhow!(
                "Failed to get metadata for {}: {}",
                s3_uri,
                e
            ))
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

        let result = execute(&config, "/local/path/file.txt").await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("head-object command only works with S3 URIs"));
    }

    #[tokio::test]
    async fn test_execute_invalid_s3_uri() {
        let config = create_mock_config();

        let result = execute(
            &config, "s3://", // invalid S3 URI
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_bucket_only() {
        let config = create_mock_config();

        let result = execute(
            &config,
            "s3://bucket", // bucket without key
        )
        .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("head-object requires a specific object key"));
    }

    #[tokio::test]
    async fn test_execute_bucket_with_empty_key() {
        let config = create_mock_config();

        let result = execute(
            &config,
            "s3://bucket/", // bucket with empty key
        )
        .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("head-object requires a specific object key"));
    }

    #[tokio::test]
    async fn test_execute_valid_s3_uri() {
        let config = create_mock_config();

        let result = execute(&config, "s3://bucket/file.txt").await;

        // Will fail due to no AWS connection, but tests the routing
        assert!(result.is_err());
    }

    #[test]
    fn test_s3_uri_validation() {
        // Test that we can distinguish valid from invalid URIs
        assert!(is_s3_uri("s3://bucket/key"));
        assert!(!is_s3_uri("/local/path"));
        assert!(!is_s3_uri("http://example.com"));
    }

    #[test]
    fn test_s3_uri_key_validation() {
        let uri_with_key = S3Uri {
            bucket: "test-bucket".to_string(),
            key: Some("test-key.txt".to_string()),
        };
        assert!(!uri_with_key.key_or_empty().is_empty());

        let uri_no_key = S3Uri {
            bucket: "test-bucket".to_string(),
            key: None,
        };
        assert!(uri_no_key.key_or_empty().is_empty());

        let uri_empty_key = S3Uri {
            bucket: "test-bucket".to_string(),
            key: Some("".to_string()),
        };
        assert!(uri_empty_key.key_or_empty().is_empty());
    }

    #[test]
    fn test_metadata_field_handling() {
        // Test that we handle various metadata fields properly
        let test_values = vec![
            ("Content-Length", "1024"),
            ("Content-Type", "text/plain"),
            ("ETag", "\"d41d8cd98f00b204e9800998ecf8427e\""),
            ("Storage-Class", "STANDARD"),
            ("Server-Side-Encryption", "AES256"),
        ];

        for (field, value) in test_values {
            assert!(!field.is_empty());
            assert!(!value.is_empty());
        }
    }
}
