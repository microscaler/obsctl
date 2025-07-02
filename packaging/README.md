# obsctl Packaging System

This directory contains the complete packaging system for obsctl, supporting multiple platforms and package formats.

## 🎯 Overview

The obsctl packaging system provides:

- **Multi-platform binary builds** (Linux x64/ARM64, macOS Intel/ARM64, Windows x64)
- **Package formats** (Debian .deb, RPM .rpm, Homebrew formula)
- **Dashboard integration** (Grafana dashboards included in all packages)
- **Automated workflows** (Complete release automation)
- **Cross-compilation support** (Build all platforms from any host)

## 📁 Directory Structure

```
packaging/
├── README.md                    # This file
├── release-workflow.sh          # 🚀 Master release workflow
├── build-releases.sh            # Multi-platform build script
├── debian/                      # Debian packaging
│   ├── control                  # Package metadata
│   ├── install                  # File installation paths
│   ├── postinst                 # Post-installation script
│   ├── prerm                    # Pre-removal script
│   └── config                   # Configuration template
├── rpm/                         # RPM packaging
│   └── obsctl.spec              # RPM spec file
├── homebrew/                    # Homebrew formula
│   ├── obsctl.rb                # Formula file
│   ├── README.md                # Homebrew-specific docs
│   ├── test-formula.sh          # Formula testing script
│   ├── release-formula.sh       # Formula release helper
│   └── update-formula-shas.sh   # SHA256 updater
├── dashboards/                  # Grafana dashboards
│   └── obsctl-unified.json      # Main dashboard
├── obsctl.1                     # Man page
└── obsctl.bash-completion       # Bash completion
```

## 🚀 Quick Start

### Complete Release Build

```bash
# Run the complete release workflow
./packaging/release-workflow.sh
```

This will:
1. ✅ Check prerequisites
2. 🧹 Clean previous builds  
3. 🧪 Run tests
4. 🔨 Build for all platforms
5. 📦 Create packages (deb, rpm)
6. 🍺 Update Homebrew formula
7. 📋 Generate release notes

### Individual Steps

```bash
# Build for all platforms
./packaging/build-releases.sh

# Test Homebrew formula
./packaging/homebrew/test-formula.sh

# Update Homebrew SHA256 values
./packaging/homebrew/update-formula-shas.sh
```

## 🛠️ Platform Support

### Supported Targets

| Platform | Architecture | Binary | Package | Status |
|----------|-------------|---------|---------|---------|
| **Linux** | x86_64 | ✅ | .deb, .rpm | Full |
| **Linux** | ARM64 | ✅ | .deb, .rpm | Full |
| **macOS** | Intel | ✅ | Homebrew | Full |
| **macOS** | Apple Silicon | ✅ | Homebrew | Full |
| **Windows** | x86_64 | ✅ | Archive | Basic |

### Package Locations

**Homebrew (macOS/Linux)**:
- Binary: `/opt/homebrew/bin/obsctl` or `/usr/local/bin/obsctl`
- Dashboards: `/opt/homebrew/share/obsctl/dashboards/`
- Man page: `man obsctl`
- Completion: Auto-loaded

**Debian (.deb)**:
- Binary: `/usr/bin/obsctl`
- Dashboards: `/usr/share/obsctl/dashboards/`
- Man page: `/usr/share/man/man1/obsctl.1`
- Completion: `/usr/share/bash-completion/completions/obsctl`

**RPM (.rpm)**:
- Binary: `/usr/bin/obsctl`
- Dashboards: `/usr/share/obsctl/dashboards/`
- Man page: `/usr/share/man/man1/obsctl.1`
- Completion: `/usr/share/bash-completion/completions/obsctl`

## 📊 Dashboard Integration

All packages include Grafana dashboard files:

### Dashboard Files
- `obsctl-unified.json` - Main observability dashboard
- Includes metrics for all S3 operations
- Auto-refresh and time range controls
- Compatible with Grafana 8.0+

### Dashboard Management
```bash
# Install dashboards to Grafana
obsctl config dashboard install

# Install to remote Grafana
obsctl config dashboard install \
  --url http://grafana.company.com:3000 \
  --username admin \
  --password secret

# List installed dashboards
obsctl config dashboard list

# Show dashboard info
obsctl config dashboard info
```

### Security Features
- Only manages obsctl-specific dashboards
- Restricted search scope (obsctl keyword only)
- Confirmation required for destructive operations
- No general Grafana administration capabilities

## 🔧 Build Requirements

### Essential Tools
- **Rust** (1.70+) - `cargo`, `rustc`
- **Git** - Version control
- **Standard tools** - `tar`, `gzip`, `shasum`

### Optional Tools
- **cross** - `cargo install cross` (easier cross-compilation)
- **dpkg-deb** - Debian package creation
- **rpmbuild** - RPM package creation  
- **Homebrew** - Formula testing

### Cross-Compilation Setup

```bash
# Install cross for easier cross-compilation
cargo install cross

# Or install targets manually
rustup target add x86_64-unknown-linux-gnu
rustup target add aarch64-unknown-linux-gnu
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin
rustup target add x86_64-pc-windows-gnu
```

## 📦 Package Creation

### Debian Packages

