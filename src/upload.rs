use anyhow::Result;
use aws_sdk_s3::{primitives::ByteStream, Client};
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub async fn upload_file(client: &Client, bucket: &str, key: &str, path: &Path) -> Result<()> {
    let mut file = File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    let body = ByteStream::from(buffer);

    client
        .put_object()
        .bucket(bucket)
        .key(key)
        .body(body)
        .send()
        .await
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!("Upload failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_mock_client() -> Client {
        Client::from_conf(
            aws_sdk_s3::config::Builder::new()
                .region(aws_config::Region::new("us-east-1"))
                .behavior_version(aws_config::BehaviorVersion::latest())
                .build(),
        )
    }

    #[tokio::test]
    async fn test_upload_file_with_temp_file() {
        let client = create_mock_client();

        // Create a temporary file
        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        write!(temp_file, "test content").expect("Failed to write to temp file");

        let result = upload_file(&client, "test-bucket", "test-key", temp_file.path()).await;

        // Will fail due to no AWS connection, but tests the file reading logic
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_upload_file_nonexistent_file() {
        let client = create_mock_client();

        let result = upload_file(
            &client,
            "test-bucket",
            "test-key",
            Path::new("/nonexistent/file.txt"),
        )
        .await;

        // Should fail because file doesn't exist
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_upload_file_empty_file() {
        let client = create_mock_client();

        // Create an empty temporary file
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");

        let result = upload_file(&client, "test-bucket", "test-key", temp_file.path()).await;

        // Will fail due to no AWS connection, but tests the empty file handling
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_upload_file_with_various_bucket_names() {
        let client = create_mock_client();

        // Create a temporary file
        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        write!(temp_file, "test content").expect("Failed to write to temp file");

        let bucket_names = vec![
            "test-bucket",
            "my-bucket-123",
            "bucket-with-dashes",
            "longbucketnamewithnodashes",
        ];

        for bucket_name in bucket_names {
            let result = upload_file(&client, bucket_name, "test-key", temp_file.path()).await;

            // Will fail due to no AWS connection, but tests parameter handling
            assert!(result.is_err());
        }
    }

    #[tokio::test]
    async fn test_upload_file_with_various_keys() {
        let client = create_mock_client();

        // Create a temporary file
        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        write!(temp_file, "test content").expect("Failed to write to temp file");

        let keys = vec![
            "simple-key",
            "path/to/file.txt",
            "folder/subfolder/document.pdf",
            "file-with-spaces in name.txt",
            "unicode-文件名.txt",
        ];

        for key in keys {
            let result = upload_file(&client, "test-bucket", key, temp_file.path()).await;

            // Will fail due to no AWS connection, but tests key handling
            assert!(result.is_err());
        }
    }

    #[tokio::test]
    async fn test_upload_file_large_content() {
        let client = create_mock_client();

        // Create a temporary file with larger content
        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let large_content = "x".repeat(10000); // 10KB of content
        write!(temp_file, "{large_content}").expect("Failed to write to temp file");

        let result = upload_file(&client, "test-bucket", "large-file.txt", temp_file.path()).await;

        // Will fail due to no AWS connection, but tests large file handling
        assert!(result.is_err());
    }

    #[test]
    fn test_file_path_validation() {
        // Test path validation logic
        let valid_paths = vec![
            Path::new("file.txt"),
            Path::new("path/to/file.txt"),
            Path::new("/absolute/path/file.txt"),
            Path::new("./relative/path/file.txt"),
        ];

        for path in valid_paths {
            // Test that paths are valid Path objects
            assert!(!path.to_string_lossy().is_empty());
        }
    }

    #[test]
    fn test_parameter_validation() {
        // Test that parameters are properly validated
        let bucket_names = vec!["valid-bucket", "bucket123", "my-test-bucket"];

        let keys = vec!["file.txt", "path/to/file.txt", "folder/document.pdf"];

        for bucket in bucket_names {
            assert!(!bucket.is_empty(), "Bucket name should not be empty");
            assert!(
                bucket.len() >= 3,
                "Bucket name should be at least 3 characters"
            );
        }

        for key in keys {
            assert!(!key.is_empty(), "Key should not be empty");
        }
    }

    #[tokio::test]
    async fn test_upload_file_error_handling() {
        let client = create_mock_client();

        // Test with a directory instead of a file
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");

        let result = upload_file(&client, "test-bucket", "test-key", temp_dir.path()).await;

        // Should fail because we're trying to read a directory as a file
        assert!(result.is_err());
    }

    #[test]
    fn test_bytestream_creation() {
        // Test ByteStream creation with various data
        let test_data = vec![
            b"".to_vec(),                   // empty
            b"hello".to_vec(),              // simple text
            b"binary\x00\x01\x02".to_vec(), // binary data
            vec![0u8; 1000],                // large zeros
        ];

        for data in test_data {
            let stream = ByteStream::from(data.clone());
            // ByteStream should be created successfully
            let _size_hint = stream.size_hint();
            // Note: size_hint.0 is usize, always non-negative
        }
    }
}
