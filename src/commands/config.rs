use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use colored::Colorize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::args::{ConfigCommands, DashboardCommands};

/// Execute config command based on subcommand
pub async fn execute(command: Option<ConfigCommands>) -> Result<()> {
    match command {
        Some(ConfigCommands::Configure { profile }) => configure_interactive(&profile).await,
        Some(ConfigCommands::Set {
            key,
            value,
            profile,
        }) => set_config_value(&key, &value, &profile).await,
        Some(ConfigCommands::Get { key, profile }) => get_config_value(&key, &profile).await,
        Some(ConfigCommands::List { profile, files }) => list_config(&profile, files).await,
        Some(ConfigCommands::Dashboard { command }) => execute_dashboard_command(command).await,
        Some(ConfigCommands::Example) => show_config_file_example().await,
        Some(ConfigCommands::Env) => show_environment_variables().await,
        Some(ConfigCommands::Otel) => show_otel_configuration().await,
        None => show_all_config_help().await,
    }
}

/// Execute dashboard management commands
async fn execute_dashboard_command(command: DashboardCommands) -> Result<()> {
    match command {
        DashboardCommands::Install {
            url,
            username,
            password,
            org_id,
            folder,
            force,
        } => install_dashboards(&url, &username, &password, &org_id, &folder, force).await,
        DashboardCommands::List {
            url,
            username,
            password,
        } => list_dashboards(&url, &username, &password).await,
        DashboardCommands::Remove {
            url,
            username,
            password,
            confirm,
        } => remove_dashboards(&url, &username, &password, confirm).await,
        DashboardCommands::Info => show_dashboard_info().await,
        DashboardCommands::System => show_system_info().await,
    }
}

/// Interactive configuration setup (equivalent to aws configure)
async fn configure_interactive(profile: &str) -> Result<()> {
    let profile_name = profile;

    println!(
        "{}",
        format!("obsctl Configuration Setup - Profile: {profile_name}")
            .bold()
            .blue()
    );
    println!("{}", "================================".blue());
    println!();

    // Get current values
    let current_config = load_config_for_profile(profile_name)?;
    let current_credentials = load_credentials_for_profile(profile_name)?;

    // Prompt for each value
    let access_key = prompt_for_value(
        "AWS Access Key ID",
        current_credentials.get("aws_access_key_id"),
        false,
    )?;

    let secret_key = prompt_for_value(
        "AWS Secret Access Key",
        current_credentials.get("aws_secret_access_key"),
        true, // Hide input for secret
    )?;

    let region = prompt_for_value(
        "Default region name",
        current_config
            .get("region")
            .or(Some(&"ru-moscow-1".to_string())),
        false,
    )?;

    let endpoint = prompt_for_value(
        "Default endpoint URL",
        current_config.get("endpoint_url"),
        false,
    )?;

    // Save credentials
    if !access_key.is_empty() {
        set_credential_value("aws_access_key_id", &access_key, profile_name).await?;
    }
    if !secret_key.is_empty() {
        set_credential_value("aws_secret_access_key", &secret_key, profile_name).await?;
    }

    // Save config
    if !region.is_empty() {
        set_config_file_value("region", &region, profile_name).await?;
    }
    if !endpoint.is_empty() {
        set_config_file_value("endpoint_url", &endpoint, profile_name).await?;
    }

    println!();
    println!("{}", "✅ Configuration saved successfully!".green().bold());
    println!("Profile: {}", profile_name.cyan());
    println!(
        "Config file: {}",
        get_config_file_path()?.display().to_string().dimmed()
    );
    println!(
        "Credentials file: {}",
        get_credentials_file_path()?.display().to_string().dimmed()
    );

    Ok(())
}

/// Set a configuration value
async fn set_config_value(key: &str, value: &str, profile: &str) -> Result<()> {
    let profile_name = profile;

    // Determine if this is a credential or config value
    if is_credential_key(key) {
        set_credential_value(key, value, profile_name).await?;
        println!(
            "{} {} = {}",
            "✅ Set credential:".green(),
            key.cyan(),
            value.yellow()
        );
    } else {
        set_config_file_value(key, value, profile_name).await?;
        println!(
            "{} {} = {}",
            "✅ Set config:".green(),
            key.cyan(),
            value.yellow()
        );
    }

    Ok(())
}

/// Get a configuration value
async fn get_config_value(key: &str, profile: &str) -> Result<()> {
    let profile_name = profile;

    let value = if is_credential_key(key) {
        let credentials = load_credentials_for_profile(profile_name)?;
        credentials.get(key).cloned()
    } else {
        let config = load_config_for_profile(profile_name)?;
        config.get(key).cloned()
    };

    match value {
        Some(val) => {
            if is_secret_key(key) {
                println!("****** (hidden)");
            } else {
                println!("{val}");
            }
        }
        None => {
            println!(
                "{}",
                format!("Key '{key}' not found in profile '{profile_name}'").red()
            );
        }
    }

    Ok(())
}

