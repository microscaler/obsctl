use anyhow::Result;
use log::info;
use std::time::Instant;

use crate::commands::s3_uri::{is_s3_uri, S3Uri};
use crate::config::Config;

pub async fn execute(
    config: &Config,
    s3_uri: &str,
    expires_in: u64,
    method: Option<&str>,
) -> Result<()> {
    let start_time = Instant::now();

    if !is_s3_uri(s3_uri) {
        return Err(anyhow::anyhow!(
            "presign command only works with S3 URIs (s3://...)"
        ));
    }

    let uri = S3Uri::parse(s3_uri)?;

    if uri.key.is_none() || uri.key_or_empty().is_empty() {
        return Err(anyhow::anyhow!(
            "presign requires a specific object key, not just a bucket"
        ));
    }

    info!("Generating presigned URL for: {s3_uri}");

    let method = method.unwrap_or("GET");

    let result = match method.to_uppercase().as_str() {
        "GET" => generate_get_presigned_url(config, &uri, expires_in).await,
        "PUT" => generate_put_presigned_url(config, &uri, expires_in).await,
        "DELETE" => generate_delete_presigned_url(config, &uri, expires_in).await,
        _ => Err(anyhow::anyhow!(
            "Unsupported HTTP method: {}. Supported methods: GET, PUT, DELETE",
            method
        )),
    };

    // Record presign operation using proper OTEL SDK
    match result {
        Ok(_) => {
            let duration = start_time.elapsed();

            {
                use crate::otel::OTEL_INSTRUMENTS;
                use opentelemetry::KeyValue;

                let operation_type = format!("presign_{}", method.to_lowercase());

                OTEL_INSTRUMENTS
                    .operations_total
                    .add(1, &[KeyValue::new("operation", operation_type.clone())]);

                let duration_seconds = duration.as_millis() as f64 / 1000.0;
                OTEL_INSTRUMENTS.operation_duration.record(
                    duration_seconds,
                    &[KeyValue::new("operation", operation_type)],
                );
            }

            Ok(())
        }
        Err(e) => {
            // Record error using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;

                let error_msg = format!(
                    "Failed to generate presigned URL for {s3_uri} ({method}): {e}"
                );
                OTEL_INSTRUMENTS.record_error_with_type(&error_msg);
            }

            Err(e)
        }
    }
}

async fn generate_get_presigned_url(
    config: &Config,
    s3_uri: &S3Uri,
    expires_in: u64,
) -> Result<()> {
    let start_time = Instant::now();
    let expiration = std::time::Duration::from_secs(expires_in);

    let result = config
        .client
        .get_object()
        .bucket(&s3_uri.bucket)
        .key(s3_uri.key_or_empty())
        .presigned(aws_sdk_s3::presigning::PresigningConfig::expires_in(
            expiration,
        )?)
        .await;

    match result {
        Ok(presigned_request) => {
            let duration = start_time.elapsed();

            // Record GET presign operation using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;
                use opentelemetry::KeyValue;

                OTEL_INSTRUMENTS.operations_total.add(
                    1,
                    &[KeyValue::new("operation", "generate_get_presigned_url")],
                );

                let duration_seconds = duration.as_millis() as f64 / 1000.0;
                OTEL_INSTRUMENTS.operation_duration.record(
                    duration_seconds,
                    &[KeyValue::new("operation", "generate_get_presigned_url")],
                );
            }

            println!("{}", presigned_request.uri());
            Ok(())
        }
        Err(e) => {
            // Record error using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;

                let error_msg = format!("Failed to generate GET presigned URL: {e}");
                OTEL_INSTRUMENTS.record_error_with_type(&error_msg);
            }

            Err(anyhow::anyhow!(
                "Failed to generate GET presigned URL: {}",
                e
            ))
        }
    }
}

async fn generate_put_presigned_url(
    config: &Config,
    s3_uri: &S3Uri,
    expires_in: u64,
) -> Result<()> {
    let start_time = Instant::now();
    let expiration = std::time::Duration::from_secs(expires_in);

    let result = config
        .client
        .put_object()
        .bucket(&s3_uri.bucket)
        .key(s3_uri.key_or_empty())
        .presigned(aws_sdk_s3::presigning::PresigningConfig::expires_in(
            expiration,
        )?)
        .await;

    match result {
        Ok(presigned_request) => {
            let duration = start_time.elapsed();

            // Record PUT presign operation using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;
                use opentelemetry::KeyValue;

                OTEL_INSTRUMENTS.operations_total.add(
                    1,
                    &[KeyValue::new("operation", "generate_put_presigned_url")],
                );

                let duration_seconds = duration.as_millis() as f64 / 1000.0;
                OTEL_INSTRUMENTS.operation_duration.record(
                    duration_seconds,
                    &[KeyValue::new("operation", "generate_put_presigned_url")],
                );
            }

            println!("{}", presigned_request.uri());
            Ok(())
        }
        Err(e) => {
            // Record error using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;

                let error_msg = format!("Failed to generate PUT presigned URL: {e}");
                OTEL_INSTRUMENTS.record_error_with_type(&error_msg);
            }

            Err(anyhow::anyhow!(
                "Failed to generate PUT presigned URL: {}",
                e
            ))
        }
    }
}

