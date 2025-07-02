use anyhow::Result;
use log::info;
use std::collections::HashMap;
use std::time::Instant;

use crate::commands::s3_uri::{is_s3_uri, S3Uri};
use crate::config::Config;

pub async fn execute(
    config: &Config,
    s3_uri: &str,
    human_readable: bool,
    summarize: bool,
    max_depth: Option<usize>,
) -> Result<()> {
    execute_with_metrics_control(config, s3_uri, human_readable, summarize, max_depth, true).await
}

pub async fn execute_transparent(
    config: &Config,
    s3_uri: &str,
    human_readable: bool,
    summarize: bool,
    max_depth: Option<usize>,
) -> Result<()> {
    execute_with_metrics_control(config, s3_uri, human_readable, summarize, max_depth, false).await
}

async fn execute_with_metrics_control(
    config: &Config,
    s3_uri: &str,
    human_readable: bool,
    summarize: bool,
    max_depth: Option<usize>,
    record_user_operation: bool,
) -> Result<()> {
    let start_time = Instant::now();

    if !is_s3_uri(s3_uri) {
        return Err(anyhow::anyhow!(
            "du command only works with S3 URIs (s3://...)"
        ));
    }

    let uri = S3Uri::parse(s3_uri)?;

    info!("Calculating storage usage for: {s3_uri}");

    let result = scan_objects(config, &uri.bucket, uri.key.as_deref()).await;

    match result {
        Ok(objects) => {
            let duration = start_time.elapsed();
            let total_size: i64 = objects.iter().map(|obj| obj.size).sum();
            let object_count = objects.len();

            // Record comprehensive du operation metrics using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;
                use opentelemetry::KeyValue;

                let prefix_str = uri.key.as_deref().unwrap_or("").to_string();
                let bucket_str = uri.bucket.clone();

                // Only record user operation metrics if this is an explicit user command
                if record_user_operation {
                    // Basic du operation metrics - only for explicit user commands
                    OTEL_INSTRUMENTS.operations_total.add(
                        1,
                        &[
                            KeyValue::new("operation", "du"),
                            KeyValue::new("bucket", bucket_str.clone()),
                        ],
                    );

                    let duration_seconds = duration.as_millis() as f64 / 1000.0;
                    OTEL_INSTRUMENTS.operation_duration.record(
                        duration_seconds,
                        &[
                            KeyValue::new("operation", "du"),
                            KeyValue::new("bucket", bucket_str.clone()),
                        ],
                    );
                }

                // Bucket storage analytics - always record these for real-time bucket insights
                OTEL_INSTRUMENTS.files_uploaded_total.add(
                    object_count as u64,
                    &[
                        KeyValue::new("operation", "bucket_object_count"),
                        KeyValue::new("bucket", bucket_str.clone()),
                        KeyValue::new("prefix", prefix_str.clone()),
                    ],
                );

                OTEL_INSTRUMENTS.bytes_uploaded_total.add(
                    total_size as u64,
                    &[
                        KeyValue::new("operation", "bucket_storage_bytes"),
                        KeyValue::new("bucket", bucket_str.clone()),
                        KeyValue::new("prefix", prefix_str),
                    ],
                );

                // Bucket size categories for analytics - always record for dashboard insights
                let size_category = if total_size < 1_000_000 {
                    "small"
                } else if total_size < 100_000_000 {
                    "medium"
                } else if total_size < 1_000_000_000 {
                    "large"
                } else {
                    "xlarge"
                };

                OTEL_INSTRUMENTS.operations_total.add(
                    1,
                    &[
                        KeyValue::new("operation", "bucket_size_category"),
                        KeyValue::new("bucket", bucket_str.clone()),
                        KeyValue::new("size_category", size_category),
                    ],
                );

                // Object count categories for analytics - always record for dashboard insights
                let count_category = if object_count < 100 {
                    "few"
                } else if object_count < 1000 {
                    "moderate"
                } else if object_count < 10000 {
                    "many"
                } else {
                    "massive"
                };

                OTEL_INSTRUMENTS.operations_total.add(
                    1,
                    &[
                        KeyValue::new("operation", "bucket_object_category"),
                        KeyValue::new("bucket", bucket_str.clone()),
                        KeyValue::new("count_category", count_category),
                    ],
                );

                let operation_type = if record_user_operation {
                    "explicit"
                } else {
                    "transparent"
                };
                info!("Du metrics recorded ({operation_type}): bucket={bucket_str}, objects={object_count}, bytes={total_size}, size_category={size_category}, count_category={count_category}");
            }

            let directory_sizes = calculate_directory_sizes(&objects, max_depth);

            if summarize {
                let size_str = if human_readable {
                    format_size_human_readable(total_size)
                } else {
                    total_size.to_string()
                };
                println!("{size_str} {s3_uri}");
            } else {
                // Sort by path for consistent output
                let mut sorted_dirs: Vec<_> = directory_sizes.iter().collect();
                sorted_dirs.sort_by_key(|&(path, _)| path);

                for (path, size) in sorted_dirs {
                    let size_str = if human_readable {
                        format_size_human_readable(*size)
                    } else {
                        size.to_string()
                    };

                    let display_path = if path.is_empty() {
                        s3_uri.to_string()
                    } else {
                        format!("{}/{}", s3_uri.trim_end_matches('/'), path)
                    };

                    println!("{size_str} {display_path}");
                }
            }

            Ok(())
        }
        Err(e) => {
            // Record error using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;

                let error_msg = format!("Failed to calculate storage usage for {s3_uri}: {e}");
                OTEL_INSTRUMENTS.record_error_with_type(&error_msg);
            }

            Err(e)
        }
    }
}