/// List all configuration values
async fn list_config(profile: &str, show_files: bool) -> Result<()> {
    let profile_name = profile;

    if show_files {
        println!("{}", "Configuration Files:".bold().blue());
        println!(
            "Config: {}",
            get_config_file_path()?.display().to_string().cyan()
        );
        println!(
            "Credentials: {}",
            get_credentials_file_path()?.display().to_string().cyan()
        );
        println!();
    }

    println!(
        "{}",
        format!("Configuration for profile: {profile_name}")
            .bold()
            .blue()
    );
    println!("{}", "=".repeat(30 + profile_name.len()).blue());
    println!();

    // Load and display credentials
    let credentials = load_credentials_for_profile(profile_name)?;
    if !credentials.is_empty() {
        println!("{}", "Credentials:".bold().green());
        for (key, value) in &credentials {
            if is_secret_key(key) {
                println!("  {} = {}", key.cyan(), "****** (hidden)".dimmed());
            } else {
                println!("  {} = {}", key.cyan(), value.yellow());
            }
        }
        println!();
    }

    // Load and display config
    let config = load_config_for_profile(profile_name)?;
    if !config.is_empty() {
        println!("{}", "Configuration:".bold().green());
        for (key, value) in &config {
            println!("  {} = {}", key.cyan(), value.yellow());
        }
        println!();
    }

    if credentials.is_empty() && config.is_empty() {
        println!(
            "{}",
            format!("No configuration found for profile '{profile_name}'").yellow()
        );
        println!(
            "Run {} to set up configuration",
            "obsctl config configure".cyan()
        );
    }

    Ok(())
}

/// Helper functions for file management
fn get_aws_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE"))?;
    Ok(PathBuf::from(home).join(".aws"))
}

fn get_config_file_path() -> Result<PathBuf> {
    Ok(get_aws_dir()?.join("config"))
}

fn get_credentials_file_path() -> Result<PathBuf> {
    Ok(get_aws_dir()?.join("credentials"))
}

fn ensure_aws_dir() -> Result<()> {
    let aws_dir = get_aws_dir()?;
    if !aws_dir.exists() {
        fs::create_dir_all(&aws_dir)?;
    }
    Ok(())
}

fn is_credential_key(key: &str) -> bool {
    matches!(
        key,
        "aws_access_key_id" | "aws_secret_access_key" | "aws_session_token"
    )
}

fn is_secret_key(key: &str) -> bool {
    matches!(key, "aws_secret_access_key" | "aws_session_token")
}

fn load_ini_file(path: &PathBuf) -> Result<HashMap<String, HashMap<String, String>>> {
    if !path.exists() {
        return Ok(HashMap::new());
    }

    let content = fs::read_to_string(path)?;
    let mut sections = HashMap::new();
    let mut current_section = String::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            current_section = line[1..line.len() - 1].to_string();
            if current_section.starts_with("profile ") {
                current_section = current_section[8..].to_string();
            }
            sections
                .entry(current_section.clone())
                .or_insert_with(HashMap::new);
        } else if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim().to_string();
            let value = line[eq_pos + 1..].trim().to_string();
            if !current_section.is_empty() {
                sections
                    .entry(current_section.clone())
                    .or_insert_with(HashMap::new)
                    .insert(key, value);
            }
        }
    }

    Ok(sections)
}

fn save_ini_file(
    path: &PathBuf,
    sections: &HashMap<String, HashMap<String, String>>,
    is_config: bool,
) -> Result<()> {
    ensure_aws_dir()?;

    let mut content = String::new();

    for (section_name, section_data) in sections {
        if is_config && section_name != "default" {
            content.push_str(&format!("[profile {section_name}]\n"));
        } else {
            content.push_str(&format!("[{section_name}]\n"));
        }

        for (key, value) in section_data {
            content.push_str(&format!("{key} = {value}\n"));
        }
        content.push('\n');
    }

    fs::write(path, content)?;
    Ok(())
}

fn load_config_for_profile(profile: &str) -> Result<HashMap<String, String>> {
    let config_file = get_config_file_path()?;
    let all_config = load_ini_file(&config_file)?;
    Ok(all_config.get(profile).cloned().unwrap_or_default())
}

fn load_credentials_for_profile(profile: &str) -> Result<HashMap<String, String>> {
    let credentials_file = get_credentials_file_path()?;
    let all_credentials = load_ini_file(&credentials_file)?;
    Ok(all_credentials.get(profile).cloned().unwrap_or_default())
}