async fn generate_delete_presigned_url(
    config: &Config,
    s3_uri: &S3Uri,
    expires_in: u64,
) -> Result<()> {
    let start_time = Instant::now();
    let expiration = std::time::Duration::from_secs(expires_in);

    let result = config
        .client
        .delete_object()
        .bucket(&s3_uri.bucket)
        .key(s3_uri.key_or_empty())
        .presigned(aws_sdk_s3::presigning::PresigningConfig::expires_in(
            expiration,
        )?)
        .await;

    match result {
        Ok(presigned_request) => {
            let duration = start_time.elapsed();

            // Record DELETE presign operation using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;
                use opentelemetry::KeyValue;

                OTEL_INSTRUMENTS.operations_total.add(
                    1,
                    &[KeyValue::new("operation", "generate_delete_presigned_url")],
                );

                let duration_seconds = duration.as_millis() as f64 / 1000.0;
                OTEL_INSTRUMENTS.operation_duration.record(
                    duration_seconds,
                    &[KeyValue::new("operation", "generate_delete_presigned_url")],
                );
            }

            println!("{}", presigned_request.uri());
            Ok(())
        }
        Err(e) => {
            // Record error using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;

                let error_msg = format!("Failed to generate DELETE presigned URL: {e}");
                OTEL_INSTRUMENTS.record_error_with_type(&error_msg);
            }

            Err(anyhow::anyhow!(
                "Failed to generate DELETE presigned URL: {}",
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
            },
        }
    }

    #[tokio::test]
    async fn test_execute_non_s3_uri() {
        let config = create_mock_config();

        let result = execute(&config, "/local/path/file.txt", 3600, None).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("presign command only works with S3 URIs"));
    }

    #[tokio::test]
    async fn test_execute_invalid_s3_uri() {
        let config = create_mock_config();

        let result = execute(
            &config, "s3://", // invalid S3 URI
            3600, None,
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_get_method() {
        let config = create_mock_config();

        let result = execute(&config, "s3://bucket/file.txt", 3600, Some("GET")).await;

        // Presign works with mock clients, so this should succeed
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_put_method() {
        let config = create_mock_config();

        let result = execute(&config, "s3://bucket/file.txt", 3600, Some("PUT")).await;

        // Presign works with mock clients, so this should succeed
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_delete_method() {
        let config = create_mock_config();

        let result = execute(&config, "s3://bucket/file.txt", 3600, Some("DELETE")).await;

        // Presign works with mock clients, so this should succeed
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_default_method() {
        let config = create_mock_config();

        // Test with no method specified (should default to GET)
        let result = execute(&config, "s3://bucket/file.txt", 3600, None).await;

        // Presign works with mock clients, so this should succeed
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_unsupported_method() {
        let config = create_mock_config();

        let result = execute(
            &config,
            "s3://bucket/file.txt",
            3600,
            Some("POST"), // unsupported method
        )
        .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unsupported HTTP method: POST"));
    }

    #[tokio::test]
    async fn test_execute_case_insensitive_method() {
        let config = create_mock_config();

        // Test with lowercase method
        let result = execute(&config, "s3://bucket/file.txt", 3600, Some("get")).await;

        // Presign works with mock clients, so this should succeed
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_generate_get_presigned_url() {
        let config = create_mock_config();
        let s3_uri = S3Uri {
            bucket: "test-bucket".to_string(),
            key: Some("test-key.txt".to_string()),
        };

        // Presign works with mock clients, so this should succeed
        let result = generate_get_presigned_url(&config, &s3_uri, 3600).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_generate_put_presigned_url() {
        let config = create_mock_config();
        let s3_uri = S3Uri {
            bucket: "test-bucket".to_string(),
            key: Some("test-key.txt".to_string()),
        };

        // Presign works with mock clients, so this should succeed
        let result = generate_put_presigned_url(&config, &s3_uri, 3600).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_generate_delete_presigned_url() {
        let config = create_mock_config();
        let s3_uri = S3Uri {
            bucket: "test-bucket".to_string(),
            key: Some("test-key.txt".to_string()),
        };

        // Presign works with mock clients, so this should succeed
        let result = generate_delete_presigned_url(&config, &s3_uri, 3600).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_expiration_duration() {
        let expires_in = 3600u64;
        let duration = std::time::Duration::from_secs(expires_in);

        assert_eq!(duration.as_secs(), 3600);
        assert_eq!(duration.as_secs(), expires_in);
    }

    #[test]
    fn test_method_normalization() {
        let methods = vec!["GET", "get", "Get", "PUT", "put", "DELETE", "delete"];

        for method in methods {
            let normalized = method.to_uppercase();
            assert!(matches!(normalized.as_str(), "GET" | "PUT" | "DELETE"));
        }
    }
}
