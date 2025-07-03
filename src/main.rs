use anyhow::Result;
use clap::Parser;
#[cfg(target_os = "linux")]
use sd_notify::NotifyState;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use obsctl::args::Args;
use obsctl::commands::execute_command;
use obsctl::config::Config;
use obsctl::logging::init_logging;
use obsctl::otel;

/// Set up broken pipe handling to prevent panics when output is piped to commands like `head`
fn setup_broken_pipe_handling() {
    // Set a custom panic hook that handles broken pipe errors gracefully
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // Check if this is a broken pipe error
        if let Some(payload) = panic_info.payload().downcast_ref::<String>() {
            if payload.contains("Broken pipe") || payload.contains("os error 32") {
                // Broken pipe - just exit gracefully without showing panic
                std::process::exit(0);
            }
        }

        // For any other panic, use the original handler
        original_hook(panic_info);
    }));

    // Also handle SIGPIPE signals on Unix systems
    #[cfg(unix)]
    {
        unsafe {
            libc::signal(libc::SIGPIPE, libc::SIG_DFL);
        }
    }
}

/// Flush output streams before exit to ensure all data is written
fn flush_output() {
    let _ = io::stdout().flush();
    let _ = io::stderr().flush();
}

/// Create error log directory if it doesn't exist
fn ensure_error_log_dir() -> Result<PathBuf> {
    let error_dir = PathBuf::from("/tmp/obsctl");
    if !error_dir.exists() {
        fs::create_dir_all(&error_dir)?;
    }
    Ok(error_dir)
}

