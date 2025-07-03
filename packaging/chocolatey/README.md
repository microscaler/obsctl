# Chocolatey Packaging for obsctl

This directory contains the Chocolatey packaging configuration for obsctl on Windows.

## Overview

Chocolatey is the primary package manager for Windows, making software installation as simple as `choco install obsctl`. Our Chocolatey package provides:

- **Automatic installation** from GitHub releases
- **PATH management** - obsctl is automatically added to system PATH
- **Verification** - SHA256 checksums ensure package integrity
- **Clean uninstallation** - Complete removal including PATH cleanup

## Package Structure

```
chocolatey/
├── obsctl.nuspec              # Package specification
├── tools/
│   ├── chocolateyinstall.ps1  # Installation script
│   ├── chocolateyuninstall.ps1# Uninstallation script
│   └── windows-x64/           # Binary files (populated during build)
└── legal/
    └── VERIFICATION.txt        # Package verification information
```

## Installation

### For Users
```powershell
# Install obsctl via Chocolatey
choco install obsctl

# Verify installation
obsctl --version

# Configure obsctl
obsctl config configure
obsctl config dashboard install
```

### For Administrators
```powershell
# Install for all users
choco install obsctl -y

# Install specific version
choco install obsctl --version 0.1.0
```

## Package Maintenance

### Building the Package

The Chocolatey package is automatically created by the main build system:

```bash
# Build all packages including Chocolatey
./packaging/build-releases.sh

# The .nupkg file will be created in the packages directory
```

### Manual Package Creation

If you need to create the package manually:

```powershell
# Navigate to the chocolatey directory
cd packaging/chocolatey

# Create the .nupkg file
choco pack obsctl.nuspec

# Test the package locally
choco install obsctl -s . -f
```

### Publishing to Chocolatey Community Repository

1. **Create account** at [chocolatey.org](https://chocolatey.org)
2. **Get API key** from your account settings
3. **Submit package**:
   ```powershell
   choco apikey -k YOUR_API_KEY -s https://push.chocolatey.org/
   choco push obsctl.0.1.0.nupkg -s https://push.chocolatey.org/
   ```

## Package Features

### Automatic Updates
The package supports automatic updates when new versions are released:

```powershell
# Check for updates
choco outdated

# Update obsctl
choco upgrade obsctl
```

### Verification
Each package includes:
- **SHA256 checksums** for download verification
- **Source verification** - links to official GitHub releases
- **Digital signatures** (when available)

### Dependencies
The package has minimal dependencies:
- Windows 10/11 or Windows Server 2016+
- PowerShell 5.0+
- .NET Framework 4.7.2+ (typically pre-installed)

## Troubleshooting

### Common Issues

**Installation fails with "Access Denied"**
```powershell
# Run as Administrator
choco install obsctl --force
```

**Package not found**
```powershell
# Refresh package list
choco source list
choco search obsctl
```

**PATH not updated**
```powershell
# Refresh environment variables
refreshenv
# or restart your terminal
```

### Manual PATH Management

If automatic PATH management fails:

```powershell
# Add to user PATH
$env:PATH += ";C:\ProgramData\chocolatey\lib\obsctl\tools\windows-x64"

# Add to system PATH (requires admin)
[Environment]::SetEnvironmentVariable("PATH", $env:PATH + ";C:\ProgramData\chocolatey\lib\obsctl\tools\windows-x64", "Machine")
```

## Development

### Testing Changes

1. **Create test package**:
   ```powershell
   choco pack obsctl.nuspec --version 0.1.0-test
   ```

2. **Install locally**:
   ```powershell
   choco install obsctl -s . -f --version 0.1.0-test
   ```

3. **Test functionality**:
   ```powershell
   obsctl --version
   obsctl config --help
   ```

4. **Uninstall test**:
   ```powershell
   choco uninstall obsctl
   ```

### Package Validation

Before publishing, validate the package:

```powershell
# Test installation
choco install obsctl -s . -f

# Test basic functionality
obsctl --version
obsctl config --help

# Test uninstallation
choco uninstall obsctl

# Verify cleanup
where obsctl  # Should return nothing
```

## Security

### Package Security Features

- **SHA256 verification** - All downloads are verified
- **HTTPS downloads** - Secure download from GitHub releases
- **No embedded binaries** - Downloads from official sources only
- **Minimal permissions** - Only requires standard user permissions

### Security Best Practices

1. **Always verify checksums** before publishing
2. **Use official release URLs** only
3. **Test on clean systems** before publishing
4. **Monitor for security advisories**

## Support

For Chocolatey-specific issues:
- [Chocolatey Documentation](https://docs.chocolatey.org/)
- [Chocolatey Community](https://community.chocolatey.org/)
- [Package Guidelines](https://docs.chocolatey.org/en-us/create/create-packages)

For obsctl issues:
- [GitHub Issues](https://github.com/your-org/obsctl/issues)
- [Documentation](https://github.com/your-org/obsctl/blob/master/README.md) 