async fn set_config_file_value(key: &str, value: &str, profile: &str) -> Result<()> {
    let config_file = get_config_file_path()?;
    let mut all_config = load_ini_file(&config_file)?;

    all_config
        .entry(profile.to_string())
        .or_insert_with(HashMap::new)
        .insert(key.to_string(), value.to_string());

    save_ini_file(&config_file, &all_config, true)?;
    Ok(())
}

async fn set_credential_value(key: &str, value: &str, profile: &str) -> Result<()> {
    let credentials_file = get_credentials_file_path()?;
    let mut all_credentials = load_ini_file(&credentials_file)?;

    all_credentials
        .entry(profile.to_string())
        .or_insert_with(HashMap::new)
        .insert(key.to_string(), value.to_string());

    save_ini_file(&credentials_file, &all_credentials, false)?;
    Ok(())
}

fn prompt_for_value(prompt: &str, current: Option<&String>, hide_input: bool) -> Result<String> {
    let current_display = match current {
        Some(_val) if hide_input => " [****** (hidden)]",
        Some(val) => &format!(" [{val}]"),
        None => "",
    };

    print!("{}{}: ", prompt.bold(), current_display.dimmed());
    io::stdout().flush()?;

    let mut input = String::new();
    if hide_input {
        // For secrets, we'll still use regular input for simplicity
        // In a production tool, you'd want to use a crate like `rpassword`
        io::stdin().read_line(&mut input)?;
    } else {
        io::stdin().read_line(&mut input)?;
    }

    let input = input.trim().to_string();
    if input.is_empty() {
        if let Some(current_value) = current {
            Ok(current_value.clone())
        } else {
            Ok(input)
        }
    } else {
        Ok(input)
    }
}

// Legacy functions for backward compatibility
async fn show_all_config_help() -> Result<()> {
    println!("{}", "obsctl Configuration Guide".bold().blue());
    println!("{}", "========================".blue());
    println!();

    println!("{}", "Configuration Commands:".bold());
    println!("  {} - Interactive setup", "obsctl config configure".cyan());
    println!(
        "  {} - Set configuration value",
        "obsctl config set <key> <value>".cyan()
    );
    println!(
        "  {} - Get configuration value",
        "obsctl config get <key>".cyan()
    );
    println!("  {} - List all configuration", "obsctl config list".cyan());
    println!();

    println!("{}", "Dashboard Commands:".bold());
    println!(
        "  {} - Install obsctl dashboards to Grafana",
        "obsctl config dashboard install".cyan()
    );
    println!(
        "  {} - List obsctl dashboards",
        "obsctl config dashboard list".cyan()
    );
    println!(
        "  {} - Remove obsctl dashboards",
        "obsctl config dashboard remove --confirm".cyan()
    );
    println!(
        "  {} - Show dashboard information",
        "obsctl config dashboard info".cyan()
    );
    println!();

    println!("{}", "Examples:".bold());
    println!("  # Interactive setup");
    println!("  {}", "obsctl config configure".yellow());
    println!();
    println!("  # Set values directly");
    println!(
        "  {}",
        "obsctl config set aws_access_key_id AKIAIOSFODNN7EXAMPLE".yellow()
    );
    println!("  {}", "obsctl config set region us-west-2".yellow());
    println!(
        "  {}",
        "obsctl config set endpoint_url http://localhost:9000".yellow()
    );
    println!();
    println!("  # Use profiles");
    println!("  {}", "obsctl config configure --profile dev".yellow());
    println!(
        "  {}",
        "obsctl config set region eu-west-1 --profile production".yellow()
    );
    println!();
    println!("  # Dashboard management");
    println!("  {}", "obsctl config dashboard install".yellow());
    println!(
        "  {}",
        "obsctl config dashboard install --url http://grafana.company.com:3000".yellow()
    );
    println!("  {}", "obsctl config dashboard list".yellow());
    println!();

    println!("{}", "Additional Help:".bold());
    println!(
        "  {} - Show environment variables",
        "obsctl config env".cyan()
    );
    println!(
        "  {} - Show config file examples",
        "obsctl config example".cyan()
    );
    println!(
        "  {} - Show OpenTelemetry configuration",
        "obsctl config otel".cyan()
    );

    Ok(())
}

