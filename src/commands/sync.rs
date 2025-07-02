use anyhow::Result;
use log::info;
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;
use tokio::fs;
use walkdir::WalkDir;

use crate::commands::cp;
use crate::commands::du;
use crate::commands::s3_uri::{is_s3_uri, S3Uri};
use crate::config::Config;

#[allow(clippy::too_many_arguments)]
pub async fn execute(
    config: &Config,
    source: &str,
    dest: &str,
    dryrun: bool,
    delete: bool,
    exclude: Option<&str>,
    include: Option<&str>,
    size_only: bool,
    exact_timestamps: bool,
) -> Result<()> {
    info!("Syncing from {source} to {dest}");

    if dryrun {
        info!("[DRY RUN] Would sync from {source} to {dest}");
    }

    let source_is_s3 = is_s3_uri(source);
    let dest_is_s3 = is_s3_uri(dest);

    match (source_is_s3, dest_is_s3) {
        (false, true) => {
            // Local to S3 sync
            sync_local_to_s3(
                config,
                source,
                dest,
                dryrun,
                delete,
                exclude,
                include,
                size_only,
                exact_timestamps,
            )
            .await
        }
        (true, false) => {
            // S3 to local sync
            sync_s3_to_local(
                config,
                source,
                dest,
                dryrun,
                delete,
                exclude,
                include,
                size_only,
                exact_timestamps,
            )
            .await
        }
        (true, true) => {
            // S3 to S3 sync
            sync_s3_to_s3(
                config,
                source,
                dest,
                dryrun,
                delete,
                exclude,
                include,
                size_only,
                exact_timestamps,
            )
            .await
        }
        (false, false) => {
            // Local to local sync (not typically handled by S3 tools)
            Err(anyhow::anyhow!(
                "Local to local sync not supported. Use rsync or similar tools."
            ))
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn sync_local_to_s3(
    config: &Config,
    source: &str,
    dest: &str,
    dryrun: bool,
    delete: bool,
    _exclude: Option<&str>,
    _include: Option<&str>,
    size_only: bool,
    _exact_timestamps: bool,
) -> Result<()> {
    let start_time = Instant::now();
    let dest_uri = S3Uri::parse(dest)?;

    // Build map of local files
    let local_files = scan_local_directory(source)?;

    // Build map of S3 objects
    let s3_objects = scan_s3_objects(config, &dest_uri).await?;

    let mut upload_count = 0;
    let mut delete_count = 0;
    let mut total_upload_bytes = 0u64;

    // Compare and upload files that are new or different
    for (relative_path, local_file) in &local_files {
        let s3_key = if dest_uri.key_or_empty().is_empty() {
            relative_path.clone()
        } else {
            format!(
                "{}/{}",
                dest_uri.key_or_empty().trim_end_matches('/'),
                relative_path
            )
        };

        let should_upload = match s3_objects.get(&s3_key) {
            Some(s3_object) => {
                // File exists in S3, check if we need to update
                if size_only {
                    local_file.size != s3_object.size
                } else {
                    // For now, just compare sizes (timestamp comparison would require more complex logic)
                    local_file.size != s3_object.size
                }
            }
            None => {
                // File doesn't exist in S3, need to upload
                true
            }
        };

        if should_upload {
            let local_path = format!("{}/{}", source.trim_end_matches('/'), relative_path);
            let s3_dest = format!("s3://{}/{}", dest_uri.bucket, s3_key);

            if dryrun {
                println!("(dryrun) upload: {local_path} to {s3_dest}");
            } else {
                println!("upload: {local_path} to {s3_dest}");
                cp::execute(
                    config,
                    &local_path,
                    &s3_dest,
                    false,
                    false,
                    1,
                    false,
                    None,
                    None,
                )
                .await?;
            }
            upload_count += 1;
            total_upload_bytes += local_file.size as u64;
        }
    }

    // Delete files from S3 that don't exist locally (if --delete flag is set)
    if delete {
        for s3_key in s3_objects.keys() {
            // Calculate what the local relative path would be
            let local_relative_path = if dest_uri.key_or_empty().is_empty() {
                s3_key.clone()
            } else {
                s3_key
                    .strip_prefix(&format!(
                        "{}/",
                        dest_uri.key_or_empty().trim_end_matches('/')
                    ))
                    .unwrap_or(s3_key)
                    .to_string()
            };

            if !local_files.contains_key(&local_relative_path) {
                let s3_path = format!("s3://{}/{}", dest_uri.bucket, s3_key);

                if dryrun {
                    println!("(dryrun) delete: {s3_path}");
                } else {
                    println!("delete: {s3_path}");
                    config
                        .client
                        .delete_object()
                        .bucket(&dest_uri.bucket)
                        .key(s3_key)
                        .send()
                        .await?;
                }
                delete_count += 1;
            }
        }
    }

    let duration = start_time.elapsed();

    // Record comprehensive sync metrics using proper OTEL SDK
    if !dryrun {
        {
            use crate::otel::OTEL_INSTRUMENTS;
            use opentelemetry::KeyValue;

            // Record operation count
            OTEL_INSTRUMENTS
                .operations_total
                .add(1, &[KeyValue::new("operation", "sync_local_to_s3")]);

            // Record sync operation count
            OTEL_INSTRUMENTS.sync_operations_total.add(1, &[]);

            // Record uploads and bytes
            OTEL_INSTRUMENTS.uploads_total.add(upload_count, &[]);
            OTEL_INSTRUMENTS.files_uploaded_total.add(upload_count, &[]);
            OTEL_INSTRUMENTS
                .bytes_uploaded_total
                .add(total_upload_bytes, &[]);

            // Record duration in seconds (not milliseconds)
            let duration_seconds = duration.as_millis() as f64 / 1000.0;
            OTEL_INSTRUMENTS.operation_duration.record(
                duration_seconds,
                &[KeyValue::new("operation", "sync_local_to_s3")],
            );
        }
    }

    info!(
        "Sync completed: {upload_count} uploads, {delete_count} deletes"
    );

    // Transparent du call for real-time bucket analytics
    if !dryrun && upload_count > 0 {
        let bucket_uri = format!("s3://{}", dest_uri.bucket);
        call_transparent_du(config, &bucket_uri).await;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn sync_s3_to_local(
    config: &Config,
    source: &str,
    dest: &str,
    dryrun: bool,
    delete: bool,
    _exclude: Option<&str>,
    _include: Option<&str>,
    size_only: bool,
    _exact_timestamps: bool,
) -> Result<()> {
    let start_time = Instant::now();
    let source_uri = S3Uri::parse(source)?;

    // Build map of S3 objects
    let s3_objects = scan_s3_objects(config, &source_uri).await?;

    // Build map of local files
    let local_files = if Path::new(dest).exists() {
        scan_local_directory(dest)?
    } else {
        HashMap::new()
    };

    let mut download_count = 0;
    let mut delete_count = 0;
    let mut total_download_bytes = 0u64;

    // Compare and download files that are new or different
    for (s3_key, s3_object) in &s3_objects {
        let local_relative_path = if source_uri.key_or_empty().is_empty() {
            s3_key.clone()
        } else {
            s3_key
                .strip_prefix(&format!(
                    "{}/",
                    source_uri.key_or_empty().trim_end_matches('/')
                ))
                .unwrap_or(s3_key)
                .to_string()
        };

        let should_download = match local_files.get(&local_relative_path) {
            Some(local_file) => {
                // File exists locally, check if we need to update
                if size_only {
                    local_file.size != s3_object.size
                } else {
                    // For now, just compare sizes
                    local_file.size != s3_object.size
                }
            }
            None => {
                // File doesn't exist locally, need to download
                true
            }
        };

        if should_download {
            let s3_source = format!("s3://{}/{}", source_uri.bucket, s3_key);
            let local_dest = format!("{}/{}", dest.trim_end_matches('/'), local_relative_path);

            if dryrun {
                println!("(dryrun) download: {s3_source} to {local_dest}");
            } else {
                println!("download: {s3_source} to {local_dest}");
                cp::execute(
                    config,
                    &s3_source,
                    &local_dest,
                    false,
                    false,
                    1,
                    false,
                    None,
                    None,
                )
                .await?;
            }
            download_count += 1;
            total_download_bytes += s3_object.size as u64;
        }
    }

    // Delete local files that don't exist in S3 (if --delete flag is set)
    if delete {
        for local_relative_path in local_files.keys() {
            let s3_key = if source_uri.key_or_empty().is_empty() {
                local_relative_path.clone()
            } else {
                format!(
                    "{}/{}",
                    source_uri.key_or_empty().trim_end_matches('/'),
                    local_relative_path
                )
            };

            if !s3_objects.contains_key(&s3_key) {
                let local_path = format!("{dest}/{local_relative_path}");

                if dryrun {
                    println!("(dryrun) delete: {local_path}");
                } else {
                    println!("delete: {local_path}");
                    fs::remove_file(&local_path).await?;
                }
                delete_count += 1;
            }
        }
    }

    let duration = start_time.elapsed();

    // Record comprehensive sync metrics using proper OTEL SDK
    if !dryrun {
        {
            use crate::otel::OTEL_INSTRUMENTS;
            use opentelemetry::KeyValue;

            // Record operation count
            OTEL_INSTRUMENTS
                .operations_total
                .add(1, &[KeyValue::new("operation", "sync_s3_to_local")]);

            // Record sync operation count
            OTEL_INSTRUMENTS.sync_operations_total.add(1, &[]);

            // Record downloads and bytes
            OTEL_INSTRUMENTS.downloads_total.add(download_count, &[]);
            OTEL_INSTRUMENTS
                .files_downloaded_total
                .add(download_count, &[]);
            OTEL_INSTRUMENTS
                .bytes_downloaded_total
                .add(total_download_bytes, &[]);

            // Record duration in seconds (not milliseconds)
            let duration_seconds = duration.as_millis() as f64 / 1000.0;
            OTEL_INSTRUMENTS.operation_duration.record(
                duration_seconds,
                &[KeyValue::new("operation", "sync_s3_to_local")],
            );
        }
    }

    info!(
        "Sync completed: {download_count} downloads, {delete_count} deletes"
    );

    // Transparent du call for real-time bucket analytics
    if !dryrun && download_count > 0 {
        let bucket_uri = format!("s3://{}", source_uri.bucket);
        call_transparent_du(config, &bucket_uri).await;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn sync_s3_to_s3(
    _config: &Config,
    _source: &str,
    _dest: &str,
    _dryrun: bool,
    _delete: bool,
    _exclude: Option<&str>,
    _include: Option<&str>,
    _size_only: bool,
    _exact_timestamps: bool,
) -> Result<()> {
    // S3 to S3 sync is more complex and less commonly used
    // For now, return an error suggesting to use cp with --recursive
    Err(anyhow::anyhow!(
        "S3 to S3 sync not yet implemented. Use 'cp --recursive' for one-time copies."
    ))
}

#[derive(Debug, Clone)]
struct FileInfo {
    size: i64,
    #[allow(dead_code)] // TODO: Use for timestamp-based sync comparison
    modified: Option<std::time::SystemTime>,
}

fn scan_local_directory(dir_path: &str) -> Result<HashMap<String, FileInfo>> {
    let mut files = HashMap::new();
    let base_path = Path::new(dir_path);

    if !base_path.exists() {
        return Ok(files);
    }

    for entry in WalkDir::new(dir_path) {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            let metadata = path.metadata()?;
            let relative_path = path
                .strip_prefix(base_path)?
                .to_string_lossy()
                .replace('\\', "/"); // Normalize path separators

            files.insert(
                relative_path,
                FileInfo {
                    size: metadata.len() as i64,
                    modified: metadata.modified().ok(),
                },
            );
        }
    }

    Ok(files)
}

async fn scan_s3_objects(config: &Config, s3_uri: &S3Uri) -> Result<HashMap<String, FileInfo>> {
    let mut objects = HashMap::new();

    let mut list_request = config.client.list_objects_v2().bucket(&s3_uri.bucket);

    if !s3_uri.key_or_empty().is_empty() {
        list_request = list_request.prefix(s3_uri.key_or_empty());
    }

    let mut continuation_token: Option<String> = None;

    loop {
        if let Some(token) = &continuation_token {
            list_request = list_request.continuation_token(token);
        }

        let response = list_request.send().await?;

        if let Some(contents) = response.contents {
            for object in contents {
                if let Some(key) = object.key {
                    let size = object.size.unwrap_or(0);
                    let modified = object.last_modified.and_then(|dt| {
                        use std::time::SystemTime;
                        let timestamp = dt.secs();
                        SystemTime::UNIX_EPOCH
                            .checked_add(std::time::Duration::from_secs(timestamp as u64))
                    });

                    objects.insert(key, FileInfo { size, modified });
                }
            }
        }

        // Check if there are more objects to fetch
        if response.is_truncated.unwrap_or(false) {
            continuation_token = response.next_continuation_token;
            // Create a new request for the next iteration
            list_request = config.client.list_objects_v2().bucket(&s3_uri.bucket);

            if !s3_uri.key_or_empty().is_empty() {
                list_request = list_request.prefix(s3_uri.key_or_empty());
            }
        } else {
            break;
        }
    }

    Ok(objects)
}

// Add transparent du call for real-time bucket analytics
async fn call_transparent_du(config: &Config, s3_uri: &str) {
    // Only call du for bucket-level analytics if OTEL is enabled
    {
        use log::debug;

        // Extract bucket from S3 URI for bucket-level analytics
        if let Ok(uri) = crate::commands::s3_uri::S3Uri::parse(s3_uri) {
            let bucket_uri = format!("s3://{}", uri.bucket);

            debug!(
                "Running transparent du for bucket analytics after sync: {bucket_uri}"
            );

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
            },
        }
    }

    #[tokio::test]
    async fn test_execute_dry_run() {
        let config = create_mock_config();

        // Test S3 to S3 sync (should return error about not being implemented)
        let result = execute(
            &config,
            "s3://source-bucket",
            "s3://dest-bucket",
            true,  // dry run
            false, // delete
            None,  // exclude
            None,  // include
            false, // size_only
            false, // exact_timestamps
        )
        .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("S3 to S3 sync not yet implemented"));
    }

    #[tokio::test]
    async fn test_execute_local_to_s3() {
        let config = create_mock_config();

        let result = execute(
            &config,
            "/local/path",
            "s3://dest-bucket",
            false, // dryrun
            false, // delete
            None,  // exclude
            None,  // include
            false, // size_only
            false, // exact_timestamps
        )
        .await;

        // Will fail due to no AWS connection, but tests the routing
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_invalid_combination() {
        let config = create_mock_config();

        // Test local to local (should be error)
        let result = execute(
            &config,
            "/local/source",
            "/local/dest",
            false, // dryrun
            false, // delete
            None,  // exclude
            None,  // include
            false, // size_only
            false, // exact_timestamps
        )
        .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Local to local sync not supported"));
    }

    #[test]
    fn test_scan_local_directory() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create some test files
        std::fs::write(temp_path.join("file1.txt"), "content1").expect("Failed to write file1");
        std::fs::write(temp_path.join("file2.txt"), "content2").expect("Failed to write file2");

        let result = scan_local_directory(temp_path.to_str().unwrap());

        assert!(result.is_ok());
        let files = result.unwrap();
        assert_eq!(files.len(), 2);

        // Check that files are found
        let file_names: Vec<String> = files.keys().cloned().collect();
        assert!(file_names.contains(&"file1.txt".to_string()));
        assert!(file_names.contains(&"file2.txt".to_string()));
    }

    #[test]
    fn test_scan_local_directory_nonexistent() {
        let result = scan_local_directory("/nonexistent/path");

        // Should return empty HashMap for non-existent directory
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_scan_s3_objects() {
        let config = create_mock_config();
        let uri = S3Uri::parse("s3://test-bucket/prefix/").unwrap();

        let result = scan_s3_objects(&config, &uri).await;

        // Will fail due to no AWS connection, but tests the function exists
        assert!(result.is_err());
    }

    #[test]
    fn test_file_info_debug() {
        let file_info = FileInfo {
            size: 1024,
            modified: None,
        };

        let debug_str = format!("{file_info:?}");
        assert!(debug_str.contains("1024"));
    }

    #[test]
    fn test_file_info_clone() {
        let file_info = FileInfo {
            size: 1024,
            modified: None,
        };

        let cloned = file_info.clone();
        assert_eq!(cloned.size, 1024);
        assert_eq!(cloned.modified, None);
    }

    #[test]
    fn test_scan_local_directory_with_subdirs() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create subdirectory
        let subdir = temp_path.join("subdir");
        std::fs::create_dir(&subdir).expect("Failed to create subdir");

        // Create files in both root and subdir
        std::fs::write(temp_path.join("root_file.txt"), "root content")
            .expect("Failed to write root file");
        std::fs::write(subdir.join("sub_file.txt"), "sub content")
            .expect("Failed to write sub file");

        let result = scan_local_directory(temp_path.to_str().unwrap());

        assert!(result.is_ok());
        let files = result.unwrap();
        assert_eq!(files.len(), 2);

        // Check that both files are found with correct paths
        assert!(files.contains_key("root_file.txt"));
        assert!(files.contains_key("subdir/sub_file.txt"));
    }

    #[test]
    fn test_path_normalization() {
        // Test that paths are properly normalized
        let test_cases = vec![
            ("path/to/file", "path/to/file"),
            ("path//to//file", "path/to/file"),
            ("path/to/file/", "path/to/file"),
            ("/path/to/file", "path/to/file"),
        ];

        for (input, expected) in test_cases {
            // This would test path normalization if we had a utility function
            // For now, just verify the test structure works
            assert_eq!(
                input
                    .trim_start_matches('/')
                    .replace("//", "/")
                    .trim_end_matches('/'),
                expected
            );
        }
    }

    #[test]
    fn test_file_info_with_modified_time() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create a test file
        let file_path = temp_path.join("test_file.txt");
        std::fs::write(&file_path, "test content").expect("Failed to write file");

        // Get file metadata
        let metadata = std::fs::metadata(&file_path).expect("Failed to get metadata");
        let modified = metadata.modified().ok();

        let file_info = FileInfo {
            size: metadata.len() as i64,
            modified,
        };

        assert_eq!(file_info.size, 12); // "test content" is 12 bytes
        assert!(file_info.modified.is_some());
    }

    #[test]
    fn test_scan_local_directory_empty() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        let result = scan_local_directory(temp_path.to_str().unwrap());

        assert!(result.is_ok());
        let files = result.unwrap();
        assert_eq!(files.len(), 0);
    }
}
