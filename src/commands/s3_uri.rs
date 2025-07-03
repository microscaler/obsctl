use anyhow::{anyhow, Result};
use std::fmt;

/// Represents a parsed S3 URI
#[derive(Debug, Clone)]
pub struct S3Uri {
    pub bucket: String,
    pub key: Option<String>,
}

impl fmt::Display for S3Uri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.key {
            Some(key) => write!(f, "s3://{}/{}", self.bucket, key),
            None => write!(f, "s3://{}", self.bucket),
        }
    }
}

impl S3Uri {
    /// Parse an S3 URI in the format s3://bucket/key or s3://bucket
    pub fn parse(uri: &str) -> Result<Self> {
        if !uri.starts_with("s3://") {
            return Err(anyhow!("S3 URI must start with 's3://', got: {}", uri));
        }

        let without_scheme = &uri[5..]; // Remove "s3://"

        if without_scheme.is_empty() {
            return Err(anyhow!("S3 URI cannot be empty after 's3://'"));
        }

        let parts: Vec<&str> = without_scheme.splitn(2, '/').collect();
        let bucket = parts[0].to_string();

        if bucket.is_empty() {
            return Err(anyhow!("Bucket name cannot be empty"));
        }

        let key = if parts.len() > 1 && !parts[1].is_empty() {
            Some(parts[1].to_string())
        } else {
            None
        };

        Ok(S3Uri { bucket, key })
    }

    /// Get the key with a default empty string if None
    pub fn key_or_empty(&self) -> &str {
        self.key.as_deref().unwrap_or("")
    }
}

/// Check if a path is an S3 URI
pub fn is_s3_uri(path: &str) -> bool {
    path.starts_with("s3://")
}