async fn show_environment_variables() -> Result<()> {
    println!("{}", "Environment Variables".bold().green());
    println!("{}", "---------------------".green());
    println!();

    println!("{}", "AWS Configuration:".bold());
    println!("  {}=your-access-key", "AWS_ACCESS_KEY_ID".cyan());
    println!("  {}=your-secret-key", "AWS_SECRET_ACCESS_KEY".cyan());
    println!("  {}=http://localhost:9000", "AWS_ENDPOINT_URL".cyan());
    println!("  {}=us-east-1", "AWS_REGION".cyan());
    println!("  {}=production", "AWS_PROFILE".cyan());
    println!();

    println!("{}", "OpenTelemetry Configuration:".bold());
    println!("  {}=true", "OTEL_ENABLED".cyan());
    println!(
        "  {}=http://localhost:4317",
        "OTEL_EXPORTER_OTLP_ENDPOINT".cyan()
    );
    println!("  {}=obsctl-prod", "OTEL_SERVICE_NAME".cyan());
    println!();

    println!("{}", "Usage Examples:".bold());
    println!("  # Use specific endpoint");
    println!(
        "  {} obsctl ls",
        "AWS_ENDPOINT_URL=http://localhost:9000".yellow()
    );
    println!();
    println!("  # Enable OpenTelemetry");
    println!(
        "  {} obsctl cp file.txt s3://bucket/",
        "OTEL_ENABLED=true".yellow()
    );
    println!();
    println!("  # Use different profile");
    println!(
        "  {} obsctl ls s3://bucket",
        "AWS_PROFILE=development".yellow()
    );

    Ok(())
}

async fn show_config_file_example() -> Result<()> {
    println!("{}", "AWS Configuration File Example".bold().green());
    println!("{}", "------------------------------".green());
    println!();

    println!("{}", "~/.aws/config:".bold());
    let config_example = r#"[default]
region = ru-moscow-1
endpoint_url = http://localhost:9000
otel_enabled = false
otel_endpoint = http://localhost:4317
otel_service_name = obsctl

[profile dev]
region = us-west-2
endpoint_url = http://localhost:9000
otel_enabled = true
otel_service_name = obsctl-dev

[profile production]
region = us-east-1
endpoint_url = https://s3.amazonaws.com
otel_enabled = true
otel_endpoint = https://otel-collector.company.com:4317
otel_service_name = obsctl-prod"#;

    println!("{}", config_example.dimmed());
    println!();

    println!("{}", "~/.aws/credentials:".bold());
    let credentials_example = r#"[default]
aws_access_key_id = your-access-key-here
aws_secret_access_key = your-secret-key-here

[dev]
aws_access_key_id = dev-access-key
aws_secret_access_key = dev-secret-key

[production]
aws_access_key_id = prod-access-key
aws_secret_access_key = prod-secret-key"#;

    println!("{}", credentials_example.dimmed());
    println!();

    println!("{}", "Usage with profiles:".bold());
    println!("  {} obsctl ls s3://bucket", "AWS_PROFILE=dev".yellow());
    println!(
        "  {} obsctl cp file.txt s3://bucket/",
        "AWS_PROFILE=production".yellow()
    );

    Ok(())
}

async fn show_otel_configuration() -> Result<()> {
    println!("{}", "OpenTelemetry Configuration".bold().green());
    println!("{}", "---------------------------".green());
    println!();

    println!(
        "{}",
        "obsctl supports OpenTelemetry for observability and metrics.".bold()
    );
    println!();

    println!("{}", "Configuration Methods:".bold());
    println!("  1. {} (recommended)", "Environment Variables".cyan());
    println!("     OTEL_ENABLED=true");
    println!("     OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317");
    println!("     OTEL_SERVICE_NAME=obsctl");
    println!();

    println!("  2. {} (in ~/.aws/config)", "Config File".cyan());
    println!("     otel_enabled = true");
    println!("     otel_endpoint = http://localhost:4317");
    println!("     otel_service_name = obsctl");
    println!();

    println!("  3. {} (using obsctl config)", "Interactive Setup".cyan());
    println!("     obsctl config set otel_enabled true");
    println!("     obsctl config set otel_endpoint http://localhost:4317");
    println!("     obsctl config set otel_service_name obsctl-prod");
    println!();

    println!("{}", "Metrics Exported:".bold());
    println!(
        "  • {} - Total operations performed",
        "obsctl_operations_total".dimmed()
    );
    println!(
        "  • {} - Bytes uploaded/downloaded",
        "obsctl_bytes_*_total".dimmed()
    );
    println!("  • {} - Files processed", "obsctl_files_*_total".dimmed());
    println!(
        "  • {} - Operation duration",
        "obsctl_operation_duration_seconds".dimmed()
    );
    println!(
        "  • {} - Transfer rates",
        "obsctl_transfer_rate_kbps".dimmed()
    );
    println!("  • {} - Bucket analytics", "obsctl_bucket_*".dimmed());
    println!();

    println!("{}", "Quick Test:".bold());
    println!("  {} obsctl ls s3://bucket", "OTEL_ENABLED=true".yellow());
    println!("  # Check metrics at http://localhost:9090 (Prometheus)");

    Ok(())
}

