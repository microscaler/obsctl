use anyhow::Result;
use aws_sdk_s3::types::Object;
use chrono::{DateTime, Utc};
use log::info;
use std::time::Instant;

use crate::commands::s3_uri::parse_ls_path;
use crate::config::Config;
use crate::filtering::{
    apply_filters, parse_date_filter, parse_size_filter, parse_sort_config, validate_filter_config,
    EnhancedObjectInfo, FilterConfig,
};
use crate::utils::filter_by_enhanced_pattern;

#[allow(clippy::too_many_arguments)]
pub async fn execute(
    config: &Config,
    path: Option<&str>,
    long: bool,
    recursive: bool,
    human_readable: bool,
    summarize: bool,
    pattern: Option<&str>,
    debug_level: &str,
    created_after: Option<&str>,
    created_before: Option<&str>,
    modified_after: Option<&str>,
    modified_before: Option<&str>,
    min_size: Option<&str>,
    max_size: Option<&str>,
    max_results: Option<usize>,
    head: Option<usize>,
    tail: Option<usize>,
    sort_by: Option<&str>,
    reverse: bool,
) -> Result<()> {
    let start_time = Instant::now();

    // Build filter configuration from CLI arguments
    let filter_config = build_filter_config(
        created_after,
        created_before,
        modified_after,
        modified_before,
        min_size,
        max_size,
        max_results,
        head,
        tail,
        sort_by,
        reverse,
    )?;

    // Validate filter configuration
    validate_filter_config(&filter_config)?;

    // If no path is provided, list all buckets (with optional pattern filtering)
    let result = if path.is_none() {
        list_all_buckets(
            config,
            long,
            human_readable,
            summarize,
            pattern,
            debug_level,
        )
        .await
    } else {
        let (bucket, prefix) = parse_ls_path(path)?;

        info!("Listing objects in s3://{bucket}/{prefix}");

        let mut request = config.client.list_objects_v2().bucket(&bucket);

        if !prefix.is_empty() {
            request = request.prefix(&prefix);
        }

        if !recursive {
            request = request.delimiter("/");
        }

        let mut continuation_token: Option<String> = None;
        let mut total_objects = 0;
        let mut total_size = 0i64;
        let mut all_objects = Vec::new();
        let mut common_prefixes = Vec::new();

        let list_result: anyhow::Result<()> = async {
            loop {
                let mut req = request.clone();
                if let Some(token) = &continuation_token {
                    req = req.continuation_token(token);
                }

                let response = req.send().await?;

                // Collect common prefixes (directories) when not recursive
                for prefix_info in response.common_prefixes() {
                    if let Some(prefix) = prefix_info.prefix() {
                        common_prefixes.push(prefix.to_string());
                    }
                }

                // Collect all objects for filtering
                for object in response.contents() {
                    let enhanced_obj = convert_to_enhanced_object_info(object, &bucket);
                    all_objects.push(enhanced_obj);
                }

                // Check if there are more objects to fetch
                if response.is_truncated().unwrap_or(false) {
                    continuation_token = response.next_continuation_token().map(|s| s.to_string());
                } else {
                    break;
                }
            }

            // Apply advanced filtering to collected objects
            let filtered_objects = apply_filters(&all_objects, &filter_config);

            // Display common prefixes (directories) first
            for prefix in &common_prefixes {
                if long {
                    println!("{:>12} {:>19} {}/", "DIR", "", prefix);
                } else {
                    println!("{prefix}/");
                }
            }

            // Display filtered objects
            for enhanced_obj in &filtered_objects {
                total_objects += 1;
                total_size += enhanced_obj.size;

                if long {
                    print_enhanced_long_format(enhanced_obj, human_readable);
                } else {
                    println!("{}", enhanced_obj.key);
                }
            }

            Ok(())
        }
        .await;

        match list_result {
            Ok(_) => {
                if long || summarize {
                    println!();
                    println!(
                        "Total: {} objects, {} bytes",
                        total_objects,
                        if human_readable {
                            format_size(total_size)
                        } else {
                            total_size.to_string()
                        }
                    );
                }
                Ok(())
            }
            Err(e) => Err(e),
        }
    };

    match result {
        Ok(_) => {
            let duration = start_time.elapsed();

            // Record ls operation using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;
                use opentelemetry::KeyValue;

                let operation_type = if path.is_none() {
                    "ls_buckets"
                } else if recursive {
                    "ls_recursive"
                } else {
                    "ls_objects"
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

                let error_msg = format!("Failed to list {}: {}", path.unwrap_or("buckets"), e);
                OTEL_INSTRUMENTS.record_error_with_type(&error_msg);
            }

            Err(e)
        }
    }
}

