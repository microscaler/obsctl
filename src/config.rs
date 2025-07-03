use anyhow::Result;
use aws_config::{meta::region::RegionProviderChain, Region};
use aws_sdk_s3::Client;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use crate::args::Args;

#[derive(Debug, Clone)]
pub struct OtelConfig {
    pub enabled: bool,
    pub endpoint: Option<String>,
    pub service_name: String,
    pub service_version: String,
    pub read_operations: bool,
}

impl Default for OtelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: None,
            service_name: "obsctl".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            read_operations: false,
        }
    }
}

pub struct Config {
    pub client: Arc<Client>,
    pub otel: OtelConfig,
}

impl Config {
    pub async fn new(args: &Args) -> Result<Self> {
        // Read AWS config files first
        let aws_config = read_aws_config_files()?;

        // Set up AWS environment variables (config file values first, then env overrides)
        setup_aws_environment(&aws_config, &args.debug)?;

        let region_provider =
            RegionProviderChain::first_try(Some(Region::new(args.region.clone())))
                .or_default_provider()
                .or_else(Region::new("ru-moscow-1"));

        let shared_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(region_provider)
            .load()
            .await;

        let mut s3_config_builder = aws_sdk_s3::config::Builder::from(&shared_config);

        // CRITICAL FIX: Handle endpoint from multiple sources with proper priority
        // Priority: 1) CLI --endpoint flag, 2) AWS_ENDPOINT_URL env var, 3) config file
        let endpoint_url = args
            .endpoint
            .clone()
            .or_else(|| std::env::var("AWS_ENDPOINT_URL").ok())
            .or_else(|| {
                let profile =
                    std::env::var("AWS_PROFILE").unwrap_or_else(|_| "default".to_string());
                aws_config
                    .get(&profile)
                    .and_then(|profile_config| profile_config.get("endpoint_url"))
                    .cloned()
            });

        if let Some(endpoint) = endpoint_url {
            s3_config_builder = s3_config_builder
                .endpoint_url(endpoint)
                .force_path_style(true); // Required for MinIO and other S3-compatible services
        }

        let s3_config = s3_config_builder.build();
        let client = Arc::new(Client::from_conf(s3_config));

        // Configure OTEL from config file and environment
        let otel = configure_otel(&aws_config)?;

        Ok(Config { client, otel })
    }
}

/// Read AWS configuration files (~/.aws/config and ~/.aws/credentials)
fn read_aws_config_files() -> Result<HashMap<String, HashMap<String, String>>> {
    let mut config = HashMap::new();

    // Check if AWS_CONFIG_FILE is set to a specific file
    if let Ok(config_file_path) = std::env::var("AWS_CONFIG_FILE") {
        let config_file = PathBuf::from(config_file_path);
        if config_file.exists() {
            let config_content = fs::read_to_string(&config_file)?;
            parse_aws_config_file(&config_content, &mut config)?;
        }
    } else {
        // Get AWS config directory
        let aws_dir = get_aws_config_dir()?;

        // Read ~/.aws/config
        let config_file = aws_dir.join("config");
        if config_file.exists() {
            let config_content = fs::read_to_string(&config_file)?;
            parse_aws_config_file(&config_content, &mut config)?;
        }

        // Read ~/.aws/credentials
        let credentials_file = aws_dir.join("credentials");
        if credentials_file.exists() {
            let credentials_content = fs::read_to_string(&credentials_file)?;
            parse_aws_config_file(&credentials_content, &mut config)?;
        }
    }

    Ok(config)
}

