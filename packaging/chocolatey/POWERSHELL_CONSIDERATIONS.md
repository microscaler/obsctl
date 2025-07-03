# PowerShell Considerations for Chocolatey Packaging

## Overview

Chocolatey packages rely heavily on PowerShell scripts for installation, uninstallation, and maintenance operations. This document outlines the implications for our obsctl cross-platform build system and how we address them.

## Key PowerShell Implications

### 1. Cross-Platform Development Challenge
- **Issue**: Our build system runs on Unix/macOS but generates PowerShell scripts
- **Solution**: Use template files with placeholder substitution instead of inline generation
- **Benefits**: Version-controlled scripts, proper syntax validation, easier maintenance

### 2. Template-Based Approach
Instead of generating PowerShell scripts inline, we use templates:

```bash
# Template processing
sed -e "s/{{VERSION}}/$VERSION/g" \
    -e "s/{{CHECKSUM}}/$checksum/g" \
    template.ps1 > output.ps1
```

### 3. PowerShell Script Requirements
- **Error handling**: `$ErrorActionPreference = 'Stop'`
- **Path management**: Use PowerShell path functions
- **Chocolatey helpers**: Leverage built-in functions
- **User feedback**: Colored output for better UX

## Template Files

### chocolateyinstall.ps1.template
- Downloads and installs obsctl from GitHub releases
- Adds to system PATH automatically
- Provides user feedback and quick start guide
- Includes installation verification

### chocolateyuninstall.ps1.template  
- Removes obsctl from system PATH
- Provides cleanup guidance for config files
- Clean uninstallation with user feedback

### obsctl.nuspec.template
- Package metadata and dependencies
- Rich description with features and usage
- Proper Chocolatey community standards

## Security & Best Practices

### Download Verification
- SHA256 checksums for all downloads
- Downloads from official GitHub releases only
- No embedded binaries in package

### Minimal Permissions
- Standard user permissions for most operations
- Machine-level PATH for system-wide access
- Clean uninstallation process

## Testing Workflow

```powershell
# Local testing
choco pack obsctl.nuspec --version 0.1.0-test
choco install obsctl -s . -f --version 0.1.0-test
obsctl --version
choco uninstall obsctl
```

## Cross-Platform Considerations

1. **Line Endings**: PowerShell handles both LF and CRLF
2. **Character Encoding**: UTF-8 without BOM recommended
3. **Path Separators**: Use PowerShell path functions
4. **Environment Variables**: PowerShell syntax in templates

This approach ensures reliable, maintainable Chocolatey packages while supporting our cross-platform build system.