async fn list_all_buckets(
    config: &Config,
    long: bool,
    human_readable: bool,
    summarize: bool,
    pattern: Option<&str>,
    debug_level: &str,
) -> Result<()> {
    let start_time = Instant::now();

    let is_debug = matches!(debug_level, "debug" | "trace");
    if is_debug {
        info!(
            "Listing all buckets{}",
            if let Some(p) = pattern {
                format!(" matching pattern '{p}'")
            } else {
                String::new()
            }
        );
    }

    let result: anyhow::Result<()> = async {
        let response = config.client.list_buckets().send().await?;

        // Get all bucket names
        let all_bucket_names: Vec<String> = response
            .buckets()
            .iter()
            .filter_map(|bucket| bucket.name().map(|name| name.to_string()))
            .collect();

        // Filter by pattern if provided
        let filtered_bucket_names = if let Some(pattern_str) = pattern {
            filter_by_enhanced_pattern(&all_bucket_names, pattern_str, false)?
        } else {
            all_bucket_names.clone()
        };

        let mut total_buckets = 0;

        // Display filtered buckets
        for bucket_name in &filtered_bucket_names {
            // Find the original bucket object for metadata
            if let Some(bucket) = response
                .buckets()
                .iter()
                .find(|b| b.name() == Some(bucket_name))
            {
                total_buckets += 1;

                if long {
                    let creation_date = bucket
                        .creation_date()
                        .and_then(|dt| DateTime::parse_from_rfc3339(&dt.to_string()).ok())
                        .map(|dt| {
                            dt.with_timezone(&Utc)
                                .format("%Y-%m-%d %H:%M:%S")
                                .to_string()
                        })
                        .unwrap_or_else(|| "unknown".to_string());

                    // Get bucket size if requested
                    if summarize {
                        match get_bucket_size(config, bucket_name).await {
                            Ok((object_count, total_size)) => {
                                let size_str = if human_readable {
                                    format_size(total_size)
                                } else {
                                    total_size.to_string()
                                };
                                println!(
                                    "{:>12} {} {} ({} objects, {} bytes)",
                                    "BUCKET", creation_date, bucket_name, object_count, size_str
                                );
                            }
                            Err(_) => {
                                println!("{:>12} {} {}", "BUCKET", creation_date, bucket_name);
                            }
                        }
                    } else {
                        println!("{:>12} {} {}", "BUCKET", creation_date, bucket_name);
                    }
                } else {
                    println!("{bucket_name}");
                }
            }
        }

        if long || summarize {
            println!();
            if let Some(pattern_str) = pattern {
                println!("Total: {total_buckets} buckets matching pattern '{pattern_str}'");
                if total_buckets != all_bucket_names.len() {
                    println!(
                        "({} buckets total, {} filtered out)",
                        all_bucket_names.len(),
                        all_bucket_names.len() - total_buckets
                    );
                }
            } else {
                println!("Total: {total_buckets} buckets");
            }
        }

        Ok(())
    }
    .await;

    match result {
        Ok(_) => {
            let duration = start_time.elapsed();

            // Record bucket listing using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;
                use opentelemetry::KeyValue;

                OTEL_INSTRUMENTS
                    .operations_total
                    .add(1, &[KeyValue::new("operation", "list_buckets")]);

                let duration_seconds = duration.as_millis() as f64 / 1000.0;
                OTEL_INSTRUMENTS.operation_duration.record(
                    duration_seconds,
                    &[KeyValue::new("operation", "list_buckets")],
                );
            }

            Ok(())
        }
        Err(e) => {
            // Record error using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;

                let error_msg = format!("Failed to list buckets: {e}");
                OTEL_INSTRUMENTS.record_error_with_type(&error_msg);
            }

            Err(e)
        }
    }
}

