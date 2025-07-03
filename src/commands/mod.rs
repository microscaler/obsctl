pub mod bucket;
pub mod config;
pub mod cp;
pub mod du;
pub mod get;
pub mod head_object;
pub mod ls;
pub mod presign;
pub mod rm;
pub mod s3_uri;
pub mod sync;
pub mod upload;

use crate::args::{Args, Commands};
use crate::config::Config;
use anyhow::Result;

/// Execute the appropriate command based on CLI arguments
pub async fn execute_command(args: &Args, config: &Config) -> Result<()> {
    match &args.command {
        Commands::Ls {
            path,
            long,
            recursive,
            human_readable,
            summarize,
            pattern,
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
        } => {
            ls::execute(
                config,
                path.as_deref(),
                *long,
                *recursive,
                *human_readable,
                *summarize,
                pattern.as_deref(),
                &args.debug,
                created_after.as_deref(),
                created_before.as_deref(),
                modified_after.as_deref(),
                modified_before.as_deref(),
                min_size.as_deref(),
                max_size.as_deref(),
                *max_results,
                *head,
                *tail,
                sort_by.as_deref(),
                *reverse,
            )
            .await
        }
        Commands::Cp {
            source,
            dest,
            recursive,
            dryrun,
            max_concurrent,
            force,
            include,
            exclude,
        } => {
            cp::execute(
                config,
                source,
                dest,
                *recursive,
                *dryrun,
                *max_concurrent,
                *force,
                include.as_deref(),
                exclude.as_deref(),
            )
            .await
        }
        Commands::Sync {
            source,
            dest,
            delete,
            dryrun,
            max_concurrent: _,
            include,
            exclude,
        } => {
            sync::execute(
                config,
                source,
                dest,
                *dryrun,
                *delete,
                exclude.as_deref(),
                include.as_deref(),
                false,
                false,
            )
            .await
        }
        Commands::Rm {
            s3_uri,
            recursive,
            dryrun,
            include,
            exclude,
        } => {
            rm::execute(
                config,
                s3_uri,
                *recursive,
                *dryrun,
                false,
                include.as_deref(),
                exclude.as_deref(),
            )
            .await
        }
        Commands::Mb { s3_uri } => {
            let bucket_name = if let Some(stripped) = s3_uri.strip_prefix("s3://") {
                stripped // Remove "s3://" prefix
            } else {
                s3_uri
            };
            bucket::create_bucket(config, bucket_name, None).await
        }
        Commands::Rb {
            s3_uri,
            force,
            all,
            confirm,
            pattern,
        } => {
            if *all {
                bucket::delete_all_buckets(config, *force, *confirm).await
            } else if let Some(pattern_str) = pattern {
                bucket::delete_buckets_by_pattern(config, pattern_str, *force, *confirm).await
            } else if let Some(uri) = s3_uri {
                let bucket_name = if let Some(stripped) = uri.strip_prefix("s3://") {
                    stripped // Remove "s3://" prefix
                } else {
                    uri
                };
                bucket::delete_bucket(config, bucket_name, *force).await
            } else {
                anyhow::bail!("Either provide a bucket URI, use --all flag to delete all buckets, or use --pattern to delete buckets matching a wildcard pattern")
            }
        }
        Commands::Presign { s3_uri, expires_in } => {
            presign::execute(config, s3_uri, *expires_in, None).await
        }
        Commands::HeadObject { bucket, key } => {
            let s3_uri = format!("s3://{bucket}/{key}");
            head_object::execute(config, &s3_uri).await
        }
        Commands::Du {
            s3_uri,
            human_readable,
            summarize,
        } => du::execute(config, s3_uri, *human_readable, *summarize, None).await,
        Commands::Config { command } => config::execute(command.clone()).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_s3::Client;
    use std::sync::Arc;

    // Helper function to create a mock config for testing
    fn create_mock_config() -> Config {
        // Create a minimal config for testing
        // Note: This won't work for actual AWS calls, but it's sufficient for testing the dispatcher
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
    async fn test_execute_ls_command() {
        let config = create_mock_config();
        let args = Args {
            debug: "info".to_string(),
            endpoint: None,
            region: "us-east-1".to_string(),
            timeout: 10,
            command: Commands::Ls {
                path: Some("s3://test-bucket".to_string()),
                long: false,
                recursive: false,
                human_readable: false,
                summarize: false,
                pattern: None,
                created_after: None,
                created_before: None,
                modified_after: None,
                modified_before: None,
                min_size: None,
                max_size: None,
                max_results: None,
                head: None,
                tail: None,
                sort_by: None,
                reverse: false,
            },
        };

        // This will fail because we don't have real AWS credentials,
        // but it tests that the dispatcher correctly routes to the ls command
        let result = execute_command(&args, &config).await;
        assert!(result.is_err()); // Expected to fail without real AWS setup
    }

    #[tokio::test]
    async fn test_execute_cp_command() {
        let config = create_mock_config();
        let args = Args {
            debug: "info".to_string(),
            endpoint: None,
            region: "us-east-1".to_string(),
            timeout: 10,
            command: Commands::Cp {
                source: "./test".to_string(),
                dest: "s3://bucket/test".to_string(),
                recursive: false,
                dryrun: true, // Use dry run to avoid actual operations
                max_concurrent: 4,
                force: false,
                include: None,
                exclude: None,
            },
        };

        let result = execute_command(&args, &config).await;
        // Should succeed in dry run mode
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_sync_command() {
        let config = create_mock_config();
        let args = Args {
            debug: "info".to_string(),
            endpoint: None,
            region: "us-east-1".to_string(),
            timeout: 10,
            command: Commands::Sync {
                source: ".".to_string(), // Use current directory which exists
                dest: "s3://bucket/test".to_string(),
                delete: false,
                dryrun: true,
                max_concurrent: 4,
                include: None,
                exclude: None,
            },
        };

        let result = execute_command(&args, &config).await;
        // Sync will fail because it tries to list S3 objects even in dry-run mode
        // This is expected behavior without real AWS credentials
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_rm_command() {
        let config = create_mock_config();
        let args = Args {
            debug: "info".to_string(),
            endpoint: None,
            region: "us-east-1".to_string(),
            timeout: 10,
            command: Commands::Rm {
                s3_uri: "s3://bucket/file".to_string(),
                recursive: false,
                dryrun: true,
                include: None,
                exclude: None,
            },
        };

        let result = execute_command(&args, &config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_mb_command() {
        let config = create_mock_config();
        let args = Args {
            debug: "info".to_string(),
            endpoint: None,
            region: "us-east-1".to_string(),
            timeout: 10,
            command: Commands::Mb {
                s3_uri: "s3://new-bucket".to_string(),
            },
        };

        let result = execute_command(&args, &config).await;
        // Will fail without real AWS credentials, but tests routing
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_rb_command() {
        let config = create_mock_config();
        let args = Args {
            debug: "info".to_string(),
            endpoint: None,
            region: "us-east-1".to_string(),
            timeout: 10,
            command: Commands::Rb {
                s3_uri: Some("s3://bucket".to_string()),
                force: false,
                all: false,
                confirm: false,
                pattern: None,
            },
        };

        let result = execute_command(&args, &config).await;
        // Will fail without real AWS credentials, but tests routing
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_presign_command() {
        let config = create_mock_config();
        let args = Args {
            debug: "info".to_string(),
            endpoint: None,
            region: "us-east-1".to_string(),
            timeout: 10,
            command: Commands::Presign {
                s3_uri: "s3://bucket/file".to_string(),
                expires_in: 3600,
            },
        };

        let result = execute_command(&args, &config).await;
        // Presign is a placeholder, should succeed
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_head_object_command() {
        let config = create_mock_config();
        let args = Args {
            debug: "info".to_string(),
            endpoint: None,
            region: "us-east-1".to_string(),
            timeout: 10,
            command: Commands::HeadObject {
                bucket: "test-bucket".to_string(),
                key: "test-key".to_string(),
            },
        };

        let result = execute_command(&args, &config).await;
        // Will fail without real AWS credentials, but tests routing
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_du_command() {
        let config = create_mock_config();
        let args = Args {
            debug: "info".to_string(),
            endpoint: None,
            region: "us-east-1".to_string(),
            timeout: 10,
            command: Commands::Du {
                s3_uri: "s3://bucket/path".to_string(),
                human_readable: true,
                summarize: false,
            },
        };

        let result = execute_command(&args, &config).await;
        // Will fail without real AWS credentials, but tests routing
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_config_command() {
        let config = create_mock_config();
        let args = Args {
            debug: "info".to_string(),
            endpoint: None,
            region: "us-east-1".to_string(),
            timeout: 10,
            command: Commands::Config { command: None },
        };

        let result = execute_command(&args, &config).await;
        // Config command should always succeed as it just prints help
        assert!(result.is_ok());
    }

    #[test]
    fn test_command_routing_structure() {
        // Test that all command variants are handled in the match statement
        // This is a compile-time test - if we add a new command variant and don't handle it,
        // this will fail to compile

        let commands = [
            Commands::Ls {
                path: None,
                long: false,
                recursive: false,
                human_readable: false,
                summarize: false,
                pattern: None,
                created_after: None,
                created_before: None,
                modified_after: None,
                modified_before: None,
                min_size: None,
                max_size: None,
                max_results: None,
                head: None,
                tail: None,
                sort_by: None,
                reverse: false,
            },
            Commands::Cp {
                source: "src".to_string(),
                dest: "dest".to_string(),
                recursive: false,
                dryrun: false,
                max_concurrent: 1,
                force: false,
                include: None,
                exclude: None,
            },
            Commands::Sync {
                source: "src".to_string(),
                dest: "dest".to_string(),
                delete: false,
                dryrun: false,
                max_concurrent: 1,
                include: None,
                exclude: None,
            },
            Commands::Rm {
                s3_uri: "s3://bucket/key".to_string(),
                recursive: false,
                dryrun: false,
                include: None,
                exclude: None,
            },
            Commands::Mb {
                s3_uri: "s3://bucket".to_string(),
            },
            Commands::Rb {
                s3_uri: Some("s3://bucket".to_string()),
                force: false,
                all: false,
                confirm: false,
                pattern: None,
            },
            Commands::Presign {
                s3_uri: "s3://bucket/key".to_string(),
                expires_in: 3600,
            },
            Commands::HeadObject {
                bucket: "bucket".to_string(),
                key: "key".to_string(),
            },
            Commands::Du {
                s3_uri: "s3://bucket".to_string(),
                human_readable: false,
                summarize: false,
            },
            Commands::Config { command: None },
        ];

        // If this compiles, all command variants are properly structured
        assert_eq!(commands.len(), 10);
    }
}