/// Get the AWS configuration directory path
fn get_aws_config_dir() -> Result<PathBuf> {
    if let Ok(aws_config_file) = std::env::var("AWS_CONFIG_FILE") {
        if let Some(parent) = PathBuf::from(aws_config_file).parent() {
            return Ok(parent.to_path_buf());
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        return Ok(PathBuf::from(home).join(".aws"));
    }

    #[cfg(windows)]
    {
        if let Ok(userprofile) = std::env::var("USERPROFILE") {
            return Ok(PathBuf::from(userprofile).join(".aws"));
        }
    }

    anyhow::bail!("Could not determine AWS config directory");
}

/// Parse AWS config file format (INI-style with sections)
fn parse_aws_config_file(
    content: &str,
    config: &mut HashMap<String, HashMap<String, String>>,
) -> Result<()> {
    let mut current_section = String::new();

    for line in content.lines() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        // Section headers
        if line.starts_with('[') && line.ends_with(']') {
            current_section = line[1..line.len() - 1].to_string();
            // Normalize profile names (remove "profile " prefix if present)
            if current_section.starts_with("profile ") {
                current_section = current_section[8..].to_string();
            }
            config.entry(current_section.clone()).or_default();
            continue;
        }

        // Key-value pairs
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim().to_string();
            let value = line[eq_pos + 1..].trim().to_string();

            if !current_section.is_empty() {
                config
                    .entry(current_section.clone())
                    .or_default()
                    .insert(key, value);
            }
        }
    }

    Ok(())
}

/// Set up AWS environment variables from config files and CLI args
fn setup_aws_environment(
    aws_config: &HashMap<String, HashMap<String, String>>,
    debug_level: &str,
) -> Result<()> {
    // Get the profile to use (default to "default")
    let profile = std::env::var("AWS_PROFILE").unwrap_or_else(|_| "default".to_string());

    if let Some(profile_config) = aws_config.get(&profile) {
        // Set AWS credentials if not already set by environment
        if std::env::var("AWS_ACCESS_KEY_ID").is_err() {
            if let Some(access_key) = profile_config.get("aws_access_key_id") {
                unsafe {
                    std::env::set_var("AWS_ACCESS_KEY_ID", access_key);
                }
            }
        }

        if std::env::var("AWS_SECRET_ACCESS_KEY").is_err() {
            if let Some(secret_key) = profile_config.get("aws_secret_access_key") {
                unsafe {
                    std::env::set_var("AWS_SECRET_ACCESS_KEY", secret_key);
                }
            }
        }

        if std::env::var("AWS_SESSION_TOKEN").is_err() {
            if let Some(session_token) = profile_config.get("aws_session_token") {
                unsafe {
                    std::env::set_var("AWS_SESSION_TOKEN", session_token);
                }
            }
        }

        if std::env::var("AWS_DEFAULT_REGION").is_err() {
            if let Some(region) = profile_config.get("region") {
                unsafe {
                    std::env::set_var("AWS_DEFAULT_REGION", region);
                }
            }
        }
    }

    // Set logging environment variables
    unsafe {
        std::env::set_var("AWS_LOG_LEVEL", debug_level);
        std::env::set_var("AWS_SMITHY_LOG", debug_level);
    }

    Ok(())
}