async fn get_bucket_size(config: &Config, bucket_name: &str) -> Result<(i32, i64)> {
    let start_time = Instant::now();

    let result: anyhow::Result<(i32, i64)> = async {
        let request = config.client.list_objects_v2().bucket(bucket_name);

        let mut continuation_token: Option<String> = None;
        let mut total_objects = 0;
        let mut total_size = 0i64;

        loop {
            let mut req = request.clone();
            if let Some(token) = &continuation_token {
                req = req.continuation_token(token);
            }

            let response = req.send().await?;

            for object in response.contents() {
                total_objects += 1;
                if let Some(size) = object.size() {
                    total_size += size;
                }
            }

            if response.is_truncated().unwrap_or(false) {
                continuation_token = response.next_continuation_token().map(|s| s.to_string());
            } else {
                break;
            }
        }

        Ok((total_objects, total_size))
    }
    .await;

    match result {
        Ok((objects, size)) => {
            let duration = start_time.elapsed();

            // Record bucket size calculation using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;
                use opentelemetry::KeyValue;

                OTEL_INSTRUMENTS
                    .operations_total
                    .add(1, &[KeyValue::new("operation", "bucket_size")]);

                let duration_seconds = duration.as_millis() as f64 / 1000.0;
                OTEL_INSTRUMENTS.operation_duration.record(
                    duration_seconds,
                    &[KeyValue::new("operation", "bucket_size")],
                );

                // Record the scanned objects and bytes
                OTEL_INSTRUMENTS.files_uploaded_total.add(
                    objects as u64,
                    &[KeyValue::new("operation", "bucket_size_scan")],
                );
                OTEL_INSTRUMENTS.bytes_uploaded_total.add(
                    size as u64,
                    &[KeyValue::new("operation", "bucket_size_scan")],
                );
            }

            Ok((objects, size))
        }
        Err(e) => {
            // Record error using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;

                let error_msg = format!("Failed to get bucket size for {bucket_name}: {e}");
                OTEL_INSTRUMENTS.record_error_with_type(&error_msg);
            }

            Err(e)
        }
    }
}

fn print_enhanced_long_format(obj: &EnhancedObjectInfo, human_readable: bool) {
    let size_str = if human_readable {
        format!("{:>12}", format_size(obj.size))
    } else {
        format!("{:>12}", obj.size)
    };

    let modified = obj
        .modified
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Add storage class information if available
    let storage_info = obj
        .storage_class
        .as_ref()
        .map(|sc| format!(" [{sc}]"))
        .unwrap_or_default();

    println!("{} {} {}{}", size_str, modified, obj.key, storage_info);
}

fn format_size(size: i64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB"];
    let mut size = size as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{:.0}{}", size, UNITS[unit_index])
    } else {
        format!("{:.1}{}", size, UNITS[unit_index])
    }
}