/// Dashboard Management Functions - Restricted to obsctl dashboards only
/// These functions only interact with dashboards that have "obsctl" in their UID or title
/// Install obsctl dashboards to Grafana
async fn install_dashboards(
    url: &str,
    username: &str,
    password: &str,
    _org_id: &str,
    folder: &str,
    force: bool,
) -> Result<()> {
    println!("{}", "Installing obsctl Dashboards".bold().blue());
    println!("{}", "============================".blue());
    println!();

    let client = reqwest::Client::new();
    let auth = general_purpose::STANDARD.encode(format!("{username}:{password}"));

    // First, test connection
    println!("🔗 Testing connection to Grafana...");
    let health_response = client
        .get(format!("{url}/api/health"))
        .header("Authorization", format!("Basic {auth}"))
        .send()
        .await?;

    if !health_response.status().is_success() {
        return Err(anyhow::anyhow!("Failed to connect to Grafana at {}", url));
    }
    println!("{}", "✅ Connected to Grafana successfully".green());

    // Create folder if it doesn't exist
    println!("📁 Creating folder '{folder}'...");
    let folder_payload = json!({
        "title": folder,
        "uid": format!("{}-folder", folder)
    });

    let folder_response = client
        .post(format!("{url}/api/folders"))
        .header("Authorization", format!("Basic {auth}"))
        .header("Content-Type", "application/json")
        .json(&folder_payload)
        .send()
        .await?;

    if folder_response.status().is_success() || folder_response.status().as_u16() == 409 {
        println!("{}", "✅ Folder ready".green());
    } else {
        println!(
            "{}",
            "⚠️  Folder creation warning (may already exist)".yellow()
        );
    }

    // Get embedded dashboard content
    let dashboard_content = get_embedded_dashboard_content();

    if !force {
        // Check if dashboard already exists
        println!("🔍 Checking for existing obsctl dashboards...");
        let search_response = client
            .get(format!("{url}/api/search?query=obsctl"))
            .header("Authorization", format!("Basic {auth}"))
            .send()
            .await?;

        if search_response.status().is_success() {
            let search_results: Value = search_response.json().await?;
            if let Some(results) = search_results.as_array() {
                if !results.is_empty() {
                    println!("{}", "⚠️  Existing obsctl dashboards found:".yellow());
                    for result in results {
                        if let Some(title) = result["title"].as_str() {
                            println!("   - {title}");
                        }
                    }
                    println!("Use {} to overwrite existing dashboards", "--force".cyan());
                    return Ok(());
                }
            }
        }
    }

    // Install the dashboard
    println!("📊 Installing obsctl Unified Dashboard...");
    let dashboard_payload = json!({
        "dashboard": dashboard_content,
        "folderId": null,
        "folderUid": format!("{}-folder", folder),
        "overwrite": force,
        "message": "Installed by obsctl config dashboard install"
    });

    let install_response = client
        .post(format!("{url}/api/dashboards/db"))
        .header("Authorization", format!("Basic {auth}"))
        .header("Content-Type", "application/json")
        .json(&dashboard_payload)
        .send()
        .await?;

    if install_response.status().is_success() {
        let response_data: Value = install_response.json().await?;
        println!("{}", "✅ Dashboard installed successfully!".green().bold());

        if let Some(dashboard_url) = response_data["url"].as_str() {
            println!("🌐 Dashboard URL: {url}{dashboard_url}");
        }

        println!();
        println!("{}", "Dashboard Features:".bold());
        println!("  📊 Business Metrics - Data transfer volumes and rates");
        println!("  ⚡ Performance Metrics - Operations and throughput");
        println!("  🚨 Error Monitoring - Error rates and types");
        println!("  📈 Real-time Updates - 5-second refresh rate");

        Ok(())
    } else {
        let error_text = install_response.text().await?;
        Err(anyhow::anyhow!(
            "Failed to install dashboard: {}",
            error_text
        ))
    }
}

/// List obsctl dashboards (only shows obsctl-related dashboards)
async fn list_dashboards(url: &str, username: &str, password: &str) -> Result<()> {
    println!("{}", "obsctl Dashboards".bold().blue());
    println!("{}", "=================".blue());
    println!();

    let client = reqwest::Client::new();
    let auth = general_purpose::STANDARD.encode(format!("{username}:{password}"));

    // Search for obsctl dashboards only
    let search_response = client
        .get(format!("{url}/api/search?query=obsctl"))
        .header("Authorization", format!("Basic {auth}"))
        .send()
        .await?;

    if !search_response.status().is_success() {
        return Err(anyhow::anyhow!("Failed to connect to Grafana at {}", url));
    }

    let search_results: Value = search_response.json().await?;

    if let Some(results) = search_results.as_array() {
        if results.is_empty() {
            println!("{}", "No obsctl dashboards found".yellow());
            println!(
                "Run {} to install dashboards",
                "obsctl config dashboard install".cyan()
            );
        } else {
            println!(
                "{}",
                format!("Found {} obsctl dashboard(s):", results.len()).green()
            );
            println!();

            for result in results {
                let title = result["title"].as_str().unwrap_or("Unknown");
                let uid = result["uid"].as_str().unwrap_or("Unknown");
                let dashboard_type = result["type"].as_str().unwrap_or("dash-db");
                let folder_title = result["folderTitle"].as_str().unwrap_or("General");

                // Only show if it's actually obsctl-related
                if title.to_lowercase().contains("obsctl") || uid.to_lowercase().contains("obsctl")
                {
                    println!("📊 {}", title.bold());
                    println!("   UID: {}", uid.dimmed());
                    println!("   Type: {}", dashboard_type.dimmed());
                    println!("   Folder: {}", folder_title.dimmed());
                    println!("   URL: {url}/d/{uid}");
                    println!();
                }
            }
        }
    }

    Ok(())
}

