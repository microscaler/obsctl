pub mod args;
pub mod commands;
pub mod config;
pub mod filtering;
pub mod logging;
pub mod otel;
pub mod upload;
pub mod utils;

pub use args::Args;
pub use config::Config;

/// The version of obsctl, automatically pulled from Cargo.toml
/// This handles both release versions (e.g., "1.2.3") and dev versions (e.g., "1.2.3-dev", "1.2.3-alpha.1")
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Get a clean version string suitable for service identification
/// Strips any pre-release suffixes for consistency in telemetry
pub fn get_service_version() -> String {
    let version = VERSION;

    // For development builds, we might have versions like "0.1.0-dev" or "0.1.0-alpha.1"
    // For service identification, we want to use just the base version
    if let Some(dash_pos) = version.find('-') {
        // Strip everything after the first dash (pre-release identifiers)
        version[..dash_pos].to_string()
    } else {
        // No pre-release identifier, use as-is
        version.to_string()
    }
}

/// Get the full version string including any pre-release identifiers
/// Use this for user-facing version displays
pub fn get_full_version() -> &'static str {
    VERSION
}