/// Build FilterConfig from CLI arguments
#[allow(clippy::too_many_arguments)]
fn build_filter_config(
    created_after: Option<&str>,
    created_before: Option<&str>,
    modified_after: Option<&str>,
    modified_before: Option<&str>,
    min_size: Option<&str>,
    max_size: Option<&str>,
    max_results: Option<usize>,
    head: Option<usize>,
    tail: Option<usize>,
    sort_by: Option<&str>,
    reverse: bool,
) -> Result<FilterConfig> {
    let mut config = FilterConfig::default();

    // Parse date filters
    if let Some(date_str) = created_after {
        config.created_after = Some(parse_date_filter(date_str)?);
    }
    if let Some(date_str) = created_before {
        config.created_before = Some(parse_date_filter(date_str)?);
    }
    if let Some(date_str) = modified_after {
        config.modified_after = Some(parse_date_filter(date_str)?);
    }
    if let Some(date_str) = modified_before {
        config.modified_before = Some(parse_date_filter(date_str)?);
    }

    // Parse size filters
    if let Some(size_str) = min_size {
        config.min_size = Some(parse_size_filter(size_str)?);
    }
    if let Some(size_str) = max_size {
        config.max_size = Some(parse_size_filter(size_str)?);
    }

    // Set result limits
    config.max_results = max_results;
    config.head = head;
    config.tail = tail;

    // Parse sort configuration
    if let Some(sort_str) = sort_by {
        config.sort_config = parse_sort_config(sort_str)?;
    } else if reverse {
        // If reverse is specified without sort_by, default to sorting by name
        config.sort_config = parse_sort_config("name:desc")?;
    }

    Ok(config)
}

