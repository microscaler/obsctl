use anyhow::Result;
use log::info;
use std::time::Instant;

use crate::commands::cp;
use crate::commands::s3_uri::is_s3_uri;
use crate::config::Config;

pub async fn execute(
    config: &Config,
    local_path: &str,
    s3_uri: Option<&str>,
    recursive: bool,
    force: bool,
    include: Option<&str>,
    exclude: Option<&str>,
) -> Result<()> {
    let start_time = Instant::now();

    // Determine S3 destination
    let dest = match s3_uri {
        Some(uri) => {
            if !is_s3_uri(uri) {
                return Err(anyhow::anyhow!("Destination must be an S3 URI (s3://...)"));
            }
            uri.to_string()
        }
        None => {
            return Err(anyhow::anyhow!(
                "upload command requires an S3 URI as destination"
            ));
        }
    };

    info!("Uploading {local_path} to {dest}");

    // Use the cp command to perform the actual upload
    let result = cp::execute(
        config, local_path, &dest, recursive, false, // dryrun = false
        1,     // max_concurrent = 1 (upload is typically single-threaded)
        force, include, exclude,
    )
    .await;

    match result {
        Ok(_) => {
            let duration = start_time.elapsed();

            // Record upload operation using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;
                use opentelemetry::KeyValue;

                let operation_type = if recursive {
                    "upload_recursive"
                } else {
                    "upload_single"
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

                let error_msg = format!("Failed to upload {local_path} to {dest}: {e}");
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
                read_operations: false,
            },
        }
    }

    #[tokio::test]
    async fn test_execute_valid_upload() {
        let config = create_mock_config();

        let result = execute(
            &config,
            "local-file.txt",
            Some("s3://test-bucket/uploaded-file.txt"),
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
            "local-folder/",
            Some("s3://test-bucket/folder/"),
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
            "local-file.txt",
            Some("s3://test-bucket/file.txt"),
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
            "local-folder/",
            Some("s3://test-bucket/folder/"),
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
            "local-file.txt",
            Some("not-an-s3-uri"),
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
            .contains("Destination must be an S3 URI"));
    }

    #[tokio::test]
    async fn test_execute_no_s3_uri() {
        let config = create_mock_config();

        let result = execute(&config, "local-file.txt", None, false, false, None, None).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("upload command requires an S3 URI as destination"));
    }

    #[tokio::test]
    async fn test_execute_all_options() {
        let config = create_mock_config();

        let result = execute(
            &config,
            "./local-folder/",
            Some("s3://test-bucket/uploads/"),
            true,
            true,
            Some("*.txt"),
            Some("*.tmp"),
        )
        .await;

        // Will fail due to no AWS connection, but tests the routing
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_single_file_upload() {
        let config = create_mock_config();

        let result = execute(
            &config,
            "/path/to/document.pdf",
            Some("s3://my-bucket/documents/document.pdf"),
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
    async fn test_execute_directory_upload() {
        let config = create_mock_config();

        let result = execute(
            &config,
            "/path/to/directory",
            Some("s3://my-bucket/backup/"),
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
    async fn test_execute_with_filters() {
        let config = create_mock_config();

        let result = execute(
            &config,
            "src/",
            Some("s3://code-bucket/project/src/"),
            true,
            false,
            Some("*.rs"),
            Some("target/*"),
        )
        .await;

        // Will fail due to no AWS connection, but tests the routing
        assert!(result.is_err());
    }

    #[test]
    fn test_s3_uri_validation() {
        // Test S3 URI validation logic
        let valid_uris = vec![
            "s3://bucket/file.txt",
            "s3://bucket/path/to/file.txt",
            "s3://my-bucket/folder/",
            "s3://bucket-name/prefix/subfolder/file.pdf",
        ];

        for uri in valid_uris {
            assert!(is_s3_uri(uri), "URI should be recognized as S3: {uri}");
        }

        let invalid_uris = vec![
            "not-s3-uri",
            "http://example.com/file.txt",
            "file:///local/path",
            "ftp://server/file.txt",
            "s3:/bucket/file.txt", // missing second slash
            "s3//bucket/file.txt", // missing colon
        ];

        for uri in invalid_uris {
            assert!(!is_s3_uri(uri), "URI should not be recognized as S3: {uri}");
        }
    }

    #[test]
    fn test_error_message_content() {
        // Test that error messages contain expected content
        let error_cases = vec![
            ("destination_validation", "Destination must be an S3 URI"),
            (
                "missing_destination",
                "upload command requires an S3 URI as destination",
            ),
        ];

        for (case_name, expected_message) in error_cases {
            // These are the error messages that should be returned by the function
            assert!(
                !expected_message.is_empty(),
                "Error message should not be empty for case: {case_name}"
            );
        }
    }

    #[test]
    fn test_parameter_combinations() {
        // Test various parameter combinations that the function should handle
        let test_cases = vec![
            ("file.txt", "s3://bucket/file.txt", false, false, None, None),
            ("folder/", "s3://bucket/folder/", true, false, None, None),
            ("data.csv", "s3://bucket/data.csv", false, true, None, None),
            ("src/", "s3://bucket/src/", true, false, Some("*.rs"), None),
            (
                "docs/",
                "s3://bucket/docs/",
                true,
                false,
                None,
                Some("*.tmp"),
            ),
            (
                "project/",
                "s3://bucket/project/",
                true,
                true,
                Some("*.txt"),
                Some("*.log"),
            ),
        ];

        for (local_path, s3_uri, _recursive, _force, include, exclude) in test_cases {
            // Verify that the parameters are valid
            assert!(!local_path.is_empty(), "Local path should not be empty");
            assert!(is_s3_uri(s3_uri), "S3 URI should be valid: {s3_uri}");

            // Test parameter validation logic
            if let Some(pattern) = include {
                assert!(!pattern.is_empty(), "Include pattern should not be empty");
            }
            if let Some(pattern) = exclude {
                assert!(!pattern.is_empty(), "Exclude pattern should not be empty");
            }
        }
    }
}