```bash
# Created automatically by build-releases.sh
# Manual creation:
dpkg-deb --build target/packages/debian-linux-x64 obsctl_0.1.0_amd64.deb
```

**Features**:
- Proper dependency management
- Post-install scripts
- Dashboard file installation
- systemd integration

### RPM Packages

```bash
# Created automatically by build-releases.sh  
# Manual creation:
rpmbuild --define "_topdir $(pwd)/target/packages/rpm-linux-x64" \
         -ba packaging/rpm/obsctl.spec
```

**Features**:
- Spec file with proper metadata
- File permissions and ownership
- Dashboard integration
- systemd compatibility

### Homebrew Formula

The formula supports multiple architectures and includes:

```ruby
# Multi-architecture support
on_macos do
  on_intel do
    url "https://github.com/your-org/obsctl/releases/download/v0.1.0/obsctl-0.1.0-macos-intel.tar.gz"
    sha256 "INTEL_SHA256"
  end
  on_arm do
    url "https://github.com/your-org/obsctl/releases/download/v0.1.0/obsctl-0.1.0-macos-arm64.tar.gz"
    sha256 "ARM64_SHA256"
  end
end
```

**Features**:
- Pre-built binaries for faster installation
- Fallback to source compilation
- Complete file installation
- Post-install messaging
- Comprehensive tests

## 🔄 Release Workflow

### 1. Preparation

```bash
# Ensure clean state
git status
git pull origin main

# Update version in Cargo.toml if needed
vim Cargo.toml
```

### 2. Build and Package

```bash
# Run complete workflow
./packaging/release-workflow.sh

# Or run individual steps
./packaging/build-releases.sh
./packaging/homebrew/update-formula-shas.sh
```

### 3. Testing

```bash
# Test Homebrew formula
./packaging/homebrew/test-formula.sh

# Test packages on target systems
# (Upload to test VMs/containers)
```

### 4. Release

```bash
# Create GitHub release
gh release create v0.1.0 \
  target/releases/*.tar.gz \
  target/releases/*.zip \
  target/packages/*.deb \
  target/packages/*.rpm \
  --title "obsctl v0.1.0" \
  --notes-file target/packages/RELEASE_NOTES_v0.1.0.md
```

### 5. Distribution

```bash
# Submit Homebrew formula
# (Create PR to homebrew-core or your tap)

# Submit to package repositories
# (Upload .deb to apt repository)
# (Upload .rpm to yum repository)
```

## 🧪 Testing

### Local Testing

```bash
# Test binary functionality
target/release/obsctl --version
target/release/obsctl config --help

# Test package installation (Docker)
docker run -it ubuntu:22.04
# Copy and install .deb package

docker run -it fedora:38  
# Copy and install .rpm package
```

### Homebrew Testing

```bash
# Test formula syntax
brew audit --strict packaging/homebrew/obsctl.rb

# Test installation
brew install --build-from-source packaging/homebrew/obsctl.rb

# Test functionality
obsctl --version
obsctl config dashboard info
```

### Dashboard Testing

```bash
# Start local Grafana
docker run -p 3000:3000 grafana/grafana

# Install dashboards
obsctl config dashboard install

# Verify in Grafana UI
open http://localhost:3000
```

## 🔍 Troubleshooting

### Build Issues

**Cross-compilation fails**:
```bash
# Install cross
cargo install cross

# Or use Docker-based compilation
cross build --release --target x86_64-unknown-linux-gnu
```

**Missing dependencies**:
```bash
# Install Rust targets
rustup target add aarch64-unknown-linux-gnu

# Install system dependencies
sudo apt-get install gcc-aarch64-linux-gnu  # Ubuntu
brew install FiloSottile/musl-cross/musl-cross  # macOS
```

### Package Issues

**Debian package creation fails**:
```bash
# Install dpkg tools
sudo apt-get install dpkg-dev

# Check package structure
dpkg-deb --contents obsctl_0.1.0_amd64.deb
```

**RPM package creation fails**:
```bash
# Install RPM tools
sudo dnf install rpm-build  # Fedora
sudo apt-get install rpm    # Ubuntu

# Check spec file
rpmbuild --parse packaging/rpm/obsctl.spec
```

### Homebrew Issues

**Formula validation fails**:
```bash
# Check Ruby syntax
ruby -c packaging/homebrew/obsctl.rb

# Run Homebrew audit
brew audit --strict packaging/homebrew/obsctl.rb
```

**Installation fails**:
```bash
# Check file permissions
ls -la target/releases/obsctl-*-macos-*.tar.gz

# Verify archive contents
tar -tzf target/releases/obsctl-0.1.0-macos-intel.tar.gz
```

## 📚 References

- [Homebrew Formula Cookbook](https://docs.brew.sh/Formula-Cookbook)
- [Debian Packaging Guide](https://www.debian.org/doc/manuals/packaging-tutorial/packaging-tutorial.en.html)
- [RPM Packaging Guide](https://rpm-packaging-guide.github.io/)
- [Rust Cross-compilation](https://rust-lang.github.io/rustup/cross-compilation.html)

## 🤝 Contributing

When adding new packaging features:

1. Update this README
2. Add tests to relevant test scripts
3. Update the release workflow
4. Test on multiple platforms
5. Document any new dependencies

## 📄 License

This packaging system is part of obsctl and follows the same license terms. 