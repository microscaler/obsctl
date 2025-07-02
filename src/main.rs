use anyhow::Result;
use clap::Parser;
#[cfg(target_os = "linux")]
use sd_notify::NotifyState;

use obsctl::args::Args;
use obsctl::commands::execute_command;
use obsctl::config::Config;
use obsctl::logging::init_logging;
use obsctl::otel;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    init_logging(&args.debug)?;

    // Initialize configuration
    let config = Config::new(&args).await?;

    // Initialize OpenTelemetry if enabled
    otel::init_tracing(&config.otel, &args.debug)?;

    #[cfg(target_os = "linux")]
    sd_notify::notify(true, &[NotifyState::Ready]).ok();

    // Execute the appropriate command
    execute_command(&args, &config).await?;

    // Shutdown OpenTelemetry
    otel::shutdown_tracing();

    #[cfg(target_os = "linux")]
    sd_notify::notify(true, &[NotifyState::Stopping]).ok();

    Ok(())
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
                "--method",
                "GET",
                "s3://bucket/file.txt",
            ],
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
        let test_cases = vec![vec!["obsctl", "head-object", "s3://bucket/file.txt"]];

        for args in test_cases {
            let result = Args::try_parse_from(args.clone());
            assert!(
                result.is_ok(),
                "Failed to parse head-object args: {args:?}"
            );
        }
    }

    #[test]
    fn test_args_parsing_du_command() {
        let test_cases = vec![
            vec!["obsctl", "du", "s3://bucket"],
            vec!["obsctl", "du", "--human-readable", "s3://bucket"],
            vec!["obsctl", "du", "--max-depth", "2", "s3://bucket"],
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