/// Remove obsctl dashboards (only removes obsctl dashboards)
async fn remove_dashboards(url: &str, username: &str, password: &str, confirm: bool) -> Result<()> {
    println!("{}", "Remove obsctl Dashboards".bold().red());
    println!("{}", "========================".red());
    println!();

    if !confirm {
        println!(
            "{}",
            "⚠️  This will remove ALL obsctl dashboards from Grafana"
                .yellow()
                .bold()
        );
        println!("Use {} to confirm removal", "--confirm".cyan());
        return Ok(());
    }

    let client = reqwest::Client::new();
    let auth = general_purpose::STANDARD.encode(format!("{username}:{password}"));

    // Search for obsctl dashboards only
    let search_response = client
        .get(format!("{url}/api/search?query=obsctl"))
        .header("Authorization", format!("Basic {auth}"))
        .send()
        .await?;

    if !search_response.status().is_success() {
        return Err(anyhow::anyhow!("Failed to connect to Grafana at {}", url));
    }

    let search_results: Value = search_response.json().await?;

    if let Some(results) = search_results.as_array() {
        if results.is_empty() {
            println!("{}", "No obsctl dashboards found to remove".yellow());
            return Ok(());
        }

        let mut removed_count = 0;

        for result in results {
            let title = result["title"].as_str().unwrap_or("Unknown");
            let uid = result["uid"].as_str().unwrap_or("");

            // Safety check: only remove if it's clearly obsctl-related
            if title.to_lowercase().contains("obsctl") || uid.to_lowercase().contains("obsctl") {
                println!("🗑️  Removing: {title}");

                let delete_response = client
                    .delete(format!("{url}/api/dashboards/uid/{uid}"))
                    .header("Authorization", format!("Basic {auth}"))
                    .send()
                    .await?;

                if delete_response.status().is_success() {
                    println!("   {}", "✅ Removed successfully".green());
                    removed_count += 1;
                } else {
                    println!("   {}", "❌ Failed to remove".red());
                }
            }
        }

        println!();
        println!(
            "{}",
            format!("Removed {removed_count} obsctl dashboard(s)")
                .green()
                .bold()
        );
    }

    Ok(())
}

/// Show obsctl dashboard information
async fn show_dashboard_info() -> Result<()> {
    println!("{}", "obsctl Dashboard Information".bold().blue());
    println!("{}", "============================".blue());
    println!();

    println!("{}", "Dashboard Management:".bold());
    println!(
        "  {} - Install obsctl dashboards",
        "obsctl config dashboard install".cyan()
    );
    println!(
        "  {} - List obsctl dashboards",
        "obsctl config dashboard list".cyan()
    );
    println!(
        "  {} - Remove obsctl dashboards",
        "obsctl config dashboard remove --confirm".cyan()
    );
    println!();

    println!("{}", "Default Installation Paths:".bold());
    println!("  📁 Folder: {}", "obsctl".yellow());
    println!("  🌐 Grafana URL: {}", "http://localhost:3000".yellow());
    println!("  👤 Default Credentials: {}", "admin/admin".yellow());
    println!();

    println!("{}", "Dashboard Features:".bold());
    println!(
        "  📊 {} - Data transfer volumes, rates, file distribution",
        "Business Metrics".green()
    );
    println!(
        "  ⚡ {} - Operations count, throughput, performance",
        "Performance Metrics".green()
    );
    println!(
        "  🚨 {} - Error rates, types, and monitoring",
        "Error Monitoring".green()
    );
    println!(
        "  📈 {} - 5-second auto-refresh",
        "Real-time Updates".green()
    );
    println!();

    println!("{}", "Security Notes:".bold());
    println!("  🔒 Only manages obsctl-specific dashboards");
    println!("  🔍 Searches are restricted to 'obsctl' keyword");
    println!("  ⚠️  Removal requires --confirm flag");
    println!("  📋 Lists only dashboards with 'obsctl' in title/UID");
    println!();

    println!("{}", "Package Installation:".bold());
    let dashboard_path = get_dashboard_installation_path();
    println!(
        "  📂 Dashboard files: {}",
        dashboard_path.display().to_string().dimmed()
    );
    println!("  📦 Installed via: obsctl.deb or obsctl.rpm");

    Ok(())
}