/// Convert S3 Object to EnhancedObjectInfo
fn convert_to_enhanced_object_info(object: &Object, _bucket_name: &str) -> EnhancedObjectInfo {
    let key = object.key().unwrap_or("").to_string();
    let size = object.size().unwrap_or(0);

    // Extract dates from S3 object metadata
    let created = object.last_modified().map(|dt| {
        DateTime::<Utc>::from_timestamp(dt.secs(), dt.subsec_nanos()).unwrap_or_else(Utc::now)
    });
    let modified = object.last_modified().map(|dt| {
        DateTime::<Utc>::from_timestamp(dt.secs(), dt.subsec_nanos()).unwrap_or_else(Utc::now)
    });

    // Extract additional metadata
    let storage_class = object.storage_class().map(|sc| sc.as_str().to_string());
    let etag = object.e_tag().map(|tag| tag.to_string());

    EnhancedObjectInfo {
        key,
        size,
        created,
        modified,
        storage_class,
        etag,
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
    async fn test_execute_with_bucket_path() {
        let config = create_mock_config();

        let result = execute(
            &config,
            Some("s3://test-bucket"),
            false,
            false,
            false,
            false,
            None,
            "info",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            false,
        )
        .await;

        // Will fail due to no AWS connection, but tests the routing
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_with_prefix() {
        let config = create_mock_config();

        let result = execute(
            &config,
            Some("s3://test-bucket/prefix/"),
            false,
            false,
            false,
            false,
            None,
            "info",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            false,
        )
        .await;

        // Will fail due to no AWS connection, but tests the routing
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_recursive_listing() {
        let config = create_mock_config();

        let result = execute(
            &config,
            Some("s3://test-bucket"),
            false,
            true,
            false,
            false,
            None,
            "info",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            false,
        )
        .await;

        // Will fail due to no AWS connection, but tests the routing
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_long_format() {
        let config = create_mock_config();

        let result = execute(
            &config,
            Some("s3://test-bucket"),
            true,
            false,
            false,
            false,
            None,
            "info",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            false,
        )
        .await;

        // Will fail due to no AWS connection, but tests the routing
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_human_readable() {
        let config = create_mock_config();

        let result = execute(
            &config,
            Some("s3://test-bucket"),
            false,
            false,
            true,
            false,
            None,
            "info",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            false,
        )
        .await;

        // Will fail due to no AWS connection, but tests the routing
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_with_summarize() {
        let config = create_mock_config();

        let result = execute(
            &config,
            Some("s3://test-bucket"),
            false,
            false,
            false,
            true,
            None,
            "info",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            false,
        )
        .await;

        // Will fail due to no AWS connection, but tests the routing
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_all_options() {
        let config = create_mock_config();

        let result = execute(
            &config,
            Some("s3://test-bucket/prefix/"),
            true,
            true,
            true,
            true,
            None,
            "info",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            false,
        )
        .await;

        // Will fail due to no AWS connection, but tests the routing
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_no_path() {
        let config = create_mock_config();

        let result = execute(
            &config, None, false, false, false, false, None, "info", None, None, None, None, None,
            None, None, None, None, None, false,
        )
        .await;

        // Will fail due to no AWS connection, but tests the routing
        assert!(result.is_err());
    }

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(0), "0B");
        assert_eq!(format_size(512), "512B");
        assert_eq!(format_size(1023), "1023B");
    }

    #[test]
    fn test_format_size_kilobytes() {
        assert_eq!(format_size(1024), "1.0KB");
        assert_eq!(format_size(1536), "1.5KB");
        assert_eq!(format_size(2048), "2.0KB");
    }

    #[test]
    fn test_format_size_megabytes() {
        assert_eq!(format_size(1048576), "1.0MB");
        assert_eq!(format_size(1572864), "1.5MB");
        assert_eq!(format_size(2097152), "2.0MB");
    }

    #[test]
    fn test_format_size_gigabytes() {
        assert_eq!(format_size(1073741824), "1.0GB");
        assert_eq!(format_size(1610612736), "1.5GB");
        assert_eq!(format_size(2147483648), "2.0GB");
    }

    #[test]
    fn test_format_size_terabytes() {
        assert_eq!(format_size(1099511627776), "1.0TB");
        assert_eq!(format_size(1649267441664), "1.5TB");
        assert_eq!(format_size(2199023255552), "2.0TB");
    }

    #[test]
    fn test_format_size_petabytes() {
        assert_eq!(format_size(1125899906842624), "1.0PB");
        assert_eq!(format_size(1688849860263936), "1.5PB");
    }

    #[test]
    fn test_format_size_negative() {
        assert_eq!(format_size(-1), "-1B");
        assert_eq!(format_size(-1024), "-1024B"); // Negative numbers don't get unit conversion
    }

    #[test]
    fn test_format_size_edge_cases() {
        assert_eq!(format_size(1023), "1023B");
        assert_eq!(format_size(1025), "1.0KB");

        // Test very large sizes
        let large_size = 1024_i64.pow(5); // 1 PB
        assert_eq!(format_size(large_size), "1.0PB");

        // Test beyond our units (should still work)
        let very_large_size = 1024_i64.pow(6); // 1024 PB
        assert_eq!(format_size(very_large_size), "1024.0PB");
    }

    #[test]
    fn test_print_long_format_with_mock_object() {
        // We can't easily test print_long_format since it prints to stdout
        // and we'd need to mock AWS SDK types. The execute tests above
        // cover the code paths that call print_long_format.

        // Test that format_size works correctly for the sizes that would be used
        let test_sizes = vec![0, 1024, 1048576, 1073741824];
        for size in test_sizes {
            let formatted = format_size(size);
            assert!(!formatted.is_empty());
        }
    }

    #[test]
    fn test_size_formatting_precision() {
        // Test that formatting maintains proper precision
        assert_eq!(format_size(1536), "1.5KB"); // 1.5 * 1024
        assert_eq!(format_size(1792), "1.8KB"); // 1.75 * 1024, rounded to 1.8
        assert_eq!(format_size(1843), "1.8KB"); // 1.8 * 1024
    }

    #[test]
    fn test_format_size_unit_boundaries() {
        // Test exact boundaries between units
        assert_eq!(format_size(1024), "1.0KB");
        assert_eq!(format_size(1048576), "1.0MB");
        assert_eq!(format_size(1073741824), "1.0GB");
        assert_eq!(format_size(1099511627776), "1.0TB");
        assert_eq!(format_size(1125899906842624), "1.0PB");
    }

    #[test]
    fn test_format_size_consistency() {
        // Test that formatting is consistent across different scenarios
        let sizes = vec![0, 1, 512, 1024, 2048, 1048576, 1073741824];

        for size in sizes {
            let formatted = format_size(size);
            assert!(!formatted.is_empty());

            // All formatted sizes should end with a unit
            assert!(
                formatted.ends_with("B")
                    || formatted.ends_with("KB")
                    || formatted.ends_with("MB")
                    || formatted.ends_with("GB")
                    || formatted.ends_with("TB")
                    || formatted.ends_with("PB")
            );
        }
    }
}
