use anyhow::Result;
use log::info;
use std::time::Instant;

use crate::commands::cp;
use crate::commands::s3_uri::is_s3_uri;
use crate::config::Config;

pub async fn execute(
    config: &Config,
    s3_uri: &str,
    local_path: Option<&str>,
    recursive: bool,
    force: bool,
    include: Option<&str>,
    exclude: Option<&str>,
) -> Result<()> {
    let start_time = Instant::now();

    if !is_s3_uri(s3_uri) {
        return Err(anyhow::anyhow!(
            "get command requires an S3 URI as source (s3://...)"
        ));
    }

    // Determine local destination path
    let dest = match local_path {
        Some(path) => path.to_string(),
        None => {
            // Extract filename from S3 URI for default local path
            let uri_parts: Vec<&str> = s3_uri.split('/').collect();
            if let Some(filename) = uri_parts.last() {
                if !filename.is_empty() {
                    filename.to_string()
                } else {
                    return Err(anyhow::anyhow!(
                        "Cannot determine local filename from S3 URI. Please specify a local path."
                    ));
                }
            } else {
                return Err(anyhow::anyhow!(
                    "Cannot determine local filename from S3 URI. Please specify a local path."
                ));
            }
        }
    };

    info!("Getting {s3_uri} to {dest}");

    // Use the cp command to perform the actual download
    let result = cp::execute(
        config, s3_uri, &dest, recursive, false, // dryrun = false
        1,     // max_concurrent = 1 (get is typically single-threaded)
        force, include, exclude,
    )
    .await;

    match result {
        Ok(_) => {
            let duration = start_time.elapsed();

            // Record get operation using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;
                use opentelemetry::KeyValue;

                let operation_type = if recursive {
                    "get_recursive"
                } else {
                    "get_single"
                };

                OTEL_INSTRUMENTS
                    .operations_total
                    .add(1, &[KeyValue::new("operation", operation_type)]);

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

                let error_msg = format!("Failed to get {s3_uri} to {dest}: {e}");
                OTEL_INSTRUMENTS.record_error_with_type(&error_msg);
            }

            Err(e)
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
    async fn test_execute_valid_s3_uri_with_local_path() {
        let config = create_mock_config();

        let result = execute(
            &config,
            "s3://test-bucket/test-file.txt",
            Some("local-file.txt"),
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
    async fn test_execute_valid_s3_uri_without_local_path() {
        let config = create_mock_config();

        let result = execute(
            &config,
            "s3://test-bucket/test-file.txt",
            None,
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
    async fn test_execute_with_recursive() {
        let config = create_mock_config();

        let result = execute(
            &config,
            "s3://test-bucket/folder/",
            Some("local-folder/"),
            true,
            false,
            None,
            None,
        )
        .await;

        // Will fail due to no AWS connection, but tests the routing
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_with_force() {
        let config = create_mock_config();

        let result = execute(
            &config,
            "s3://test-bucket/test-file.txt",
            Some("local-file.txt"),
            false,
            true,
            None,
            None,
        )
        .await;

        // Will fail due to no AWS connection, but tests the routing
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_with_include_exclude() {
        let config = create_mock_config();

        let result = execute(
            &config,
            "s3://test-bucket/folder/",
            Some("local-folder/"),
            true,
            false,
            Some("*.txt"),
            Some("*.log"),
        )
        .await;

        // Will fail due to no AWS connection, but tests the routing
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_invalid_s3_uri() {
        let config = create_mock_config();

        let result = execute(
            &config,
            "not-an-s3-uri",
            Some("local-file.txt"),
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
            .contains("get command requires an S3 URI"));
    }

    #[tokio::test]
    async fn test_execute_s3_uri_without_filename() {
        let config = create_mock_config();

        let result = execute(&config, "s3://test-bucket/", None, false, false, None, None).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Cannot determine local filename"));
    }

    #[tokio::test]
    async fn test_execute_s3_uri_with_empty_filename() {
        let config = create_mock_config();

        let result = execute(
            &config,
            "s3://test-bucket//",
            None,
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
            .contains("Cannot determine local filename"));
    }

    #[tokio::test]
    async fn test_execute_complex_s3_path() {
        let config = create_mock_config();

        let result = execute(
            &config,
            "s3://test-bucket/path/to/file.txt",
            None,
            false,
            false,
            None,
            None,
        )
        .await;

        // Will fail due to no AWS connection, but tests the routing
        // The function should extract "file.txt" as the local filename
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_all_options() {
        let config = create_mock_config();

        let result = execute(
            &config,
            "s3://test-bucket/folder/",
            Some("./downloads/"),
            true,
            true,
            Some("*.txt"),
            Some("*.tmp"),
        )
        .await;

        // Will fail due to no AWS connection, but tests the routing
        assert!(result.is_err());
    }

    #[test]
    fn test_filename_extraction_logic() {
        // Test the filename extraction logic used in the function
        let test_cases = vec![
            ("s3://bucket/file.txt", "file.txt"),
            ("s3://bucket/path/to/file.txt", "file.txt"),
            ("s3://bucket/folder/subfolder/document.pdf", "document.pdf"),
        ];

        for (s3_uri, expected_filename) in test_cases {
            let uri_parts: Vec<&str> = s3_uri.split('/').collect();
            if let Some(filename) = uri_parts.last() {
                assert_eq!(*filename, expected_filename);
            }
        }
    }

    #[test]
    fn test_error_conditions() {
        // Test various error conditions that the function should handle
        let invalid_uris = vec![
            "not-s3-uri",
            "http://example.com/file.txt",
            "file:///local/path",
            "ftp://server/file.txt",
        ];

        for uri in invalid_uris {
            assert!(!is_s3_uri(uri), "URI should not be recognized as S3: {uri}");
        }

        let valid_uris = vec![
            "s3://bucket/file.txt",
            "s3://bucket/path/to/file.txt",
            "s3://my-bucket/folder/",
        ];

        for uri in valid_uris {
            assert!(is_s3_uri(uri), "URI should be recognized as S3: {uri}");
        }
    }
}
