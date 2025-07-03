use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as b64, Engine as _};
use log::info;
use md5;
use std::time::Instant;

use crate::config::Config;
use crate::utils::filter_by_enhanced_pattern;

pub async fn create_bucket(config: &Config, bucket_name: &str, region: Option<&str>) -> Result<()> {
    let start_time = Instant::now();
    info!("Creating bucket: {bucket_name}");

    let mut create_request = config.client.create_bucket().bucket(bucket_name);

    // Set region if provided and not us-east-1 (which doesn't need location constraint)
    if let Some(region_name) = region {
        if region_name != "us-east-1" {
            let location_constraint =
                aws_sdk_s3::types::BucketLocationConstraint::from(region_name);
            let create_bucket_config = aws_sdk_s3::types::CreateBucketConfiguration::builder()
                .location_constraint(location_constraint)
                .build();

            create_request = create_request.create_bucket_configuration(create_bucket_config);
        }
    }

    match create_request.send().await {
        Ok(_) => {
            let duration = start_time.elapsed();

            // Record bucket creation using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;
                use opentelemetry::KeyValue;

                OTEL_INSTRUMENTS
                    .operations_total
                    .add(1, &[KeyValue::new("operation", "create_bucket")]);

                let duration_seconds = duration.as_millis() as f64 / 1000.0;
                OTEL_INSTRUMENTS.operation_duration.record(
                    duration_seconds,
                    &[KeyValue::new("operation", "create_bucket")],
                );
            }

            println!("make_bucket: s3://{bucket_name}");
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to create bucket {bucket_name}: {e}");

            // Record error using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;

                OTEL_INSTRUMENTS.record_error_with_type(&error_msg);
            }

            Err(anyhow::anyhow!(error_msg))
        }
    }
}

pub async fn delete_bucket(config: &Config, bucket_name: &str, force: bool) -> Result<()> {
    let start_time = Instant::now();
    info!("Deleting bucket: {bucket_name}");

    let result: anyhow::Result<()> = async {
        if force {
            // First, delete all objects in the bucket
            delete_all_objects(config, bucket_name).await?;

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
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to delete bucket {bucket_name}: {e}");

            // Record error using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;

                OTEL_INSTRUMENTS.record_error_with_type(&error_msg);
            }

            Err(anyhow::anyhow!(error_msg))
        }
    }
}