/// Write detailed error information to a log file
fn write_error_log(error: &anyhow::Error) -> Option<PathBuf> {
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let error_id = uuid::Uuid::new_v4().to_string()[..8].to_string();

    if let Ok(error_dir) = ensure_error_log_dir() {
        let log_file = error_dir.join(format!("error-{timestamp}-{error_id}.log"));

        if let Ok(mut file) = fs::File::create(&log_file) {
            let detailed_error = format!(
                "obsctl Error Report\n\
                 ==================\n\
                 Timestamp: {}\n\
                 Error ID: {}\n\
                 Version: {}\n\
                 \n\
                 Error Details:\n\
                 {:#}\n\
                 \n\
                 Error Chain:\n\
                 {}\n\
                 \n\
                 Environment:\n\
                 - OS: {}\n\
                 - Architecture: {}\n\
                 - Args: {:?}\n",
                chrono::Utc::now().to_rfc3339(),
                error_id,
                env!("CARGO_PKG_VERSION"),
                error,
                error
                    .chain()
                    .enumerate()
                    .map(|(i, e)| format!("  {i}: {e}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
                std::env::consts::OS,
                std::env::consts::ARCH,
                std::env::args().collect::<Vec<_>>()
            );

            if file.write_all(detailed_error.as_bytes()).is_ok() {
                return Some(log_file);
            }
        }
    }

    None
}

#[tokio::main]
async fn main() -> Result<()> {
    setup_broken_pipe_handling();

    let args = Args::parse();

    init_logging(&args.debug)?;

    let config = Config::new(&args).await?;

    // Initialize OpenTelemetry if enabled
    otel::init_tracing(&config.otel, &args.debug)?;

    #[cfg(target_os = "linux")]
    sd_notify::notify(false, &[NotifyState::Ready]).ok();

    // Execute the appropriate command
    let result = execute_command(&args, &config).await;

    // Shutdown OpenTelemetry
    otel::shutdown_tracing(&args.debug);

    #[cfg(target_os = "linux")]
    sd_notify::notify(true, &[NotifyState::Stopping]).ok();

    // Handle errors with clean user-friendly output and detailed logging
    if let Err(e) = result {
        // Always write detailed error to log file
        let log_file = write_error_log(&e);

        // Show appropriate error message based on debug level
        if matches!(args.debug.as_str(), "debug" | "trace") {
            eprintln!("Error: {e:#}"); // Full error chain with debug details
            if let Some(log_path) = log_file {
                eprintln!("Detailed error log: {}", log_path.display());
            }
        } else {
            // Clean user-friendly error message
            eprintln!("Error: {}", format_user_error(&e));
            if let Some(log_path) = log_file {
                eprintln!(
                    "For detailed error information, see: {}",
                    log_path.display()
                );
                eprintln!("Or run with --debug debug for more details");
            }
        }

        // Flush output before exit
        flush_output();
        std::process::exit(1);
    }

    // Flush output before exit
    flush_output();
    Ok(())
}

/// Format errors for user-friendly display
fn format_user_error(error: &anyhow::Error) -> String {
    let error_str = error.to_string();

    // Handle AWS service errors with cleaner formatting
    if error_str.contains("service error") {
        // First check for specific error types in the error string
        if error_str.contains("SignatureDoesNotMatch") {
            return "Authentication failed: Invalid AWS credentials or signature. Please check your access key, secret key, and endpoint configuration.".to_string();
        } else if error_str.contains("NoSuchBucket") {
            return "Bucket not found: The specified bucket does not exist or you don't have access to it.".to_string();
        } else if error_str.contains("AccessDenied") {
            return "Access denied: You don't have permission to perform this operation."
                .to_string();
        } else if error_str.contains("InvalidBucketName") {
            return "Invalid bucket name: Bucket names must follow AWS naming conventions."
                .to_string();
        } else if error_str.contains("BucketAlreadyExists") {
            return "Bucket already exists: Choose a different bucket name.".to_string();
        } else if error_str.contains("NoSuchKey") {
            return "Object not found: The specified object does not exist.".to_string();
        } else if error_str.contains("dispatch failure") {
            return "Connection failed: Unable to connect to the S3 service. Check your endpoint URL and network connection.".to_string();
        }

        // Try to extract error code and message from AWS error format
        // Format: Error { code: "ErrorCode", message: "Error message", ... }
        if let Some(code_start) = error_str.find("code: \"") {
            if let Some(code_end) = error_str[code_start + 7..].find('"') {
                let code = &error_str[code_start + 7..code_start + 7 + code_end];

                if let Some(msg_start) = error_str.find("message: \"") {
                    if let Some(msg_end) = error_str[msg_start + 10..].find('"') {
                        let message = &error_str[msg_start + 10..msg_start + 10 + msg_end];
                        return format!("S3 service error ({code}): {message}");
                    }
                }

                // If we have a code but no message, just use the code
                return format!("S3 service error: {code}");
            }
        }

        // Fallback: try to extract the error type from parentheses
        if let Some(start) = error_str.find("unhandled error (") {
            if let Some(end) = error_str[start + 17..].find(')') {
                let error_type = &error_str[start + 17..start + 17 + end];
                return format!("S3 service error: {error_type}");
            }
        }

        // Final fallback for service errors
        return "S3 service error: Please check your credentials and endpoint configuration."
            .to_string();
    }

    // Handle network/connection errors
    if error_str.contains("Connection refused") || error_str.contains("connection error") {
        return "Connection failed: Unable to connect to the S3 service. Check your endpoint URL and network connection.".to_string();
    }

    // Handle DNS errors
    if error_str.contains("failed to lookup address")
        || error_str.contains("Name or service not known")
    {
        return "DNS lookup failed: Unable to resolve the S3 endpoint. Check your endpoint URL."
            .to_string();
    }

    // Handle timeout errors
    if error_str.contains("timeout") || error_str.contains("timed out") {
        return "Operation timed out: The request took too long to complete. Try again or increase the timeout.".to_string();
    }

    // Handle file system errors
    if error_str.contains("No such file or directory") {
        return "File not found: The specified local file or directory does not exist.".to_string();
    }

    if error_str.contains("Permission denied") {
        return "Permission denied: You don't have permission to access the specified file or directory.".to_string();
    }

    // For other errors, return the first line only (remove stack trace)
    error_str.lines().next().unwrap_or(&error_str).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_args_help() {
        // Test that help works
        let result = std::panic::catch_unwind(|| {
            Args::parse_from(["obsctl", "--help"]);
        });
        // This will panic because --help exits, but that's expected
        assert!(result.is_err());
    }

    #[test]
    fn test_args_version() {
        // Test that version works
        let result = std::panic::catch_unwind(|| {
            Args::parse_from(["obsctl", "--version"]);
        });
        // This will panic because --version exits, but that's expected
        assert!(result.is_err());
    }

    #[test]
    fn test_args_parsing_basic_command() {
        // Test parsing basic commands without execution
        let test_cases = vec![
            vec!["obsctl", "ls"],
            vec!["obsctl", "ls", "s3://bucket"],
            vec!["obsctl", "--debug", "info", "ls"],
            vec!["obsctl", "--region", "us-west-2", "ls"],
            vec!["obsctl", "--endpoint", "http://localhost:9000", "ls"],
        ];

        for args in test_cases {
            let result = Args::try_parse_from(args.clone());
            assert!(result.is_ok(), "Failed to parse args: {args:?}");
        }
    }

    #[test]
    fn test_args_parsing_cp_command() {
        let test_cases = vec![
            vec!["obsctl", "cp", "file.txt", "s3://bucket/file.txt"],
            vec!["obsctl", "cp", "s3://bucket/file.txt", "local-file.txt"],
            vec![
                "obsctl",
                "cp",
                "--recursive",
                "folder/",
                "s3://bucket/folder/",
            ],
        ];

        for args in test_cases {
            let result = Args::try_parse_from(args.clone());
            assert!(result.is_ok(), "Failed to parse cp args: {args:?}");
        }
    }

    #[test]
    fn test_args_parsing_sync_command() {
        let test_cases = vec![
            vec!["obsctl", "sync", "folder/", "s3://bucket/folder/"],
            vec!["obsctl", "sync", "s3://bucket/folder/", "local-folder/"],
            vec![
                "obsctl",
                "sync",
                "--delete",
                "folder/",
                "s3://bucket/folder/",
            ],
        ];

        for args in test_cases {
            let result = Args::try_parse_from(args.clone());
            assert!(result.is_ok(), "Failed to parse sync args: {args:?}");
        }
    }

    #[test]
    fn test_args_parsing_rm_command() {
        let test_cases = vec![
            vec!["obsctl", "rm", "s3://bucket/file.txt"],
            vec!["obsctl", "rm", "--recursive", "s3://bucket/folder/"],
            vec!["obsctl", "rm", "--dryrun", "s3://bucket/file.txt"],
        ];

        for args in test_cases {
            let result = Args::try_parse_from(args.clone());
            assert!(result.is_ok(), "Failed to parse rm args: {args:?}");
        }
    }

    #[test]
    fn test_args_parsing_bucket_commands() {
        let test_cases = vec![
            vec!["obsctl", "mb", "s3://new-bucket"],
            vec!["obsctl", "rb", "s3://bucket-to-remove"],
            vec!["obsctl", "rb", "--force", "s3://bucket-to-remove"],
        ];

        for args in test_cases {
            let result = Args::try_parse_from(args.clone());
            assert!(result.is_ok(), "Failed to parse bucket args: {args:?}");
        }
    }

    #[test]
    fn test_args_parsing_presign_command() {
        let test_cases = vec![
            vec!["obsctl", "presign", "s3://bucket/file.txt"],
            vec![
                "obsctl",
                "presign",
                "--expires-in",
                "3600",
                "s3://bucket/file.txt",
            ],
        ];

        for args in test_cases {
            let result = Args::try_parse_from(args.clone());
            assert!(result.is_ok(), "Failed to parse presign args: {args:?}");
        }
    }

    #[test]
    fn test_args_parsing_head_object_command() {
        let test_cases = vec![vec![
            "obsctl",
            "head-object",
            "--bucket",
            "my-bucket",
            "--key",
            "my-key",
        ]];

        for args in test_cases {
            let result = Args::try_parse_from(args.clone());
            assert!(result.is_ok(), "Failed to parse head-object args: {args:?}");
        }
    }

    #[test]
    fn test_args_parsing_du_command() {
        let test_cases = vec![
            vec!["obsctl", "du", "s3://bucket"],
            vec!["obsctl", "du", "--human-readable", "s3://bucket"],
            vec!["obsctl", "du", "--summarize", "s3://bucket"],
        ];

        for args in test_cases {
            let result = Args::try_parse_from(args.clone());
            assert!(result.is_ok(), "Failed to parse du args: {args:?}");
        }
    }

    #[test]
    fn test_args_parsing_global_options() {
        let test_cases = vec![
            vec!["obsctl", "--debug", "trace", "ls"],
            vec!["obsctl", "--debug", "debug", "ls"],
            vec!["obsctl", "--debug", "info", "ls"],
            vec!["obsctl", "--debug", "warn", "ls"],
            vec!["obsctl", "--debug", "error", "ls"],
            vec!["obsctl", "--region", "us-east-1", "ls"],
            vec!["obsctl", "--region", "eu-west-1", "ls"],
            vec!["obsctl", "--endpoint", "https://s3.amazonaws.com", "ls"],
            vec!["obsctl", "--timeout", "30", "ls"],
        ];

        for args in test_cases {
            let result = Args::try_parse_from(args.clone());
            assert!(result.is_ok(), "Failed to parse global options: {args:?}");
        }
    }

    #[test]
    fn test_args_parsing_invalid_commands() {
        let test_cases = vec![
            vec!["obsctl", "invalid-command"],
            vec!["obsctl", "ls", "--invalid-flag"],
            vec!["obsctl", "cp"], // missing required args
            vec!["obsctl", "--debug", "invalid-level", "ls"],
            vec!["obsctl", "--timeout", "invalid-number", "ls"],
        ];

        for args in test_cases {
            let result = Args::try_parse_from(args.clone());
            assert!(
                result.is_err(),
                "Should have failed to parse invalid args: {args:?}"
            );
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_sd_notify_states() {
        // Test that sd_notify states are valid
        let _ready_state = NotifyState::Ready;
        let _stopping_state = NotifyState::Stopping;

        // These should compile and be valid - no assertion needed
    }

    #[test]
    fn test_imports_are_valid() {
        // Test that all imports are accessible
        // All imports should be valid - no assertion needed
    }

    #[test]
    fn test_main_function_components() {
        // Test individual components that main() uses

        // Test that Args can be created (though not parsed without actual CLI args)
        let result = Args::try_parse_from(vec!["obsctl", "ls"]);
        assert!(result.is_ok());

        // Test that the main function signature is correct
        // (this is a compile-time test) - no assertion needed
    }
}