/// Get the path where dashboards are installed by the package
fn get_dashboard_installation_path() -> PathBuf {
    // This will be the path where .deb/.rpm installs the dashboard files
    PathBuf::from("/usr/share/obsctl/dashboards")
}

/// Get embedded dashboard content (this would be the actual dashboard JSON)
fn get_embedded_dashboard_content() -> Value {
    // Try to read from installation path first
    let installation_path = get_dashboard_installation_path().join("obsctl-unified.json");

    if installation_path.exists() {
        match fs::read_to_string(&installation_path) {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(dashboard) => return dashboard,
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to parse dashboard from {}: {}",
                        installation_path.display(),
                        e
                    );
                }
            },
            Err(e) => {
                eprintln!(
                    "Warning: Failed to read dashboard from {}: {}",
                    installation_path.display(),
                    e
                );
            }
        }
    }

    // Fallback to embedded minimal dashboard
    json!({
        "annotations": {
            "list": []
        },
        "editable": true,
        "fiscalYearStartMonth": 0,
        "graphTooltip": 0,
        "id": null,
        "links": [],
        "liveNow": false,
        "panels": [
            {
                "collapsed": false,
                "gridPos": {
                    "h": 1,
                    "w": 24,
                    "x": 0,
                    "y": 0
                },
                "id": 100,
                "panels": [],
                "title": "📊 OBSCTL BUSINESS METRICS",
                "type": "row"
            },
            {
                "datasource": {
                    "type": "prometheus",
                    "uid": "prometheus"
                },
                "description": "Total data transferred OUT (uploaded to S3)",
                "fieldConfig": {
                    "defaults": {
                        "color": {
                            "mode": "thresholds"
                        },
                        "mappings": [],
                        "thresholds": {
                            "steps": [
                                {
                                    "color": "green",
                                    "value": null
                                }
                            ]
                        },
                        "unit": "bytes"
                    }
                },
                "gridPos": {
                    "h": 6,
                    "w": 12,
                    "x": 0,
                    "y": 1
                },
                "id": 1,
                "options": {
                    "colorMode": "value",
                    "graphMode": "area",
                    "justifyMode": "auto",
                    "orientation": "auto",
                    "reduceOptions": {
                        "calcs": ["lastNotNull"],
                        "fields": "",
                        "values": false
                    },
                    "textMode": "auto"
                },
                "targets": [
                    {
                        "datasource": {
                            "type": "prometheus",
                            "uid": "prometheus"
                        },
                        "expr": "obsctl_bytes_uploaded_total",
                        "interval": "",
                        "legendFormat": "Bytes Uploaded",
                        "refId": "A"
                    }
                ],
                "title": "📤 Data Uploaded",
                "type": "stat"
            },
            {
                "datasource": {
                    "type": "prometheus",
                    "uid": "prometheus"
                },
                "description": "Total operations performed",
                "fieldConfig": {
                    "defaults": {
                        "color": {
                            "mode": "thresholds"
                        },
                        "mappings": [],
                        "thresholds": {
                            "steps": [
                                {
                                    "color": "green",
                                    "value": null
                                }
                            ]
                        },
                        "unit": "short"
                    }
                },
                "gridPos": {
                    "h": 6,
                    "w": 12,
                    "x": 12,
                    "y": 1
                },
                "id": 2,
                "options": {
                    "colorMode": "value",
                    "graphMode": "area",
                    "justifyMode": "auto",
                    "orientation": "auto",
                    "reduceOptions": {
                        "calcs": ["lastNotNull"],
                        "fields": "",
                        "values": false
                    },
                    "textMode": "auto"
                },
                "targets": [
                    {
                        "datasource": {
                            "type": "prometheus",
                            "uid": "prometheus"
                        },
                        "expr": "obsctl_operations_total",
                        "interval": "",
                        "legendFormat": "Operations",
                        "refId": "A"
                    }
                ],
                "title": "🔄 Operations",
                "type": "stat"
            }
        ],
        "refresh": "5s",
        "schemaVersion": 39,
        "style": "dark",
        "tags": ["obsctl", "unified", "business", "performance", "errors"],
        "templating": {
            "list": []
        },
        "time": {
            "from": "now-1h",
            "to": "now"
        },
        "timepicker": {
            "refresh_intervals": ["5s", "10s", "30s", "1m", "5m", "15m", "30m", "1h", "2h", "1d"]
        },
        "timezone": "",
        "title": "obsctl Unified Dashboard",
        "uid": "obsctl-unified",
        "version": 1,
        "weekStart": ""
    })
}

