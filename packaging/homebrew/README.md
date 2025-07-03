# obsctl Homebrew Formula

This directory contains the Homebrew formula for installing obsctl on macOS and Linux.

## Installation Methods

### Method 1: Direct Formula Installation (Recommended for Testing)

```bash
# Install directly from the formula file
brew install --build-from-source packaging/homebrew/obsctl.rb

# Or install from a local tap
brew tap-new your-org/obsctl
brew extract --version=0.1.0 obsctl your-org/obsctl
brew install your-org/obsctl/obsctl
```

### Method 2: Official Tap (Production)

Once the formula is submitted to Homebrew core or your own tap:

```bash
# From official tap (replace with actual tap name)
brew tap your-org/homebrew-obsctl
brew install obsctl

# Or from Homebrew core (if accepted)
brew install obsctl
```

## What Gets Installed

The formula installs:

- **Binary**: `/opt/homebrew/bin/obsctl` (Apple Silicon) or `/usr/local/bin/obsctl` (Intel)
- **Man Page**: `man obsctl` available system-wide
- **Bash Completion**: Tab completion for obsctl commands
- **Dashboard Files**: `/opt/homebrew/share/obsctl/dashboards/*.json`
- **Config Template**: `/opt/homebrew/etc/obsctl/config`

## Post-Installation

After installation, you'll see helpful information about:

- Dashboard management commands
- Quick start guide
- File locations
- Configuration options

## Testing the Formula

To test the formula locally:

```bash
# Audit the formula
brew audit --strict packaging/homebrew/obsctl.rb

# Test installation (dry run)
brew install --build-from-source --verbose packaging/homebrew/obsctl.rb

# Run formula tests
brew test obsctl
```

## Formula Features

### Cross-Platform Support
- macOS (Intel and Apple Silicon)
- Linux (via Homebrew on Linux)

### Complete Installation
- Builds from source using Rust/Cargo
- Installs all components (binary, docs, completions, dashboards)
- Creates necessary directories
- Provides helpful post-install messaging

### Quality Assurance
- Comprehensive test suite
- Validates all installed components
- Tests core functionality
- Ensures dashboard files are present

## Updating the Formula

When releasing a new version:

1. Update the `url` and `sha256` in the formula
2. Update the version number
3. Test the formula thoroughly
4. Submit to appropriate tap or Homebrew core

## Dashboard Integration

The formula automatically installs Grafana dashboard files to:
- **Location**: `$(brew --prefix)/share/obsctl/dashboards/`
- **Symlink**: `$(brew --prefix)/share/obsctl/grafana-dashboards/` (for convenience)

Users can then use:
```bash
obsctl config dashboard install  # Install to localhost Grafana
obsctl config dashboard list     # List available dashboards
```

## Development

For formula development:

```bash
# Create a new tap for testing
brew tap-new your-org/obsctl

# Add the formula to your tap
cp packaging/homebrew/obsctl.rb $(brew --repository your-org/obsctl)/Formula/

# Install from your tap
brew install your-org/obsctl/obsctl
``` 