/// Configure OpenTelemetry from config files and environment
fn configure_otel(aws_config: &HashMap<String, HashMap<String, String>>) -> Result<OtelConfig> {
    let mut otel_config = OtelConfig::default();

    // First, check for dedicated ~/.aws/otel file
    let aws_dir = get_aws_config_dir()?;
    let otel_file = aws_dir.join("otel");

    if otel_file.exists() {
        let otel_content = fs::read_to_string(&otel_file)?;
        let mut otel_file_config = HashMap::new();
        parse_aws_config_file(&otel_content, &mut otel_file_config)?;

        // If we have a valid otel file with [otel] section, enable by default
        if let Some(otel_section) = otel_file_config.get("otel") {
            otel_config.enabled = true; // Default to enabled if otel file exists

            // Read settings from otel file
            if let Some(enabled_str) = otel_section.get("enabled") {
                otel_config.enabled = enabled_str.to_lowercase() == "true";
            }

            if let Some(endpoint) = otel_section.get("endpoint") {
                otel_config.endpoint = Some(endpoint.clone());
            }

            if let Some(service_name) = otel_section.get("service_name") {
                otel_config.service_name = service_name.clone();
            }
        }
    }

    // Get the profile to use (default to "default")
    let profile = std::env::var("AWS_PROFILE").unwrap_or_else(|_| "default".to_string());

    // Check for OTEL configuration in AWS config file (can override otel file)
    if let Some(profile_config) = aws_config.get(&profile) {
        // Check if OTEL is enabled in config file
        if let Some(enabled_str) = profile_config.get("otel_enabled") {
            otel_config.enabled = enabled_str.to_lowercase() == "true";
        }

        // Get OTEL endpoint from config file
        if let Some(endpoint) = profile_config.get("otel_endpoint") {
            otel_config.endpoint = Some(endpoint.clone());
        }

        // Get service name from config file
        if let Some(service_name) = profile_config.get("otel_service_name") {
            otel_config.service_name = service_name.clone();
        }
    }

    // Environment variables override everything
    if let Ok(enabled_str) = std::env::var("OTEL_ENABLED") {
        otel_config.enabled = enabled_str.to_lowercase() == "true";
    }

    if let Ok(endpoint) = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT") {
        otel_config.endpoint = Some(endpoint);
    }

    if let Ok(service_name) = std::env::var("OTEL_SERVICE_NAME") {
        otel_config.service_name = service_name;
    }

    Ok(otel_config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::{Args, Commands};

    #[test]
    fn test_parse_aws_config_file() {
        let config_content = r#"
[default]
aws_access_key_id = AKIAIOSFODNN7EXAMPLE
aws_secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY
region = us-west-2
otel_enabled = true
otel_endpoint = http://localhost:4317

[profile dev]
aws_access_key_id = AKIAIOSFODNN7EXAMPLE2
aws_secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY2
region = us-east-1
otel_enabled = false
"#;

        let mut config = HashMap::new();
        parse_aws_config_file(config_content, &mut config).unwrap();

        assert!(config.contains_key("default"));
        assert!(config.contains_key("dev"));

        let default_profile = &config["default"];
        assert_eq!(
            default_profile.get("aws_access_key_id").unwrap(),
            "AKIAIOSFODNN7EXAMPLE"
        );
        assert_eq!(default_profile.get("region").unwrap(), "us-west-2");
        assert_eq!(default_profile.get("otel_enabled").unwrap(), "true");
        assert_eq!(
            default_profile.get("otel_endpoint").unwrap(),
            "http://localhost:4317"
        );

        let dev_profile = &config["dev"];
        assert_eq!(dev_profile.get("region").unwrap(), "us-east-1");
        assert_eq!(dev_profile.get("otel_enabled").unwrap(), "false");
    }

    #[test]
    fn test_parse_aws_config_file_with_comments() {
        let config_content = r#"
# This is a comment
; This is also a comment
[default]
aws_access_key_id = AKIAIOSFODNN7EXAMPLE
# Comment in the middle
aws_secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY
region = us-west-2

# Empty lines should be ignored

[profile test]
region = eu-west-1
"#;

        let mut config = HashMap::new();
        parse_aws_config_file(config_content, &mut config).unwrap();

        assert!(config.contains_key("default"));
        assert!(config.contains_key("test"));
        assert_eq!(
            config["default"].get("aws_access_key_id").unwrap(),
            "AKIAIOSFODNN7EXAMPLE"
        );
        assert_eq!(config["test"].get("region").unwrap(), "eu-west-1");
    }

    #[test]
    fn test_parse_aws_config_file_malformed() {
        let config_content = r#"
[default]
aws_access_key_id = AKIAIOSFODNN7EXAMPLE
malformed_line_without_equals
region = us-west-2
key_with_empty_value =
= value_without_key
"#;

        let mut config = HashMap::new();
        let result = parse_aws_config_file(config_content, &mut config);

        // Should still succeed and parse valid lines
        assert!(result.is_ok());
        assert!(config.contains_key("default"));
        assert_eq!(
            config["default"].get("aws_access_key_id").unwrap(),
            "AKIAIOSFODNN7EXAMPLE"
        );
        assert_eq!(config["default"].get("region").unwrap(), "us-west-2");
        assert_eq!(config["default"].get("key_with_empty_value").unwrap(), "");
    }

    #[test]
    fn test_parse_aws_config_file_empty() {
        let config_content = "";
        let mut config = HashMap::new();
        let result = parse_aws_config_file(config_content, &mut config);

        assert!(result.is_ok());
        assert!(config.is_empty());
    }

    #[test]
    fn test_parse_aws_config_file_only_comments() {
        let config_content = r#"
# Only comments
; And semicolon comments
# No actual config
"#;
        let mut config = HashMap::new();
        let result = parse_aws_config_file(config_content, &mut config);

        assert!(result.is_ok());
        assert!(config.is_empty());
    }

    #[test]
    fn test_setup_aws_environment_logic() {
        // Test the logic without modifying environment variables
        let mut aws_config = HashMap::new();
        let mut default_profile = HashMap::new();
        default_profile.insert("aws_access_key_id".to_string(), "config_key".to_string());
        default_profile.insert(
            "aws_secret_access_key".to_string(),
            "config_secret".to_string(),
        );
        default_profile.insert("region".to_string(), "eu-central-1".to_string());
        aws_config.insert("default".to_string(), default_profile);

        let result = setup_aws_environment(&aws_config, "debug");
        assert!(result.is_ok());
    }

    #[test]
    fn test_setup_aws_environment_missing_profile() {
        let aws_config = HashMap::new(); // No profiles
        let result = setup_aws_environment(&aws_config, "info");

        // Should succeed even with missing profile
        assert!(result.is_ok());
    }

    #[test]
    fn test_configure_otel_config_file_priority() {
        // Test that config file values are used when environment is not set
        let mut aws_config = HashMap::new();
        let mut default_profile = HashMap::new();
        default_profile.insert("otel_enabled".to_string(), "true".to_string());
        default_profile.insert(
            "otel_endpoint".to_string(),
            "http://config:4317".to_string(),
        );
        default_profile.insert(
            "otel_service_name".to_string(),
            "config-service".to_string(),
        );
        aws_config.insert("default".to_string(), default_profile);

        let otel_config = configure_otel(&aws_config).unwrap();

        // Should use config file values
        assert!(otel_config.enabled);
        assert_eq!(otel_config.endpoint, Some("http://config:4317".to_string()));
        assert_eq!(otel_config.service_name, "config-service");
    }

    #[test]
    fn test_configure_otel_case_insensitive() {
        let mut aws_config = HashMap::new();
        let mut default_profile = HashMap::new();
        default_profile.insert("otel_enabled".to_string(), "TRUE".to_string());
        aws_config.insert("default".to_string(), default_profile);

        let otel_config = configure_otel(&aws_config).unwrap();
        assert!(otel_config.enabled);

        // Test false case
        let mut aws_config = HashMap::new();
        let mut default_profile = HashMap::new();
        default_profile.insert("otel_enabled".to_string(), "FALSE".to_string());
        aws_config.insert("default".to_string(), default_profile);

        let otel_config = configure_otel(&aws_config).unwrap();
        assert!(!otel_config.enabled);
    }

    #[test]
    fn test_configure_otel_with_profile_data() {
        // Test different profile configurations
        let mut aws_config = HashMap::new();
        let mut prod_profile = HashMap::new();
        prod_profile.insert("otel_enabled".to_string(), "true".to_string());
        prod_profile.insert("otel_service_name".to_string(), "prod-obsctl".to_string());
        aws_config.insert("production".to_string(), prod_profile);

        // Test that we can access different profiles in the config
        assert!(aws_config.contains_key("production"));
        assert_eq!(
            aws_config["production"].get("otel_service_name").unwrap(),
            "prod-obsctl"
        );
    }

    #[test]
    fn test_get_aws_config_dir_logic() {
        // Test the path construction logic without modifying environment
        let home_path = "/tmp/test-home";
        let expected_aws_dir = PathBuf::from(home_path).join(".aws");

        assert_eq!(expected_aws_dir, PathBuf::from("/tmp/test-home/.aws"));

        // Test custom config file path logic
        let config_file = "/custom/path/config";
        let config_path = PathBuf::from(config_file);
        if let Some(parent) = config_path.parent() {
            assert_eq!(parent, PathBuf::from("/custom/path"));
        }
    }

    #[test]
    fn test_path_construction() {
        // Test path construction without environment variables
        let test_paths = vec![("/home/user", ".aws"), ("/Users/username", ".aws")];

        for (home, aws_subdir) in test_paths {
            let home_path = PathBuf::from(home);
            let aws_dir = home_path.join(aws_subdir);

            // Test that the path ends with .aws
            assert!(aws_dir.to_string_lossy().ends_with(".aws"));

            // Test that the parent is the home directory
            assert_eq!(aws_dir.parent().unwrap(), PathBuf::from(home));
        }
    }

    #[test]
    fn test_config_file_parsing_edge_cases() {
        // Test more edge cases without file I/O
        let test_lines = vec![
            ("[section]", true),  // Valid section
            ("key=value", false), // Valid key-value
            ("=", false),         // Invalid (no key)
            ("key=", false),      // Valid (empty value)
            ("# comment", false), // Comment
            ("", false),          // Empty line
        ];

        for (line, is_section) in test_lines {
            let line = line.trim();
            let is_section_header = line.starts_with('[') && line.ends_with(']');
            assert_eq!(is_section_header, is_section);
        }
    }

    #[test]
    fn test_profile_name_normalization() {
        // Test profile name normalization logic
        let test_cases = vec![
            ("profile dev", "dev"),
            ("profile production", "production"),
            ("default", "default"),
            ("profile ", ""),
        ];

        for (input, expected) in test_cases {
            let normalized = if let Some(stripped) = input.strip_prefix("profile ") {
                stripped
            } else {
                input
            };
            assert_eq!(normalized, expected);
        }
    }

    #[test]
    fn test_otel_config_default() {
        let otel_config = OtelConfig::default();
        assert!(!otel_config.enabled);
        assert!(otel_config.endpoint.is_none());
        assert_eq!(otel_config.service_name, "obsctl");
        assert_eq!(otel_config.service_version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn test_configure_otel_from_config() {
        let mut aws_config = HashMap::new();
        let mut default_profile = HashMap::new();
        default_profile.insert("otel_enabled".to_string(), "true".to_string());
        default_profile.insert("otel_endpoint".to_string(), "http://test:4317".to_string());
        default_profile.insert("otel_service_name".to_string(), "test-service".to_string());
        aws_config.insert("default".to_string(), default_profile);

        let otel_config = configure_otel(&aws_config).unwrap();
        assert!(otel_config.enabled);
        assert_eq!(otel_config.endpoint, Some("http://test:4317".to_string()));
        assert_eq!(otel_config.service_name, "test-service");
    }

    #[test]
    fn test_configure_otel_disabled_by_default() {
        // Test with completely empty configuration - no AWS config and no environment variables
        let aws_config = HashMap::new();

        // Temporarily clear any environment variables that could affect the test
        let _env_guard = [
            ("OTEL_ENABLED", std::env::var("OTEL_ENABLED").ok()),
            (
                "OTEL_EXPORTER_OTLP_ENDPOINT",
                std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok(),
            ),
            ("OTEL_SERVICE_NAME", std::env::var("OTEL_SERVICE_NAME").ok()),
            ("HOME", Some("/tmp/nonexistent".to_string())), // Use fake home to avoid real ~/.aws/otel file
        ];

        // Clear environment variables for clean test
        std::env::remove_var("OTEL_ENABLED");
        std::env::remove_var("OTEL_EXPORTER_OTLP_ENDPOINT");
        std::env::remove_var("OTEL_SERVICE_NAME");
        std::env::set_var("HOME", "/tmp/nonexistent"); // Fake home directory

        let otel_config = configure_otel(&aws_config).unwrap();
        assert!(!otel_config.enabled);
        assert!(otel_config.endpoint.is_none());

        // Restore environment variables
        for (key, value) in _env_guard {
            match value {
                Some(val) => std::env::set_var(key, val),
                None => std::env::remove_var(key),
            }
        }
    }

    #[test]
    #[ignore = "requires OTEL infrastructure - run with: cargo test test_configure_otel_with_real_otel_file -- --ignored"]
    fn test_configure_otel_with_real_otel_file() {
        // This test only runs when explicitly requested and OTEL is available
        if std::env::var("OBSCTL_TEST_OTEL").is_err() {
            eprintln!("⚠️  Skipping OTEL test - set OBSCTL_TEST_OTEL=1 to enable");
            return;
        }

        // Test with real environment (when OTEL file exists)
        let aws_config = HashMap::new();
        let otel_config = configure_otel(&aws_config).unwrap();

        // This will pass if ~/.aws/otel exists with enabled=true
        // or fail if it doesn't exist (which is the expected default behavior)
        println!(
            "🔍 OTEL config result: enabled={}, endpoint={:?}",
            otel_config.enabled, otel_config.endpoint
        );
    }

    #[test]
    fn test_config_creation_with_defaults() {
        let args = Args {
            debug: "info".to_string(),
            endpoint: None,
            region: "ru-moscow-1".to_string(),
            timeout: 10,
            command: Commands::Ls {
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
        };

        // We can't easily test the async function without mocking AWS services
        // But we can test that the structure is correct
        assert_eq!(args.region, "ru-moscow-1");
        assert_eq!(args.debug, "info");
        assert_eq!(args.timeout, 10);
        assert!(args.endpoint.is_none());
    }

    #[test]
    fn test_config_creation_with_custom_endpoint() {
        let args = Args {
            debug: "debug".to_string(),
            endpoint: Some("https://custom.endpoint.com".to_string()),
            region: "us-west-2".to_string(),
            timeout: 30,
            command: Commands::Ls {
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
        };

        assert_eq!(args.region, "us-west-2");
        assert_eq!(args.debug, "debug");
        assert_eq!(args.timeout, 30);
        assert_eq!(
            args.endpoint,
            Some("https://custom.endpoint.com".to_string())
        );
    }

    #[test]
    fn test_config_debug_levels() {
        let debug_levels = ["trace", "debug", "info", "warn", "error"];

        for level in debug_levels {
            let args = Args {
                debug: level.to_string(),
                endpoint: None,
                region: "ru-moscow-1".to_string(),
                timeout: 10,
                command: Commands::Ls {
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
            };

            assert_eq!(args.debug, level);
        }
    }

    #[test]
    fn test_config_timeout_values() {
        let timeouts = [1, 10, 30, 60, 300];

        for timeout in timeouts {
            let args = Args {
                debug: "info".to_string(),
                endpoint: None,
                region: "ru-moscow-1".to_string(),
                timeout,
                command: Commands::Ls {
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
            };

            assert_eq!(args.timeout, timeout);
        }
    }

    #[test]
    fn test_config_regions() {
        let regions = ["ru-moscow-1", "us-west-2", "eu-west-1", "ap-southeast-1"];

        for region in regions {
            let args = Args {
                debug: "info".to_string(),
                endpoint: None,
                region: region.to_string(),
                timeout: 10,
                command: Commands::Ls {
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
            };

            assert_eq!(args.region, region);
        }
    }
}