/// Parse either a bucket name or full S3 URI for ls command compatibility
pub fn parse_ls_path(path: Option<&str>) -> Result<(String, String)> {
    match path {
        Some(path) => {
            if is_s3_uri(path) {
                let s3_uri = S3Uri::parse(path)?;
                let bucket = s3_uri.bucket.clone();
                let key = s3_uri.key_or_empty().to_string();
                Ok((bucket, key))
            } else {
                // Treat as bucket name for backwards compatibility
                Ok((path.to_string(), String::new()))
            }
        }
        None => {
            // List all buckets (not implemented yet, but this is the AWS CLI behavior)
            Err(anyhow!("Listing all buckets not yet implemented. Please specify a bucket: s3://bucket-name"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_s3_uri() {
        // Test bucket only
        let uri = S3Uri::parse("s3://my-bucket").unwrap();
        assert_eq!(uri.bucket, "my-bucket");
        assert_eq!(uri.key, None);

        // Test bucket with key
        let uri = S3Uri::parse("s3://my-bucket/path/to/file.txt").unwrap();
        assert_eq!(uri.bucket, "my-bucket");
        assert_eq!(uri.key, Some("path/to/file.txt".to_string()));

        // Test bucket with trailing slash
        let uri = S3Uri::parse("s3://my-bucket/").unwrap();
        assert_eq!(uri.bucket, "my-bucket");
        assert_eq!(uri.key, None);
    }

    #[test]
    fn test_invalid_s3_uri() {
        assert!(S3Uri::parse("http://bucket").is_err());
        assert!(S3Uri::parse("s3://").is_err());
        assert!(S3Uri::parse("bucket").is_err());
    }

    #[test]
    fn test_to_string() {
        let uri = S3Uri {
            bucket: "my-bucket".to_string(),
            key: Some("path/file.txt".to_string()),
        };
        assert_eq!(uri.to_string(), "s3://my-bucket/path/file.txt");

        let uri = S3Uri {
            bucket: "my-bucket".to_string(),
            key: None,
        };
        assert_eq!(uri.to_string(), "s3://my-bucket");
    }

    #[test]
    fn test_key_or_empty() {
        let uri_with_key = S3Uri {
            bucket: "bucket".to_string(),
            key: Some("path/file.txt".to_string()),
        };
        assert_eq!(uri_with_key.key_or_empty(), "path/file.txt");

        let uri_without_key = S3Uri {
            bucket: "bucket".to_string(),
            key: None,
        };
        assert_eq!(uri_without_key.key_or_empty(), "");
    }

    #[test]
    fn test_is_s3_uri() {
        assert!(is_s3_uri("s3://bucket"));
        assert!(is_s3_uri("s3://bucket/key"));
        assert!(is_s3_uri("s3://bucket/path/to/file"));

        assert!(!is_s3_uri("http://bucket"));
        assert!(!is_s3_uri("https://bucket"));
        assert!(!is_s3_uri("bucket"));
        assert!(!is_s3_uri("./local/path"));
        assert!(!is_s3_uri(""));
    }

    #[test]
    fn test_parse_ls_path_with_s3_uri() {
        // Test with full S3 URI
        let result = parse_ls_path(Some("s3://my-bucket/path")).unwrap();
        assert_eq!(result.0, "my-bucket");
        assert_eq!(result.1, "path");

        // Test with bucket only S3 URI
        let result = parse_ls_path(Some("s3://my-bucket")).unwrap();
        assert_eq!(result.0, "my-bucket");
        assert_eq!(result.1, "");
    }

    #[test]
    fn test_parse_ls_path_with_bucket_name() {
        // Test with plain bucket name (backwards compatibility)
        let result = parse_ls_path(Some("my-bucket")).unwrap();
        assert_eq!(result.0, "my-bucket");
        assert_eq!(result.1, "");
    }

    #[test]
    fn test_parse_ls_path_with_none() {
        // Test with None (should error)
        let result = parse_ls_path(None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Listing all buckets not yet implemented"));
    }

    #[test]
    fn test_parse_s3_uri_edge_cases() {
        // Test with complex paths
        let uri = S3Uri::parse("s3://bucket/path/with/many/segments/file.txt").unwrap();
        assert_eq!(uri.bucket, "bucket");
        assert_eq!(
            uri.key,
            Some("path/with/many/segments/file.txt".to_string())
        );

        // Test with special characters in key
        let uri = S3Uri::parse("s3://bucket/path with spaces/file-name_123.txt").unwrap();
        assert_eq!(uri.bucket, "bucket");
        assert_eq!(
            uri.key,
            Some("path with spaces/file-name_123.txt".to_string())
        );

        // Test with numbers in bucket name
        let uri = S3Uri::parse("s3://bucket-123/file").unwrap();
        assert_eq!(uri.bucket, "bucket-123");
        assert_eq!(uri.key, Some("file".to_string()));
    }

    #[test]
    fn test_parse_s3_uri_error_cases() {
        // Test various invalid formats
        assert!(S3Uri::parse("").is_err());
        assert!(S3Uri::parse("s3:").is_err());
        assert!(S3Uri::parse("s3:/").is_err());
        assert!(S3Uri::parse("s3://").is_err());
        assert!(S3Uri::parse("http://bucket").is_err());
        assert!(S3Uri::parse("https://bucket").is_err());
        assert!(S3Uri::parse("ftp://bucket").is_err());
        assert!(S3Uri::parse("bucket").is_err());
        assert!(S3Uri::parse("./bucket").is_err());
        assert!(S3Uri::parse("/bucket").is_err());
    }

    #[test]
    fn test_s3_uri_clone() {
        let original = S3Uri {
            bucket: "bucket".to_string(),
            key: Some("key".to_string()),
        };

        let cloned = original.clone();
        assert_eq!(original.bucket, cloned.bucket);
        assert_eq!(original.key, cloned.key);
    }

    #[test]
    fn test_s3_uri_debug() {
        let uri = S3Uri {
            bucket: "bucket".to_string(),
            key: Some("key".to_string()),
        };

        let debug_str = format!("{uri:?}");
        assert!(debug_str.contains("bucket"));
        assert!(debug_str.contains("key"));
    }

    #[test]
    fn test_parse_ls_path_with_invalid_s3_uri() {
        // Test with invalid S3 URI
        let result = parse_ls_path(Some("s3://"));
        assert!(result.is_err());
    }

    #[test]
    fn test_is_s3_uri_edge_cases() {
        // Test edge cases for is_s3_uri
        assert!(!is_s3_uri("s3:/"));
        assert!(!is_s3_uri("s3:"));
        assert!(!is_s3_uri("s3"));
        assert!(!is_s3_uri("S3://bucket")); // Case sensitive
        assert!(is_s3_uri("s3://"));
    }
}
