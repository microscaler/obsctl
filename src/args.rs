use clap::{Parser, Subcommand};

/// A comprehensive S3-compatible storage CLI tool for Cloud.ru OBS and similar services
#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct Args {
    /// Set log verbosity level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info", global = true)]
    pub debug: String,

    /// Custom endpoint URL
    #[arg(short, long, global = true)]
    pub endpoint: Option<String>,

    /// AWS region
    #[arg(short, long, default_value = "ru-moscow-1", global = true)]
    pub region: String,

    /// Timeout (in seconds) for all HTTP operations
    #[arg(long, default_value_t = 10, global = true)]
    pub timeout: u64,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// List objects in bucket (equivalent to aws s3 ls)
    Ls {
        /// S3 URI (s3://bucket/prefix) or bucket name
        #[arg(value_name = "S3_URI")]
        path: Option<String>,

        /// Show detailed information
        #[arg(long, default_value_t = false)]
        long: bool,

        /// Recursive listing
        #[arg(long, default_value_t = false)]
        recursive: bool,

        /// Human readable sizes
        #[arg(long, default_value_t = false)]
        human_readable: bool,

        /// Show summary only
        #[arg(long, default_value_t = false)]
        summarize: bool,

        /// Wildcard pattern for bucket names (e.g., "test-*", "*-prod", "user-?-bucket")
        #[arg(long)]
        pattern: Option<String>,

        // Date filtering
        /// Show objects created after date (YYYYMMDD or relative like '7d')
        #[arg(long)]
        created_after: Option<String>,

        /// Show objects created before date (YYYYMMDD or relative like '7d')
        #[arg(long)]
        created_before: Option<String>,

        /// Show objects modified after date (YYYYMMDD or relative like '7d')
        #[arg(long)]
        modified_after: Option<String>,

        /// Show objects modified before date (YYYYMMDD or relative like '7d')
        #[arg(long)]
        modified_before: Option<String>,

        // Size filtering (MB default)
        /// Minimum file size (default MB, e.g., '5' or '5MB' or '1GB')
        #[arg(long)]
        min_size: Option<String>,

        /// Maximum file size (default MB, e.g., '100' or '100MB' or '1GB')
        #[arg(long)]
        max_size: Option<String>,

        // Result limiting
        /// Maximum number of results to return
        #[arg(long)]
        max_results: Option<usize>,

        /// Show only first N results
        #[arg(long, conflicts_with = "tail")]
        head: Option<usize>,

        /// Show only last N results (by modification date)
        #[arg(long, conflicts_with = "head")]
        tail: Option<usize>,

        // Sorting
        /// Sort results by field (name, size, created, modified). Supports multi-level sorting like 'modified:desc,size:asc'
        #[arg(long)]
        sort_by: Option<String>,

        /// Reverse sort order (only for single field sorting)
        #[arg(long)]
        reverse: bool,
    },

    /// Copy files/objects (equivalent to aws s3 cp)
    Cp {
        /// Source (local path or s3://bucket/key)
        source: String,

        /// Destination (local path or s3://bucket/key)
        dest: String,

        /// Copy recursively
        #[arg(long, default_value_t = false)]
        recursive: bool,

        /// Dry run mode
        #[arg(long, default_value_t = false)]
        dryrun: bool,

        /// Maximum parallel operations
        #[arg(long, default_value_t = 4)]
        max_concurrent: usize,

        /// Force overwrite
        #[arg(long, default_value_t = false)]
        force: bool,

        /// Include files that match pattern
        #[arg(long)]
        include: Option<String>,

        /// Exclude files that match pattern
        #[arg(long)]
        exclude: Option<String>,
    },

    /// Sync directories (equivalent to aws s3 sync)
    Sync {
        /// Source directory (local or s3://bucket/prefix)
        source: String,

        /// Destination directory (local or s3://bucket/prefix)
        dest: String,

        /// Delete files in dest that don't exist in source
        #[arg(long, default_value_t = false)]
        delete: bool,

        /// Dry run mode
        #[arg(long, default_value_t = false)]
        dryrun: bool,

        /// Maximum parallel operations
        #[arg(long, default_value_t = 4)]
        max_concurrent: usize,

        /// Include files that match pattern
        #[arg(long)]
        include: Option<String>,

        /// Exclude files that match pattern
        #[arg(long)]
        exclude: Option<String>,
    },

    /// Remove objects (equivalent to aws s3 rm)
    Rm {
        /// S3 URI (s3://bucket/key)
        s3_uri: String,

        /// Delete recursively
        #[arg(long, default_value_t = false)]
        recursive: bool,

        /// Dry run mode
        #[arg(long, default_value_t = false)]
        dryrun: bool,

        /// Include files that match pattern
        #[arg(long)]
        include: Option<String>,

        /// Exclude files that match pattern
        #[arg(long)]
        exclude: Option<String>,
    },

    /// Create a new bucket (equivalent to aws s3 mb)
    Mb {
        /// S3 URI (s3://bucket-name)
        s3_uri: String,
    },

    /// Remove an empty bucket (equivalent to aws s3 rb)
    Rb {
        /// S3 URI (s3://bucket-name) - optional when using --all or --pattern
        s3_uri: Option<String>,

        /// Force removal (delete all objects first)
        #[arg(long, default_value_t = false)]
        force: bool,

        /// Remove all buckets
        #[arg(long, default_value_t = false)]
        all: bool,

        /// Confirm destructive operations (required for --all or --pattern)
        #[arg(long, default_value_t = false)]
        confirm: bool,

        /// Wildcard pattern for bucket names (e.g., "test-*", "*-prod", "user-?-bucket")
        #[arg(long)]
        pattern: Option<String>,
    },

    /// Generate presigned URLs (equivalent to aws s3 presign)
    Presign {
        /// S3 URI (s3://bucket/key)
        s3_uri: String,

        /// URL expiration time in seconds
        #[arg(long, default_value_t = 3600)]
        expires_in: u64,
    },

    /// Show object metadata (equivalent to aws s3api head-object)
    #[command(name = "head-object")]
    HeadObject {
        /// S3 bucket name
        #[arg(long)]
        bucket: String,

        /// S3 key
        #[arg(long)]
        key: String,
    },

    /// Show storage usage statistics (custom extension)
    Du {
        /// S3 URI (s3://bucket/prefix)
        s3_uri: String,

        /// Human readable sizes
        #[arg(long, default_value_t = false)]
        human_readable: bool,

        /// Show summary only
        #[arg(short, long, default_value_t = false)]
        summarize: bool,
    },

    /// Configuration management and setup guidance
    Config {
        #[command(subcommand)]
        command: Option<ConfigCommands>,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum ConfigCommands {
    /// Interactive configuration setup (like 'aws configure')
    Configure {
        /// AWS profile name
        #[arg(long, default_value = "default")]
        profile: String,
    },
    /// Set a configuration value
    Set {
        /// Configuration key (e.g., region, aws_access_key_id, endpoint_url)
        key: String,
        /// Configuration value
        value: String,
        /// AWS profile name
        #[arg(long, default_value = "default")]
        profile: String,
    },
    /// Get a configuration value
    Get {
        /// Configuration key to retrieve
        key: String,
        /// AWS profile name
        #[arg(long, default_value = "default")]
        profile: String,
    },
    /// List all configuration for a profile
    List {
        /// AWS profile name
        #[arg(long, default_value = "default")]
        profile: String,
        /// Show file paths where configuration is stored
        #[arg(long)]
        files: bool,
    },
    /// Dashboard management commands
    Dashboard {
        #[command(subcommand)]
        command: DashboardCommands,
    },
    /// Show configuration examples
    Example,
    /// Show environment variables
    Env,
    /// Show OpenTelemetry configuration
    Otel,
}

#[derive(Debug, Clone, Subcommand)]
pub enum DashboardCommands {
    /// Install obsctl dashboards to Grafana
    Install {
        /// Grafana URL
        #[arg(long, default_value = "http://localhost:3000")]
        url: String,
        /// Grafana username
        #[arg(long, default_value = "admin")]
        username: String,
        /// Grafana password
        #[arg(long, default_value = "admin")]
        password: String,
        /// Organization ID
        #[arg(long, default_value = "1")]
        org_id: String,
        /// Folder name for obsctl dashboards
        #[arg(long, default_value = "obsctl")]
        folder: String,
        /// Force overwrite existing obsctl dashboards
        #[arg(long)]
        force: bool,
    },
    /// List obsctl dashboards (only shows obsctl-related dashboards)
    List {
        /// Grafana URL
        #[arg(long, default_value = "http://localhost:3000")]
        url: String,
        /// Grafana username
        #[arg(long, default_value = "admin")]
        username: String,
        /// Grafana password
        #[arg(long, default_value = "admin")]
        password: String,
    },
    /// Remove obsctl dashboards from Grafana (only removes obsctl dashboards)
    Remove {
        /// Grafana URL
        #[arg(long, default_value = "http://localhost:3000")]
        url: String,
        /// Grafana username
        #[arg(long, default_value = "admin")]
        username: String,
        /// Grafana password
        #[arg(long, default_value = "admin")]
        password: String,
        /// Confirm removal of obsctl dashboards
        #[arg(long)]
        confirm: bool,
    },
    /// Show obsctl dashboard information and installation paths
    Info,
    /// Show system information including file descriptor monitoring
    System,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ls_command_parsing() {
        let args = Args::parse_from([
            "obsctl",
            "ls",
            "s3://my-bucket",
            "--long",
            "--recursive",
            "--human-readable",
        ]);

        if let Commands::Ls {
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
        } = args.command
        {
            assert_eq!(path, Some("s3://my-bucket".to_string()));
            assert!(long);
            assert!(recursive);
            assert!(human_readable);
            assert!(!summarize);
            assert_eq!(pattern, None);
            assert_eq!(created_after, None);
            assert_eq!(created_before, None);
            assert_eq!(modified_after, None);
            assert_eq!(modified_before, None);
            assert_eq!(min_size, None);
            assert_eq!(max_size, None);
            assert_eq!(max_results, None);
            assert_eq!(head, None);
            assert_eq!(tail, None);
            assert_eq!(sort_by, None);
            assert!(!reverse);
        } else {
            panic!("Expected Ls command");
        }
    }

    #[test]
    fn test_cp_command_parsing() {
        let args = Args::parse_from([
            "obsctl",
            "cp",
            "./local",
            "s3://bucket/remote",
            "--recursive",
            "--dryrun",
            "--force",
            "--max-concurrent",
            "8",
        ]);

        if let Commands::Cp {
            source,
            dest,
            recursive,
            dryrun,
            max_concurrent,
            force,
            ..
        } = args.command
        {
            assert_eq!(source, "./local");
            assert_eq!(dest, "s3://bucket/remote");
            assert!(recursive);
            assert!(dryrun);
            assert!(force);
            assert_eq!(max_concurrent, 8);
        } else {
            panic!("Expected Cp command");
        }
    }

    #[test]
    fn test_sync_command_parsing() {
        let args = Args::parse_from([
            "obsctl",
            "sync",
            "./local",
            "s3://bucket/remote",
            "--delete",
            "--include",
            "*.log",
            "--exclude",
            "*.tmp",
        ]);

        if let Commands::Sync {
            source,
            dest,
            delete,
            include,
            exclude,
            ..
        } = args.command
        {
            assert_eq!(source, "./local");
            assert_eq!(dest, "s3://bucket/remote");
            assert!(delete);
            assert_eq!(include, Some("*.log".to_string()));
            assert_eq!(exclude, Some("*.tmp".to_string()));
        } else {
            panic!("Expected Sync command");
        }
    }

    #[test]
    fn test_rm_command_parsing() {
        let args = Args::parse_from([
            "obsctl",
            "rm",
            "s3://bucket/file",
            "--recursive",
            "--dryrun",
        ]);

        if let Commands::Rm {
            s3_uri,
            recursive,
            dryrun,
            ..
        } = args.command
        {
            assert_eq!(s3_uri, "s3://bucket/file");
            assert!(recursive);
            assert!(dryrun);
        } else {
            panic!("Expected Rm command");
        }
    }

    #[test]
    fn test_mb_command_parsing() {
        let args = Args::parse_from(["obsctl", "mb", "s3://new-bucket"]);

        if let Commands::Mb { s3_uri } = args.command {
            assert_eq!(s3_uri, "s3://new-bucket");
        } else {
            panic!("Expected Mb command");
        }
    }

    #[test]
    fn test_rb_command_parsing() {
        let args = Args::parse_from(["obsctl", "rb", "s3://old-bucket", "--force"]);

        if let Commands::Rb {
            s3_uri,
            force,
            all,
            confirm,
            pattern,
        } = args.command
        {
            assert_eq!(s3_uri, Some("s3://old-bucket".to_string()));
            assert!(force);
            assert!(!all);
            assert!(!confirm);
            assert_eq!(pattern, None);
        } else {
            panic!("Expected Rb command");
        }
    }

    #[test]
    fn test_presign_command_parsing() {
        let args = Args::parse_from([
            "obsctl",
            "presign",
            "s3://bucket/file",
            "--expires-in",
            "7200",
        ]);

        if let Commands::Presign { s3_uri, expires_in } = args.command {
            assert_eq!(s3_uri, "s3://bucket/file");
            assert_eq!(expires_in, 7200);
        } else {
            panic!("Expected Presign command");
        }
    }

    #[test]
    fn test_head_object_command_parsing() {
        let args = Args::parse_from([
            "obsctl",
            "head-object",
            "--bucket",
            "my-bucket",
            "--key",
            "my-key",
        ]);

        if let Commands::HeadObject { bucket, key } = args.command {
            assert_eq!(bucket, "my-bucket");
            assert_eq!(key, "my-key");
        } else {
            panic!("Expected HeadObject command");
        }
    }

    #[test]
    fn test_du_command_parsing() {
        let args = Args::parse_from([
            "obsctl",
            "du",
            "s3://bucket/path",
            "--human-readable",
            "--summarize",
        ]);

        if let Commands::Du {
            s3_uri,
            human_readable,
            summarize,
        } = args.command
        {
            assert_eq!(s3_uri, "s3://bucket/path");
            assert!(human_readable);
            assert!(summarize);
        } else {
            panic!("Expected Du command");
        }
    }

    #[test]
    fn test_global_args_parsing() {
        let args = Args::parse_from([
            "obsctl",
            "--debug",
            "trace",
            "--endpoint",
            "https://custom.endpoint.com",
            "--region",
            "us-west-2",
            "--timeout",
            "30",
            "ls",
            "s3://bucket",
        ]);

        assert_eq!(args.debug, "trace");
        assert_eq!(
            args.endpoint,
            Some("https://custom.endpoint.com".to_string())
        );
        assert_eq!(args.region, "us-west-2");
        assert_eq!(args.timeout, 30);
    }

    #[test]
    fn test_default_values() {
        let args = Args::parse_from(["obsctl", "ls", "s3://bucket"]);

        assert_eq!(args.debug, "info");
        assert_eq!(args.endpoint, None);
        assert_eq!(args.region, "ru-moscow-1");
        assert_eq!(args.timeout, 10);
    }

    #[test]
    fn test_cp_with_filters() {
        let args = Args::parse_from([
            "obsctl",
            "cp",
            "./src",
            "s3://bucket/dest",
            "--include",
            "*.rs",
            "--exclude",
            "target/*",
        ]);

        if let Commands::Cp {
            include, exclude, ..
        } = args.command
        {
            assert_eq!(include, Some("*.rs".to_string()));
            assert_eq!(exclude, Some("target/*".to_string()));
        } else {
            panic!("Expected Cp command");
        }
    }

    #[test]
    fn test_config_command_parsing() {
        // Test config command with no subcommand (show all)
        let args = Args::parse_from(["obsctl", "config"]);

        if let Commands::Config { command } = args.command {
            assert!(command.is_none());
        } else {
            panic!("Expected Config command");
        }
    }

    #[test]
    fn test_config_command_with_subcommands() {
        // Test config configure subcommand
        let args = Args::parse_from(["obsctl", "config", "configure", "--profile", "dev"]);

        if let Commands::Config { command } = args.command {
            if let Some(ConfigCommands::Configure { profile }) = command {
                assert_eq!(profile, "dev");
            } else {
                panic!("Expected Configure subcommand");
            }
        } else {
            panic!("Expected Config command");
        }

        // Test config set subcommand
        let args = Args::parse_from([
            "obsctl",
            "config",
            "set",
            "region",
            "us-west-2",
            "--profile",
            "production",
        ]);

        if let Commands::Config { command } = args.command {
            if let Some(ConfigCommands::Set {
                key,
                value,
                profile,
            }) = command
            {
                assert_eq!(key, "region");
                assert_eq!(value, "us-west-2");
                assert_eq!(profile, "production");
            } else {
                panic!("Expected Set subcommand");
            }
        } else {
            panic!("Expected Config command");
        }

        // Test config dashboard install subcommand
        let args = Args::parse_from([
            "obsctl",
            "config",
            "dashboard",
            "install",
            "--url",
            "http://grafana.example.com:3000",
        ]);

        if let Commands::Config { command } = args.command {
            if let Some(ConfigCommands::Dashboard {
                command: dashboard_cmd,
            }) = command
            {
                if let DashboardCommands::Install {
                    url,
                    username,
                    password,
                    org_id,
                    folder,
                    force,
                } = dashboard_cmd
                {
                    assert_eq!(url, "http://grafana.example.com:3000");
                    assert_eq!(username, "admin");
                    assert_eq!(password, "admin");
                    assert_eq!(org_id, "1");
                    assert_eq!(folder, "obsctl");
                    assert!(!force);
                } else {
                    panic!("Expected Dashboard Install subcommand");
                }
            } else {
                panic!("Expected Dashboard subcommand");
            }
        } else {
            panic!("Expected Config command");
        }
    }
}