#[derive(Debug)]
struct ObjectInfo {
    key: String,
    size: i64,
}

async fn scan_objects(
    config: &Config,
    bucket: &str,
    prefix: Option<&str>,
) -> Result<Vec<ObjectInfo>> {
    let start_time = Instant::now();
    let mut objects = Vec::new();
    let mut continuation_token: Option<String> = None;
    let mut page_count = 0;

    let result: Result<Vec<ObjectInfo>> = async {
        loop {
            let mut request = config.client.list_objects_v2().bucket(bucket);

            if let Some(prefix_val) = prefix {
                request = request.prefix(prefix_val);
            }

            if let Some(token) = continuation_token {
                request = request.continuation_token(token);
            }

            let response = request.send().await?;
            page_count += 1;

            if let Some(contents) = response.contents {
                for object in contents {
                    if let Some(key) = object.key {
                        let size = object.size.unwrap_or(0);
                        objects.push(ObjectInfo { key, size });
                    }
                }
            }

            if response.is_truncated.unwrap_or(false) {
                continuation_token = response.next_continuation_token;
            } else {
                break;
            }
        }

        Ok(objects)
    }
    .await;

    match result {
        Ok(objects) => {
            let duration = start_time.elapsed();

            // Record scan operation using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;
                use opentelemetry::KeyValue;

                OTEL_INSTRUMENTS
                    .operations_total
                    .add(1, &[KeyValue::new("operation", "scan_objects")]);

                let duration_seconds = duration.as_millis() as f64 / 1000.0;
                OTEL_INSTRUMENTS.operation_duration.record(
                    duration_seconds,
                    &[KeyValue::new("operation", "scan_objects")],
                );

                // Record pagination metrics
                OTEL_INSTRUMENTS.operations_total.add(
                    page_count,
                    &[KeyValue::new("operation", "list_objects_page")],
                );
            }

            Ok(objects)
        }
        Err(e) => {
            // Record error using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;

                let error_msg = format!("Failed to scan objects in bucket {bucket}: {e}");
                OTEL_INSTRUMENTS.record_error_with_type(&error_msg);
            }

            Err(e)
        }
    }
}

fn calculate_directory_sizes(
    objects: &[ObjectInfo],
    max_depth: Option<usize>,
) -> HashMap<String, i64> {
    let mut directory_sizes = HashMap::new();

    for object in objects {
        let mut current_path = String::new();
        let parts: Vec<&str> = object.key.split('/').collect();

        // Determine the maximum depth to process
        let depth_limit = max_depth.unwrap_or(parts.len());
        let actual_depth = std::cmp::min(depth_limit, parts.len());

        // Add size to root
        *directory_sizes.entry(String::new()).or_insert(0) += object.size;

        // Add size to each directory level up to the depth limit
        for i in 0..actual_depth {
            if i > 0 {
                current_path.push('/');
            }
            current_path.push_str(parts[i]);

            // Don't count the file itself as a directory if we're at the last part
            if i < parts.len() - 1 || !parts[i].contains('.') {
                *directory_sizes.entry(current_path.clone()).or_insert(0) += object.size;
            }
        }
    }

    directory_sizes
}