/// Show system information including file descriptor monitoring
async fn show_system_info() -> Result<()> {
    use crate::utils::fd_monitor;

    println!("{}", "System Information".bold().blue());
    println!("{}", "===================".blue());
    println!();

    // Basic system info
    println!("{}", "Platform Information:".bold());
    println!("  OS: {}", std::env::consts::OS);
    println!("  Architecture: {}", std::env::consts::ARCH);
    println!("  Family: {}", std::env::consts::FAMILY);
    println!();

    // Process information
    println!("{}", "Process Information:".bold());
    println!("  Process ID: {}", std::process::id());

    // File descriptor monitoring
    println!("{}", "File Descriptor Monitoring:".bold());

    match fd_monitor::get_current_fd_count() {
        Ok(count) => {
            println!("  Current FD/Handle Count: {}", count.to_string().green());

            // Check health
            match fd_monitor::check_fd_health() {
                Ok(healthy) => {
                    if healthy {
                        println!("  Status: {}", "Healthy".green());
                    } else {
                        println!("  Status: {}", "Warning - High Usage".yellow());
                    }
                }
                Err(e) => {
                    println!("  Status: {} - {}", "Error".red(), e);
                }
            }
        }
        Err(e) => {
            println!("  Count: {} - {}", "Error".red(), e);
        }
    }

    // Detailed FD information (if available)
    match fd_monitor::get_fd_info() {
        Ok(fd_info) => {
            println!();
            println!("{}", "Detailed File Descriptor Information:".bold());

            if fd_info.details.len() <= 10 {
                // Show all if reasonable number
                for detail in &fd_info.details {
                    println!("  {detail}");
                }
            } else {
                // Show first 5 and last 5 if too many
                println!(
                    "  {} (showing first 5 and last 5):",
                    format!("Total: {}", fd_info.count).cyan()
                );
                for detail in fd_info.details.iter().take(5) {
                    println!("  {detail}");
                }
                println!("  {}", "...".cyan());
                for detail in fd_info.details.iter().skip(fd_info.details.len() - 5) {
                    println!("  {detail}");
                }
            }
        }
        Err(e) => {
            println!("  Details: {} - {}", "Error".red(), e);
        }
    }

    println!();

    // Platform-specific tips
    println!("{}", "Platform-Specific Notes:".bold());
    match std::env::consts::OS {
        "linux" => {
            println!("  • Uses /proc/self/fd/ for FD monitoring");
            println!("  • Check 'ulimit -n' for FD limits");
            println!(
                "  • Use 'lsof -p {}' for detailed FD info",
                std::process::id()
            );
        }
        "macos" => {
            println!("  • Uses 'lsof' command for FD monitoring");
            println!("  • Check 'ulimit -n' for FD limits");
            println!(
                "  • Use 'lsof -p {}' for detailed FD info",
                std::process::id()
            );
            println!("  • Consider 'sudo launchctl limit maxfiles' for system limits");
        }
        "windows" => {
            println!("  • Uses PowerShell/WMI for handle monitoring");
            println!("  • Windows handles include more than just files");
            println!(
                "  • Use 'Get-Process -Id {} | Select HandleCount' in PowerShell",
                std::process::id()
            );
            println!("  • Consider Process Explorer for detailed handle info");
        }
        _ => {
            println!("  • Platform-specific monitoring not fully supported");
            println!("  • Generic fallback methods used");
        }
    }

    println!();

    // Monitoring example
    println!("{}", "Live Monitoring Example:".bold());
    println!("  Testing FD monitoring during a simple operation...");

    match fd_monitor::FdMonitor::new() {
        Ok(mut monitor) => {
            println!("  Initial count: {}", monitor.sample().unwrap_or(0));

            // Simulate some file operations
            let temp_dir = std::env::temp_dir();
            let test_file = temp_dir.join("obsctl_fd_test.tmp");

            // Create and immediately close a file
            if std::fs::write(&test_file, "test").is_ok() {
                let _ = monitor.sample();
                let _ = std::fs::read(&test_file);
                let _ = monitor.sample();
                let _ = std::fs::remove_file(&test_file);
            }

            let final_count = monitor.sample().unwrap_or(0);
            println!("  Final count: {final_count}");
            println!("  {}", monitor.report().cyan());
        }
        Err(e) => {
            println!("  Monitor creation failed: {e}");
        }
    }

    println!();
    println!("{}", "💡 Use this information to:".bold());
    println!("  • Monitor resource usage during large operations");
    println!("  • Debug file descriptor leaks");
    println!("  • Optimize concurrent operation limits");
    println!("  • Ensure system stability under load");

    Ok(())
}