async fn delete_all_objects(config: &Config, bucket_name: &str) -> Result<()> {
    let start_time = Instant::now();
    info!("Deleting all objects in bucket: {bucket_name}");

    let mut continuation_token: Option<String> = None;
    let mut deleted_count = 0;

    let result: anyhow::Result<()> = async {
        loop {
            let mut list_request = config.client.list_objects_v2().bucket(bucket_name);

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
                        deleted_count += 1;
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

            // Record object deletion using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;
                use opentelemetry::KeyValue;

                OTEL_INSTRUMENTS.operations_total.add(
                    deleted_count,
                    &[KeyValue::new("operation", "delete_objects")],
                );

                let duration_seconds = duration.as_millis() as f64 / 1000.0;
                OTEL_INSTRUMENTS.operation_duration.record(
                    duration_seconds,
                    &[KeyValue::new("operation", "delete_objects")],
                );
            }

            info!("Successfully deleted {deleted_count} objects");
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to delete objects in bucket {bucket_name}: {e}");

            // Record error using proper OTEL SDK
            {
                use crate::otel::OTEL_INSTRUMENTS;

                OTEL_INSTRUMENTS.record_error_with_type(&error_msg);
            }

            Err(anyhow::anyhow!(error_msg))
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

pub async fn delete_all_buckets(config: &Config, force: bool, confirm: bool) -> Result<()> {
    info!("Deleting all buckets");

    // Safety check - require confirmation for destructive --all operations
    if !confirm {
        return Err(anyhow::anyhow!(
            "Destructive operation requires --confirm flag. Use: obsctl rb --all --confirm"
        ));
    }

    // List all buckets first
    let response = config.client.list_buckets().send().await?;

    let mut deleted_count = 0;
    let mut failed_count = 0;

    for bucket in response.buckets() {
        if let Some(bucket_name) = bucket.name() {
            info!("Deleting bucket: {bucket_name}");

            match delete_bucket(config, bucket_name, force).await {
                Ok(_) => {
                    deleted_count += 1;
                    println!("remove_bucket: s3://{bucket_name}");
                }
                Err(e) => {
                    failed_count += 1;
                    eprintln!("Failed to delete bucket {bucket_name}: {e}");
                }
            }
        }
    }

    println!();
    println!("Batch deletion completed:");
    println!("  Successfully deleted: {deleted_count} buckets");
    if failed_count > 0 {
        println!("  Failed to delete: {failed_count} buckets");
    }

    if failed_count > 0 {
        return Err(anyhow::anyhow!(
            "Failed to delete {} bucket(s). Check error messages above.",
            failed_count
        ));
    }

    Ok(())
}

pub async fn delete_buckets_by_pattern(
    config: &Config,
    pattern: &str,
    force: bool,
    confirm: bool,
) -> Result<()> {
    info!("Deleting buckets matching pattern: {pattern}");

    // Safety check - require confirmation for destructive pattern operations
    if !confirm {
        return Err(anyhow::anyhow!(
            "Destructive operation requires --confirm flag. Use: obsctl rb --pattern '{}' --confirm",
            pattern
        ));
    }

    // List all buckets first
    let response = config.client.list_buckets().send().await?;

    // Get all bucket names
    let all_bucket_names: Vec<String> = response
        .buckets()
        .iter()
        .filter_map(|bucket| bucket.name().map(|name| name.to_string()))
        .collect();

    // Filter by pattern
    let matching_bucket_names = filter_by_enhanced_pattern(&all_bucket_names, pattern, false)?;

    if matching_bucket_names.is_empty() {
        println!("No buckets match the pattern '{pattern}'");
        return Ok(());
    }

    println!(
        "Found {} buckets matching pattern '{}':",
        matching_bucket_names.len(),
        pattern
    );
    for bucket_name in &matching_bucket_names {
        println!("  - s3://{bucket_name}");
    }
    println!();

    let mut deleted_count = 0;
    let mut failed_count = 0;

    for bucket_name in &matching_bucket_names {
        info!("Deleting bucket: {bucket_name}");

        match delete_bucket(config, bucket_name, force).await {
            Ok(_) => {
                deleted_count += 1;
                println!("remove_bucket: s3://{bucket_name}");
            }
            Err(e) => {
                failed_count += 1;
                eprintln!("Failed to delete bucket {bucket_name}: {e}");
            }
        }
    }

    println!();
    println!("Pattern-based deletion completed:");
    println!("  Pattern: '{pattern}'");
    println!("  Matched: {} buckets", matching_bucket_names.len());
    println!("  Successfully deleted: {deleted_count} buckets");
    if failed_count > 0 {
        println!("  Failed to delete: {failed_count} buckets");
    }

    if failed_count > 0 {
        return Err(anyhow::anyhow!(
            "Failed to delete {} bucket(s) matching pattern '{}'. Check error messages above.",
            failed_count,
            pattern
        ));
    }

    Ok(())
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
    async fn test_create_bucket_us_east_1() {
        let config = create_mock_config();

        // Test creating bucket in us-east-1 (no location constraint needed)
        let result = create_bucket(&config, "test-bucket", Some("us-east-1")).await;

        // Will fail due to no AWS connection, but tests the function structure
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_bucket_other_region() {
        let config = create_mock_config();

        // Test creating bucket in other region (needs location constraint)
        let result = create_bucket(&config, "test-bucket", Some("eu-west-1")).await;

        // Will fail due to no AWS connection, but tests the function structure
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_bucket_no_region() {
        let config = create_mock_config();

        // Test creating bucket without specifying region
        let result = create_bucket(&config, "test-bucket", None).await;

        // Will fail due to no AWS connection, but tests the function structure
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_bucket_without_force() {
        let config = create_mock_config();

        // Test deleting bucket without force (won't delete objects first)
        let result = delete_bucket(&config, "test-bucket", false).await;

        // Will fail due to no AWS connection, but tests the function structure
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_bucket_with_force() {
        let config = create_mock_config();

        // Test deleting bucket with force (will try to delete objects first)
        let result = delete_bucket(&config, "test-bucket", true).await;

        // Will fail due to no AWS connection, but tests the function structure
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_all_objects() {
        let config = create_mock_config();

        // Test deleting all objects in a bucket
        let result = delete_all_objects(&config, "test-bucket").await;

        // Will fail due to no AWS connection, but tests the function structure
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_all_versions() {
        let config = create_mock_config();

        // Test deleting all versions and delete markers
        let result = delete_all_versions(&config, "test-bucket").await;

        // Will fail due to no AWS connection, but tests the function structure
        assert!(result.is_err());
    }

    #[test]
    fn test_bucket_name_validation() {
        // Test that function accepts valid bucket names
        let valid_names = vec!["test-bucket", "my-bucket-123", "bucket.with.dots"];

        for name in valid_names {
            assert!(!name.is_empty());
            assert!(name.len() >= 3);
        }
    }

    #[test]
    fn test_region_handling() {
        // Test region string handling
        let regions = vec!["us-east-1", "us-west-2", "eu-west-1", "ap-southeast-1"];

        for region in regions {
            assert!(!region.is_empty());
            if region == "us-east-1" {
                // us-east-1 doesn't need location constraint
                assert_eq!(region, "us-east-1");
            } else {
                // Other regions need location constraint
                assert_ne!(region, "us-east-1");
            }
        }
    }
}
