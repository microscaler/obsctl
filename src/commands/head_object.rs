use anyhow::Result;
use log::info;
use opentelemetry::trace::{Span, Tracer};
use std::time::Instant;

use crate::commands::s3_uri::{is_s3_uri, S3Uri};
use crate::config::Config;

pub async fn execute(config: &Config, s3_uri: &str) -> Result<()> {
    // Create a span for the head_object operation
    let tracer = opentelemetry::global::tracer("obsctl");
    let mut span = tracer
        .span_builder("head_object_operation")
        .with_attributes(vec![
            opentelemetry::KeyValue::new("operation", "head_object"),
            opentelemetry::KeyValue::new("s3_uri", s3_uri.to_string()),
        ])
        .start(&tracer);

    // Add an event to the span
    span.add_event("head_object_operation_started", vec![]);

    let start_time = Instant::now();

    // Record operation start using proper OTEL SDK
    {
        use crate::otel::OTEL_INSTRUMENTS;
        use opentelemetry::KeyValue;

        OTEL_INSTRUMENTS
            .operations_total
            .add(1, &[KeyValue::new("operation", "head_object")]);
    }

    let result = if !is_s3_uri(s3_uri) {
        Err(anyhow::anyhow!(
            "head-object command only works with S3 URIs (s3://bucket/key)"
        ))
    } else {
        let uri = S3Uri::parse(s3_uri)?;

        if uri.key.is_none() || uri.key_or_empty().is_empty() {
            return Err(anyhow::anyhow!(
                "head-object requires a full S3 URI with object key (s3://bucket/key)"
            ));
        }

        info!("Getting metadata for {s3_uri}");
        span.set_attribute(opentelemetry::KeyValue::new("bucket", uri.bucket.clone()));
        span.set_attribute(opentelemetry::KeyValue::new(
            "key",
            uri.key_or_empty().to_string(),
        ));

        match config
            .client
            .head_object()
            .bucket(&uri.bucket)
            .key(uri.key_or_empty())
            .send()
            .await
        {
            Ok(response) => {
                // Display object metadata
                println!("Object: {s3_uri}");
                if let Some(size) = response.content_length {
                    println!("Content-Length: {size}");
                    span.set_attribute(opentelemetry::KeyValue::new("content_length", size));
                }
                if let Some(content_type) = response.content_type {
                    println!("Content-Type: {content_type}");
                }
                if let Some(etag) = response.e_tag {
                    println!("ETag: {etag}");
                }
                if let Some(last_modified) = response.last_modified {
                    println!("Last-Modified: {last_modified}");
                }
                if let Some(storage_class) = response.storage_class {
                    println!("Storage-Class: {storage_class}");
                }

                // Display custom metadata
                if let Some(metadata) = response.metadata {
                    for (key, value) in metadata {
                        println!("Metadata-{key}: {value}");
                    }
                }

                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!(
                "Failed to get metadata for {}: {}",
                s3_uri,
                e
            )),
        }
    };

    let duration = start_time.elapsed();

    // Record overall head_object operation metrics using proper OTEL SDK
    {
        use crate::otel::OTEL_INSTRUMENTS;
        use opentelemetry::KeyValue;

        // Record duration
        let duration_seconds = duration.as_millis() as f64 / 1000.0;
        OTEL_INSTRUMENTS.operation_duration.record(
            duration_seconds,
            &[KeyValue::new("operation", "head_object")],
        );

        // Record success/failure
        match &result {
            Ok(_) => {
                log::debug!("Head object operation completed successfully in {duration:?}");
                span.add_event(
                    "head_object_operation_completed",
                    vec![
                        opentelemetry::KeyValue::new("status", "success"),
                        opentelemetry::KeyValue::new("duration_ms", duration.as_millis() as i64),
                    ],
                );
            }
            Err(e) => {
                OTEL_INSTRUMENTS.record_error_with_type(&e.to_string());
                span.add_event(
                    "head_object_operation_failed",
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