fn format_size_human_readable(size: i64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size_f = size as f64;
    let mut unit_index = 0;

    while size_f >= 1024.0 && unit_index < UNITS.len() - 1 {
        size_f /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", size, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size_f, UNITS[unit_index])
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

        let result = execute(&config, "/local/path", false, false, None).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("du command only works with S3 URIs"));
    }

    #[tokio::test]
    async fn test_execute_invalid_s3_uri() {
        let config = create_mock_config();

        let result = execute(
            &config, "s3://", // invalid S3 URI
            false, false, None,
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_valid_s3_uri() {
        let config = create_mock_config();

        let result = execute(&config, "s3://test-bucket/path/", false, false, None).await;

        // Will fail due to no AWS connection, but tests the routing
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_with_summarize() {
        let config = create_mock_config();

        let result = execute(&config, "s3://test-bucket", true, true, None).await;

        // Will fail due to no AWS connection, but tests the routing
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_with_max_depth() {
        let config = create_mock_config();

        let result = execute(
            &config,
            "s3://test-bucket/deep/path/",
            false,
            false,
            Some(2),
        )
        .await;

        // Will fail due to no AWS connection, but tests the routing
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_directory_sizes() {
        let objects = vec![
            ObjectInfo {
                key: "file1.txt".to_string(),
                size: 100,
            },
            ObjectInfo {
                key: "dir1/file2.txt".to_string(),
                size: 200,
            },
            ObjectInfo {
                key: "dir1/subdir/file3.txt".to_string(),
                size: 300,
            },
            ObjectInfo {
                key: "dir2/file4.txt".to_string(),
                size: 400,
            },
        ];

        let sizes = calculate_directory_sizes(&objects, None);

        // Root should contain all files
        assert_eq!(sizes.get(""), Some(&1000));

        // dir1 should contain file2.txt and subdir contents
        assert_eq!(sizes.get("dir1"), Some(&500));

        // dir2 should contain file4.txt
        assert_eq!(sizes.get("dir2"), Some(&400));

        // subdir should contain file3.txt
        assert_eq!(sizes.get("dir1/subdir"), Some(&300));
    }

    #[test]
    fn test_calculate_directory_sizes_with_max_depth() {
        let objects = vec![ObjectInfo {
            key: "dir1/subdir1/subdir2/file.txt".to_string(),
            size: 100,
        }];

        let sizes = calculate_directory_sizes(&objects, Some(2));

        // Should only go 2 levels deep
        assert_eq!(sizes.get(""), Some(&100));
        assert_eq!(sizes.get("dir1"), Some(&100));
        assert_eq!(sizes.get("dir1/subdir1"), Some(&100));
        assert!(!sizes.contains_key("dir1/subdir1/subdir2"));
    }

    #[test]
    fn test_format_size_human_readable() {
        assert_eq!(format_size_human_readable(0), "0 B");
        assert_eq!(format_size_human_readable(512), "512 B");
        assert_eq!(format_size_human_readable(1024), "1.0 KB");
        assert_eq!(format_size_human_readable(1536), "1.5 KB");
        assert_eq!(format_size_human_readable(1048576), "1.0 MB");
        assert_eq!(format_size_human_readable(1073741824), "1.0 GB");
        assert_eq!(format_size_human_readable(1099511627776), "1.0 TB");
        assert_eq!(format_size_human_readable(2199023255552), "2.0 TB");
    }

    #[test]
    fn test_format_size_edge_cases() {
        assert_eq!(format_size_human_readable(-1), "-1 B");
        assert_eq!(format_size_human_readable(1023), "1023 B");
        assert_eq!(format_size_human_readable(1025), "1.0 KB");

        // Test very large sizes
        let large_size = 1024_i64.pow(4); // 1 TB
        assert_eq!(format_size_human_readable(large_size), "1.0 TB");

        let very_large_size = 1024_i64.pow(5); // 1024 TB (beyond our units)
        assert_eq!(format_size_human_readable(very_large_size), "1024.0 TB");
    }

    #[test]
    fn test_object_info_debug() {
        let obj = ObjectInfo {
            key: "test.txt".to_string(),
            size: 1024,
        };

        let debug_str = format!("{obj:?}");
        assert!(debug_str.contains("test.txt"));
        assert!(debug_str.contains("1024"));
    }

    #[test]
    fn test_directory_sizes_empty_objects() {
        let objects = vec![];
        let sizes = calculate_directory_sizes(&objects, None);

        // Empty objects should result in empty HashMap
        assert_eq!(sizes.get(""), None);
        assert_eq!(sizes.len(), 0);
    }

    #[test]
    fn test_directory_sizes_single_file_in_root() {
        let objects = vec![ObjectInfo {
            key: "file.txt".to_string(),
            size: 100,
        }];

        let sizes = calculate_directory_sizes(&objects, None);

        // Root should contain the file
        assert_eq!(sizes.get(""), Some(&100));

        // Should only have one entry (root)
        assert_eq!(sizes.len(), 1);
    }

    #[test]
    fn test_s3_uri_validation() {
        // Test that we can distinguish valid from invalid URIs
        assert!(is_s3_uri("s3://bucket/key"));
        assert!(!is_s3_uri("/local/path"));
        assert!(!is_s3_uri("http://example.com"));
    }
}
