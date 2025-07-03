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

    // Handle AWS service errors with comprehensive troubleshooting guidance
    if error_str.contains("service error") {
        // First check for specific error types in the error string
        if error_str.contains("SignatureDoesNotMatch") {
            return "❌ AUTHENTICATION FAILED: Invalid AWS credentials or signature mismatch

🔍 WHAT THIS MEANS:
Your AWS credentials (access key/secret key) don't match what the S3 server expects.
This is usually a credential mismatch between obsctl and your S3 service.

🛠️  STEP-BY-STEP TROUBLESHOOTING:

1️⃣  CHECK YOUR CREDENTIALS:
   • For MinIO (local development): Use 'minioadmin' / 'minioadmin123'
   • For AWS S3: Use your AWS access key and secret
   • For Cloud.ru: Use your Cloud.ru OBS credentials

2️⃣  VERIFY CONFIGURATION:
   Run: obsctl config list
   Check that your credentials are set correctly

3️⃣  SET CREDENTIALS (choose one method):

   METHOD A - Environment Variables:
   export AWS_ACCESS_KEY_ID=your_access_key
   export AWS_SECRET_ACCESS_KEY=your_secret_key
   export AWS_ENDPOINT_URL=your_endpoint_url

   METHOD B - Configuration Files:
   obsctl config set aws_access_key_id your_access_key
   obsctl config set aws_secret_access_key your_secret_key
   obsctl config set endpoint_url your_endpoint_url

4️⃣  COMMON CREDENTIAL EXAMPLES:
   • MinIO: minioadmin / minioadmin123
   • AWS S3: AKIAIOSFODNN7EXAMPLE / wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY
   • Cloud.ru: Your OBS access key / secret from portal

5️⃣  TEST CONNECTION:
   obsctl ls (should list your buckets)

💡 HINT: If using MinIO, the default password is 'minioadmin123' (not 'minioadmin')"
                .to_string();
        } else if error_str.contains("NoSuchBucket") {
            return "❌ BUCKET NOT FOUND: The specified bucket does not exist

🔍 WHAT THIS MEANS:
The bucket you're trying to access doesn't exist or you don't have permission to see it.

🛠️  STEP-BY-STEP TROUBLESHOOTING:

1️⃣  LIST AVAILABLE BUCKETS:
   obsctl ls
   (This shows all buckets you can access)

2️⃣  CREATE THE BUCKET IF NEEDED:
   obsctl mb s3://your-bucket-name

3️⃣  CHECK BUCKET NAME SPELLING:
   • Bucket names are case-sensitive
   • No spaces or special characters (except hyphens)
   • Must be 3-63 characters long

4️⃣  VERIFY PERMISSIONS:
   Make sure your credentials have access to this bucket

💡 HINT: Try 'obsctl ls' first to see what buckets exist"
                .to_string();
        } else if error_str.contains("AccessDenied") {
            return "❌ ACCESS DENIED: You don't have permission for this operation

🔍 WHAT THIS MEANS:
Your credentials are valid, but you don't have permission to perform this specific action.

🛠️  STEP-BY-STEP TROUBLESHOOTING:

1️⃣  CHECK YOUR PERMISSIONS:
   • Can you list buckets? Try: obsctl ls
   • Can you read this bucket? Try: obsctl ls s3://bucket-name

2️⃣  VERIFY BUCKET OWNERSHIP:
   • Did you create this bucket?
   • Are you using the right AWS account/credentials?

3️⃣  CHECK IAM POLICIES (AWS S3):
   Your user needs permissions for:
   • s3:ListBucket (to list objects)
   • s3:GetObject (to download)
   • s3:PutObject (to upload)
   • s3:DeleteObject (to delete)

4️⃣  FOR MINIO:
   Check MinIO console at http://localhost:9001
   Verify your user has the right bucket policies

💡 HINT: Start with 'obsctl ls' to test basic access"
                .to_string();
        } else if error_str.contains("InvalidBucketName") {
            return "❌ INVALID BUCKET NAME: Bucket name doesn't follow naming rules

🔍 WHAT THIS MEANS:
Bucket names must follow specific naming conventions.

🛠️  BUCKET NAMING RULES:

1️⃣  LENGTH: 3-63 characters
2️⃣  CHARACTERS: Only lowercase letters, numbers, hyphens (-)
3️⃣  START/END: Must start and end with letter or number
4️⃣  NO DOTS: Avoid periods (.) in bucket names
5️⃣  NO SPACES: No spaces or special characters

✅ GOOD EXAMPLES:
   • my-company-backups
   • user-data-2024
   • project-assets

❌ BAD EXAMPLES:
   • My Bucket (spaces, capitals)
   • my.bucket.name (periods)
   • -invalid-start (starts with hyphen)
   • invalid-end- (ends with hyphen)

💡 HINT: Use lowercase letters, numbers, and hyphens only"
                .to_string();
        } else if error_str.contains("BucketAlreadyExists") {
            return "❌ BUCKET ALREADY EXISTS: This bucket name is already taken

🔍 WHAT THIS MEANS:
Someone else (or you on another account) already created a bucket with this name.
Bucket names are globally unique across all users.

🛠️  STEP-BY-STEP SOLUTIONS:

1️⃣  TRY A DIFFERENT NAME:
   Add your company/username to make it unique:
   • my-company-bucket-name
   • username-project-data
   • company-backups-2024

2️⃣  CHECK IF YOU OWN IT:
   obsctl ls
   (See if the bucket appears in your list)

3️⃣  USE A MORE SPECIFIC NAME:
   • project-name-environment (e.g., myapp-production)
   • department-purpose-date (e.g., marketing-assets-2024)

💡 HINT: Add your organization name to make bucket names unique"
                .to_string();
        } else if error_str.contains("NoSuchKey") {
            return "❌ OBJECT NOT FOUND: The specified file/object does not exist

🔍 WHAT THIS MEANS:
The file you're looking for doesn't exist in the bucket or has a different path.

🛠️  STEP-BY-STEP TROUBLESHOOTING:

1️⃣  LIST BUCKET CONTENTS:
   obsctl ls s3://bucket-name
   (See what files actually exist)

2️⃣  CHECK THE FULL PATH:
   obsctl ls s3://bucket-name/folder/
   (List contents of specific folders)

3️⃣  VERIFY FILE PATH:
   • Paths are case-sensitive
   • Use forward slashes (/) not backslashes (\\)
   • Don't include leading slash

✅ CORRECT EXAMPLES:
   • s3://mybucket/folder/file.txt
   • s3://mybucket/data/2024/report.pdf

❌ INCORRECT EXAMPLES:
   • s3://mybucket\\folder\\file.txt (backslashes)
   • s3://mybucket/Folder/File.txt (wrong case)

💡 HINT: Use 'obsctl ls s3://bucket-name' to explore the bucket structure"
                .to_string();
        } else if error_str.contains("dispatch failure") || error_str.contains("connection error") {
            return "❌ CONNECTION FAILED: Cannot connect to S3 service

🔍 WHAT THIS MEANS:
obsctl cannot reach your S3 service. This is usually a network or endpoint configuration issue.

🛠️  STEP-BY-STEP TROUBLESHOOTING:

1️⃣  CHECK YOUR ENDPOINT URL:
   obsctl config get endpoint_url

   Common endpoints:
   • AWS S3: https://s3.amazonaws.com (or leave blank)
   • MinIO local: http://localhost:9000 or http://127.0.0.1:9000
   • Cloud.ru OBS: https://obs.ru-moscow-1.hc.sbercloud.ru

2️⃣  VERIFY SERVICE IS RUNNING:
   For MinIO: Check if Docker container is running
   docker ps | grep minio

   For AWS S3: Check internet connection
   ping s3.amazonaws.com

3️⃣  TEST ENDPOINT MANUALLY:
   curl -I http://localhost:9000    (for MinIO)
   Should return HTTP headers if service is running

4️⃣  CHECK FIREWALL/NETWORK:
   • Is port 9000 blocked? (MinIO)
   • Are you behind a corporate firewall?
   • Is your internet connection working?

5️⃣  FIX ENDPOINT CONFIGURATION:
   obsctl config set endpoint_url http://localhost:9000    (for MinIO)
   obsctl config set endpoint_url https://s3.amazonaws.com (for AWS)

💡 HINT: For MinIO, make sure Docker is running and container is started"
                .to_string();
        }

        // Try to extract error code and message from AWS error format
        if let Some(code_start) = error_str.find("code: \"") {
            if let Some(code_end) = error_str[code_start + 7..].find('"') {
                let code = &error_str[code_start + 7..code_start + 7 + code_end];

                if let Some(msg_start) = error_str.find("message: \"") {
                    if let Some(msg_end) = error_str[msg_start + 10..].find('"') {
                        let message = &error_str[msg_start + 10..msg_start + 10 + msg_end];
                        return format!(
                            "❌ S3 SERVICE ERROR: {code}

🔍 WHAT HAPPENED:
{message}

🛠️  GENERAL TROUBLESHOOTING STEPS:

1️⃣  CHECK CONFIGURATION:
   obsctl config list
   (Verify your credentials and endpoint are set)

2️⃣  TEST BASIC CONNECTION:
   obsctl ls
   (Try listing buckets first)

3️⃣  VERIFY CREDENTIALS:
   • AWS_ACCESS_KEY_ID is set
   • AWS_SECRET_ACCESS_KEY is set
   • AWS_ENDPOINT_URL points to correct service

4️⃣  GET DETAILED ERROR INFO:
   Add --debug debug to your command for more details
   Example: obsctl --debug debug ls

5️⃣  COMMON SOLUTIONS:
   • Wrong credentials → Run 'obsctl config configure'
   • Wrong endpoint → Check endpoint URL
   • Service down → Verify S3 service is running
   • Network issues → Check internet/firewall

💡 HINT: Run 'obsctl config configure' to set up credentials interactively"
                        );
                    }
                }

                // If we have a code but no message, provide generic guidance
                return format!(
                    "❌ S3 SERVICE ERROR: {code}

🛠️  TROUBLESHOOTING STEPS:

1️⃣  CHECK YOUR CONFIGURATION:
   obsctl config list

2️⃣  VERIFY CREDENTIALS ARE SET:
   • Access Key ID
   • Secret Access Key
   • Endpoint URL (if not using AWS)

3️⃣  TEST CONNECTION:
   obsctl ls

4️⃣  GET MORE DETAILS:
   Run your command with --debug debug for detailed error information

💡 HINT: Try 'obsctl config configure' to set up credentials step-by-step"
                );
            }
        }

        // Final fallback for service errors
        return "❌ S3 SERVICE ERROR: Configuration or connection problem

🔍 WHAT THIS USUALLY MEANS:
Your credentials, endpoint, or network configuration needs attention.

🛠️  COMPLETE TROUBLESHOOTING CHECKLIST:

1️⃣  VERIFY BASIC SETUP:
   obsctl config list
   (Check if credentials and endpoint are configured)

2️⃣  SET UP CREDENTIALS (if missing):
   obsctl config configure
   (Interactive setup - recommended for beginners)

3️⃣  COMMON CREDENTIAL SETS:

   FOR MINIO (LOCAL DEVELOPMENT):
   • Access Key: minioadmin
   • Secret Key: minioadmin123
   • Endpoint: http://localhost:9000

   FOR AWS S3:
   • Access Key: Your AWS access key (starts with AKIA...)
   • Secret Key: Your AWS secret key
   • Endpoint: (leave blank or https://s3.amazonaws.com)

   FOR CLOUD.RU OBS:
   • Access Key: Your OBS access key
   • Secret Key: Your OBS secret key
   • Endpoint: https://obs.ru-moscow-1.hc.sbercloud.ru

4️⃣  TEST YOUR SETUP:
   obsctl ls
   (Should list your buckets without errors)

5️⃣  GET DETAILED DIAGNOSTICS:
   obsctl --debug debug ls
   (Shows exactly what's happening)

💡 NEED HELP? Run 'obsctl config' for configuration guidance"
            .to_string();
    }

    // Handle network/connection errors with detailed guidance
    if error_str.contains("Connection refused") {
        return "❌ CONNECTION REFUSED: S3 service is not accepting connections

🔍 WHAT THIS MEANS:
The S3 service is either not running or not accessible on the specified port.

🛠️  STEP-BY-STEP SOLUTIONS:

1️⃣  FOR MINIO (LOCAL):
   Check if MinIO is running:
   docker ps | grep minio

   Start MinIO if not running:
   docker compose up -d minio

2️⃣  CHECK THE PORT:
   MinIO typically runs on port 9000
   Verify: curl -I http://localhost:9000

3️⃣  VERIFY ENDPOINT CONFIGURATION:
   obsctl config get endpoint_url
   Should be: http://localhost:9000 (for MinIO)

4️⃣  CHECK FIREWALL:
   Is port 9000 blocked by firewall?

💡 HINT: For MinIO, run 'docker compose up -d' to start services"
            .to_string();
    }

    // Handle DNS errors
    if error_str.contains("failed to lookup address")
        || error_str.contains("Name or service not known")
    {
        return "❌ DNS LOOKUP FAILED: Cannot resolve the S3 endpoint address

🔍 WHAT THIS MEANS:
Your computer cannot find the IP address for the S3 service hostname.

🛠️  STEP-BY-STEP SOLUTIONS:

1️⃣  CHECK ENDPOINT URL:
   obsctl config get endpoint_url

   Common endpoints:
   • AWS: s3.amazonaws.com
   • MinIO: localhost or 127.0.0.1
   • Cloud.ru: obs.ru-moscow-1.hc.sbercloud.ru

2️⃣  TEST DNS RESOLUTION:
   ping s3.amazonaws.com        (for AWS)
   ping localhost               (for MinIO)

3️⃣  CHECK INTERNET CONNECTION:
   Can you browse the web normally?

4️⃣  FOR MINIO LOCAL ISSUES:
   Try using IP address instead:
   obsctl config set endpoint_url http://127.0.0.1:9000

💡 HINT: For local MinIO, use 127.0.0.1 instead of localhost"
            .to_string();
    }

    // Handle timeout errors
    if error_str.contains("timeout") || error_str.contains("timed out") {
        return "❌ OPERATION TIMED OUT: Request took too long to complete

🔍 WHAT THIS MEANS:
The S3 service is responding too slowly, or there's a network issue.

🛠️  STEP-BY-STEP SOLUTIONS:

1️⃣  CHECK NETWORK SPEED:
   Is your internet connection slow?

2️⃣  VERIFY SERVICE HEALTH:
   For MinIO: Check container resources
   docker stats | grep minio

3️⃣  TRY SMALLER OPERATIONS:
   • List buckets first: obsctl ls
   • Try smaller files if uploading

4️⃣  CHECK SERVICE LOAD:
   Is the S3 service overloaded?

💡 HINT: Try the operation again - temporary network issues are common"
            .to_string();
    }

    // Handle file system errors with guidance
    if error_str.contains("No such file or directory") {
        return "❌ FILE NOT FOUND: The specified local file does not exist

🔍 WHAT THIS MEANS:
obsctl cannot find the local file you're trying to upload or access.

🛠️  STEP-BY-STEP SOLUTIONS:

1️⃣  CHECK FILE PATH:
   ls -la /path/to/your/file
   (Verify the file actually exists)

2️⃣  USE EXPLICIT PATHS:
   Instead of: obsctl cp file.txt s3://bucket/
   Try: obsctl cp ./file.txt s3://bucket/

3️⃣  CHECK CURRENT DIRECTORY:
   pwd
   (Make sure you're in the right directory)

4️⃣  LIST FILES IN CURRENT DIRECTORY:
   ls -la
   (See what files are actually available)

💡 HINT: Use tab completion or absolute paths to avoid typos"
            .to_string();
    }

    if error_str.contains("Permission denied") {
        return "❌ PERMISSION DENIED: Cannot access the specified file or directory

🔍 WHAT THIS MEANS:
Your user account doesn't have permission to read/write the local file.

🛠️  STEP-BY-STEP SOLUTIONS:

1️⃣  CHECK FILE PERMISSIONS:
   ls -la /path/to/file
   (Look at the permission flags)

2️⃣  FIX FILE PERMISSIONS:
   chmod 644 /path/to/file      (for files)
   chmod 755 /path/to/directory (for directories)

3️⃣  CHECK OWNERSHIP:
   whoami                       (your username)
   ls -la /path/to/file        (file owner)

4️⃣  USE SUDO IF NEEDED:
   sudo obsctl cp /root/file.txt s3://bucket/
   (Only if you need root access)

💡 HINT: Make sure you own the file or have read permissions"
            .to_string();
    }

    // For other errors, return the first line only (remove stack trace) with basic guidance
    let first_line = error_str.lines().next().unwrap_or(&error_str);
    format!(
        "❌ ERROR: {first_line}

🛠️  GENERAL TROUBLESHOOTING:

1️⃣  CHECK CONFIGURATION:
   obsctl config list

2️⃣  VERIFY CREDENTIALS:
   obsctl config configure

3️⃣  TEST CONNECTION:
   obsctl ls

4️⃣  GET DETAILED INFO:
   Add --debug debug to your command

💡 HINT: Run 'obsctl config' for configuration help"
    )
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
