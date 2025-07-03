#!/bin/bash
set -e

# Multi-platform build script for obsctl
# Builds binaries for multiple architectures and creates platform-specific packages

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_DIR="$PROJECT_ROOT/target/releases"
PACKAGE_DIR="$PROJECT_ROOT/target/packages"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Target platforms for obsctl
TARGETS=(
    "x86_64-unknown-linux-gnu"      # Linux x64
    "aarch64-unknown-linux-gnu"     # Linux ARM64
    "armv7-unknown-linux-gnueabihf" # Linux ARM7 (Raspberry Pi)
    "x86_64-apple-darwin"           # macOS Intel
    "aarch64-apple-darwin"          # macOS Apple Silicon
    "x86_64-pc-windows-gnu"         # Windows x64
)

# Platform-specific information
declare -A PLATFORM_NAMES=(
    ["x86_64-unknown-linux-gnu"]="linux-x64"
    ["aarch64-unknown-linux-gnu"]="linux-arm64"
    ["armv7-unknown-linux-gnueabihf"]="linux-armv7"
    ["x86_64-apple-darwin"]="macos-intel"
    ["aarch64-apple-darwin"]="macos-arm64"
    ["x86_64-pc-windows-gnu"]="windows-x64"
)

declare -A BINARY_NAMES=(
    ["x86_64-unknown-linux-gnu"]="obsctl"
    ["aarch64-unknown-linux-gnu"]="obsctl"
    ["armv7-unknown-linux-gnueabihf"]="obsctl"
    ["x86_64-apple-darwin"]="obsctl"
    ["aarch64-apple-darwin"]="obsctl"
    ["x86_64-pc-windows-gnu"]="obsctl.exe"
)

# Function to print colored output
print_step() {
    echo -e "${BLUE}🔧 $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Get version from Cargo.toml
get_version() {
    grep '^version = ' "$PROJECT_ROOT/Cargo.toml" | sed 's/version = "\(.*\)"/\1/'
}

# Check if cross-compilation tools are available
check_cross_tools() {
    print_step "Checking cross-compilation tools..."

    if ! command -v cargo >/dev/null 2>&1; then
        print_error "Cargo not found. Please install Rust."
        exit 1
    fi

    # Check if cross is installed for easier cross-compilation
    if command -v cross >/dev/null 2>&1; then
        echo "✅ cross tool available for cross-compilation"
        USE_CROSS=true
    else
        echo "ℹ️  cross tool not found. Install with: cargo install cross"
        echo "ℹ️  Will use cargo with manual target installation"
        USE_CROSS=false
    fi
}

# Install Rust targets
install_targets() {
    print_step "Installing Rust targets..."

    for target in "${TARGETS[@]}"; do
        echo "Installing target: $target"
        rustup target add "$target" || {
            print_warning "Failed to install target $target (may not be available on this host)"
        }
    done
}

# Build for a specific target
build_target() {
    local target="$1"
    local platform_name="${PLATFORM_NAMES[$target]}"
    local binary_name="${BINARY_NAMES[$target]}"

    print_step "Building for $target ($platform_name)..."

    cd "$PROJECT_ROOT"

    # Choose build method
    if [[ "$USE_CROSS" == "true" ]]; then
        # Use cross for easier cross-compilation
        cross build --release --target "$target" || {
            print_warning "Cross-compilation failed for $target, skipping..."
            return 1
        }
    else
        # Use regular cargo
        cargo build --release --target "$target" || {
            print_warning "Compilation failed for $target, skipping..."
            return 1
        }
    fi

    # Check if binary was created
    local binary_path="$PROJECT_ROOT/target/$target/release/$binary_name"
    if [[ ! -f "$binary_path" ]]; then
        print_error "Binary not found at $binary_path"
        return 1
    fi

    # Create release directory
    local release_dir="$BUILD_DIR/$platform_name"
    mkdir -p "$release_dir"

    # Copy binary
    cp "$binary_path" "$release_dir/"

    # Copy additional files
    cp "$PROJECT_ROOT/README.md" "$release_dir/"
    cp "$PROJECT_ROOT/packaging/obsctl.1" "$release_dir/"
    cp "$PROJECT_ROOT/packaging/obsctl.bash-completion" "$release_dir/"

    # Copy dashboard files
    mkdir -p "$release_dir/dashboards"
    cp "$PROJECT_ROOT/packaging/dashboards"/*.json "$release_dir/dashboards/"

    # Create platform-specific archive
    local archive_name="obsctl-$VERSION-$platform_name"

    cd "$BUILD_DIR"

    if [[ "$target" == *"windows"* ]]; then
        # Create ZIP for Windows
        zip -r "$archive_name.zip" "$platform_name/"
        print_success "Created $archive_name.zip"
    else
        # Create tar.gz for Unix-like systems
        tar -czf "$archive_name.tar.gz" "$platform_name/"
        print_success "Created $archive_name.tar.gz"
    fi

    cd "$PROJECT_ROOT"

    return 0
}

# Create macOS Universal Binary (fat binary) from Intel and ARM64 builds
create_macos_universal_binary() {
    print_step "Creating macOS Universal Binary..."

    local intel_dir="$BUILD_DIR/macos-intel"
    local arm64_dir="$BUILD_DIR/macos-arm64"
    local universal_dir="$BUILD_DIR/macos-universal"

    # Check if both macOS builds exist
    if [[ ! -d "$intel_dir" ]] || [[ ! -d "$arm64_dir" ]]; then
        print_warning "Both macOS Intel and ARM64 builds required for Universal Binary"
        return 1
    fi

    if [[ ! -f "$intel_dir/obsctl" ]] || [[ ! -f "$arm64_dir/obsctl" ]]; then
        print_warning "obsctl binaries not found in macOS build directories"
        return 1
    fi

    # Check if lipo is available (should be on macOS)
    if ! command -v lipo >/dev/null 2>&1; then
        print_warning "lipo command not available - Universal Binary creation requires macOS"
        return 1
    fi

    print_step "Combining Intel and ARM64 binaries with lipo..."

    # Create universal directory
    mkdir -p "$universal_dir"

    # Copy all files from Intel build (they should be identical except for the binary)
    cp -r "$intel_dir"/* "$universal_dir/"

    # Create universal binary using lipo
    lipo -create \
        "$intel_dir/obsctl" \
        "$arm64_dir/obsctl" \
        -output "$universal_dir/obsctl"

    # Verify the universal binary
    if lipo -info "$universal_dir/obsctl" | grep -q "x86_64 arm64"; then
        print_success "Universal Binary created successfully"
        lipo -info "$universal_dir/obsctl"
    else
        print_error "Universal Binary creation failed"
        return 1
    fi

    # Create universal archive
    local archive_name="obsctl-$VERSION-macos-universal"

    cd "$BUILD_DIR"
    tar -czf "$archive_name.tar.gz" "macos-universal/"
    print_success "Created $archive_name.tar.gz"

    cd "$PROJECT_ROOT"
    return 0
}

# Create Debian packages for Linux targets
create_debian_packages() {
    print_step "Creating Debian packages..."

    local linux_targets=("x86_64-unknown-linux-gnu" "aarch64-unknown-linux-gnu" "armv7-unknown-linux-gnueabihf")

    for target in "${linux_targets[@]}"; do
        local platform_name="${PLATFORM_NAMES[$target]}"
        local release_dir="$BUILD_DIR/$platform_name"

        if [[ ! -d "$release_dir" ]]; then
            print_warning "Release directory not found for $platform_name, skipping Debian package"
            continue
        fi

        print_step "Creating Debian package for $platform_name..."

        # Architecture mapping for Debian
        local deb_arch
        case "$target" in
            "x86_64-unknown-linux-gnu") deb_arch="amd64" ;;
            "aarch64-unknown-linux-gnu") deb_arch="arm64" ;;
            "armv7-unknown-linux-gnueabihf") deb_arch="armhf" ;;
            *)
                print_warning "Unknown architecture for Debian: $target"
                continue
                ;;
        esac

        # Create package directory structure
        local pkg_dir="$PACKAGE_DIR/debian-$platform_name"
        mkdir -p "$pkg_dir"/{DEBIAN,usr/bin,usr/share/man/man1,usr/share/bash-completion/completions,usr/share/obsctl/dashboards,etc/obsctl}

        # Copy files
        cp "$release_dir/obsctl" "$pkg_dir/usr/bin/"
        cp "$release_dir/obsctl.1" "$pkg_dir/usr/share/man/man1/"
        cp "$release_dir/obsctl.bash-completion" "$pkg_dir/usr/share/bash-completion/completions/obsctl"
        cp "$release_dir/dashboards"/*.json "$pkg_dir/usr/share/obsctl/dashboards/"
        cp "$PROJECT_ROOT/packaging/debian/config" "$pkg_dir/etc/obsctl/"

        # Create control file
        cat > "$pkg_dir/DEBIAN/control" << EOF
Package: obsctl
Version: $VERSION
Section: utils
Priority: optional
Architecture: $deb_arch
Maintainer: obsctl Team <team@example.com>
Description: S3-compatible CLI tool with OpenTelemetry observability
 obsctl is a high-performance S3-compatible CLI tool with built-in OpenTelemetry
 observability and Grafana dashboard support. It provides comprehensive metrics,
 tracing, and monitoring capabilities for S3 operations.
Depends: libc6
EOF

        # Copy postinst script
        cp "$PROJECT_ROOT/packaging/debian/postinst" "$pkg_dir/DEBIAN/"
        chmod 755 "$pkg_dir/DEBIAN/postinst"

        # Set permissions
        chmod 755 "$pkg_dir/usr/bin/obsctl"
        chmod 644 "$pkg_dir/usr/share/man/man1/obsctl.1"
        chmod 644 "$pkg_dir/usr/share/bash-completion/completions/obsctl"
        chmod 644 "$pkg_dir/usr/share/obsctl/dashboards"/*.json
        chmod 644 "$pkg_dir/etc/obsctl/config"

        # Build package
        local deb_file="$PACKAGE_DIR/obsctl_${VERSION}_${deb_arch}.deb"
        if command -v dpkg-deb >/dev/null 2>&1; then
            dpkg-deb --build "$pkg_dir" "$deb_file"
            print_success "Created $deb_file"
        else
            print_warning "dpkg-deb not available, skipping .deb creation"
        fi
    done
}

# Create RPM packages for Linux targets
create_rpm_packages() {
    print_step "Creating RPM packages..."

    if ! command -v rpmbuild >/dev/null 2>&1; then
        print_warning "rpmbuild not available, skipping RPM creation"
        return
    fi

    local linux_targets=("x86_64-unknown-linux-gnu" "aarch64-unknown-linux-gnu" "armv7-unknown-linux-gnueabihf")

    for target in "${linux_targets[@]}"; do
        local platform_name="${PLATFORM_NAMES[$target]}"
        local release_dir="$BUILD_DIR/$platform_name"

        if [[ ! -d "$release_dir" ]]; then
            print_warning "Release directory not found for $platform_name, skipping RPM package"
            continue
        fi

        # Architecture mapping for RPM
        local rpm_arch
        case "$target" in
            "x86_64-unknown-linux-gnu") rpm_arch="x86_64" ;;
            "aarch64-unknown-linux-gnu") rpm_arch="aarch64" ;;
            "armv7-unknown-linux-gnueabihf") rpm_arch="armhf" ;;
            *)
                print_warning "Unknown architecture for RPM: $target"
                continue
                ;;
        esac

        print_step "Creating RPM package for $platform_name ($rpm_arch)..."

        # Create RPM build structure
        local rpm_dir="$PACKAGE_DIR/rpm-$platform_name"
        mkdir -p "$rpm_dir"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}

        # Create source tarball
        local source_dir="$rpm_dir/SOURCES/obsctl-$VERSION"
        mkdir -p "$source_dir"
        cp -r "$release_dir"/* "$source_dir/"

        cd "$rpm_dir/SOURCES"
        tar -czf "obsctl-$VERSION.tar.gz" "obsctl-$VERSION/"
        cd "$PROJECT_ROOT"

        # Create spec file with correct architecture
        sed "s/BuildArch: noarch/BuildArch: $rpm_arch/" "$PROJECT_ROOT/packaging/rpm/obsctl.spec" > "$rpm_dir/SPECS/obsctl.spec"
        sed -i "s/Version:.*/Version: $VERSION/" "$rpm_dir/SPECS/obsctl.spec"

        # Build RPM
        rpmbuild --define "_topdir $rpm_dir" -ba "$rpm_dir/SPECS/obsctl.spec" || {
            print_warning "RPM build failed for $platform_name"
            continue
        }

        # Copy RPM to package directory
        find "$rpm_dir/RPMS" -name "*.rpm" -exec cp {} "$PACKAGE_DIR/" \;
        print_success "Created RPM package for $platform_name"
    done
}

# Update Homebrew formula with checksums
update_homebrew_formula() {
    print_step "Updating Homebrew formula with release information..."

    local formula_file="$PROJECT_ROOT/packaging/homebrew/obsctl.rb"

    if [[ ! -f "$formula_file" ]]; then
        print_warning "Homebrew formula not found, skipping update"
        return
    fi

    # Calculate SHA256 for macOS archives (Homebrew typically uses macOS builds)
    local macos_intel_archive="$BUILD_DIR/obsctl-$VERSION-macos-intel.tar.gz"
    local macos_arm_archive="$BUILD_DIR/obsctl-$VERSION-macos-arm64.tar.gz"

    if [[ -f "$macos_intel_archive" ]]; then
        local sha256=$(shasum -a 256 "$macos_intel_archive" | cut -d' ' -f1)
        print_success "macOS Intel SHA256: $sha256"
        echo "# Update formula with: sha256 \"$sha256\""
    fi
}

# Generate release summary
generate_summary() {
    print_step "Generating release summary..."

    local summary_file="$PACKAGE_DIR/RELEASE_SUMMARY.md"

    cat > "$summary_file" << EOF
# obsctl v$VERSION Release Summary

Generated on: $(date)

## Binary Archives

EOF

    # List all created archives
    for file in "$BUILD_DIR"/*.{tar.gz,zip}; do
        if [[ -f "$file" ]]; then
            local filename=$(basename "$file")
            local size=$(du -h "$file" | cut -f1)
            local sha256=$(shasum -a 256 "$file" | cut -d' ' -f1)

            echo "### $filename" >> "$summary_file"
            echo "- **Size**: $size" >> "$summary_file"
            echo "- **SHA256**: \`$sha256\`" >> "$summary_file"
            echo "" >> "$summary_file"
        fi
    done

    cat >> "$summary_file" << EOF

## Platform Support

### Linux
- **x64** (Intel/AMD 64-bit) - Most servers and desktops
- **ARM64** (64-bit ARM) - Modern ARM servers, AWS Graviton
- **ARMv7** (32-bit ARM) - Raspberry Pi, embedded devices

### macOS
- **Universal Binary** - Single binary supports both Intel and Apple Silicon
- **Intel** (x86_64) - Traditional Mac hardware
- **Apple Silicon** (ARM64) - M1, M2, M3 Macs

### Windows
- **x64** (Intel/AMD 64-bit) - Standard Windows systems

## Package Files

EOF

    # List all created packages
    for file in "$PACKAGE_DIR"/*.{deb,rpm}; do
        if [[ -f "$file" ]]; then
            local filename=$(basename "$file")
            local size=$(du -h "$file" | cut -f1)

            echo "- **$filename** ($size)" >> "$summary_file"
        fi
    done

    cat >> "$summary_file" << EOF

## Installation Instructions

### Chocolatey (Windows)
\`\`\`powershell
choco install obsctl
\`\`\`

### Homebrew (macOS/Linux)
\`\`\`bash
brew install obsctl
\`\`\`

### Debian/Ubuntu
\`\`\`bash
sudo dpkg -i obsctl_${VERSION}_amd64.deb
# or
sudo dpkg -i obsctl_${VERSION}_arm64.deb
# or
sudo dpkg -i obsctl_${VERSION}_armhf.deb
\`\`\`

### RPM (RHEL/CentOS/Fedora)
\`\`\`bash
sudo rpm -i obsctl-${VERSION}-1.x86_64.rpm
# or
sudo rpm -i obsctl-${VERSION}-1.aarch64.rpm
# or
sudo rpm -i obsctl-${VERSION}-1.armhf.rpm
\`\`\`

### Manual Installation
1. Download the appropriate archive for your platform
2. Extract: \`tar -xzf obsctl-$VERSION-<platform>.tar.gz\`
3. Copy binary to PATH: \`sudo cp obsctl /usr/local/bin/\`
4. Install man page: \`sudo cp obsctl.1 /usr/local/share/man/man1/\`
5. Install bash completion: \`sudo cp obsctl.bash-completion /usr/local/share/bash-completion/completions/obsctl\`

## Dashboard Installation

After installing obsctl:
\`\`\`bash
obsctl config dashboard install  # Install to localhost Grafana
obsctl config dashboard list     # List available dashboards
\`\`\`

EOF

    print_success "Release summary created: $summary_file"
}

# Create Chocolatey packages for Windows
create_chocolatey_packages() {
    print_step "Creating Chocolatey packages..."

    local windows_target="x86_64-pc-windows-gnu"
    local platform_name="${PLATFORM_NAMES[$windows_target]}"
    local release_dir="$BUILD_DIR/$platform_name"

    if [[ ! -d "$release_dir" ]]; then
        print_warning "Windows release directory not found, skipping Chocolatey package"
        return
    fi

    if [[ ! -f "$release_dir/obsctl.exe" ]]; then
        print_warning "obsctl.exe not found in Windows build directory"
        return
    fi

    print_step "Creating Chocolatey package for Windows..."

    # Create chocolatey package directory structure
    local choco_dir="$PACKAGE_DIR/chocolatey"
    mkdir -p "$choco_dir"/{tools,legal}

    # Calculate checksum for the Windows archive first
    local windows_archive="$BUILD_DIR/obsctl-$VERSION-windows-x64.zip"
    local checksum=""
    if [[ -f "$windows_archive" ]]; then
        checksum=$(shasum -a 256 "$windows_archive" | cut -d' ' -f1)
        print_success "Windows archive checksum: $checksum"
    else
        print_warning "Windows archive not found for checksum calculation"
        checksum="PLACEHOLDER_CHECKSUM"
    fi

    # Create nuspec file from template
    if [[ -f "$PROJECT_ROOT/packaging/chocolatey/obsctl.nuspec.template" ]]; then
        sed -e "s/{{VERSION}}/$VERSION/g" \
            -e "s/{{YEAR}}/$(date +%Y)/g" \
            "$PROJECT_ROOT/packaging/chocolatey/obsctl.nuspec.template" > "$choco_dir/obsctl.nuspec"
    else
        print_warning "Chocolatey nuspec template not found, creating basic version"
        cat > "$choco_dir/obsctl.nuspec" << EOF
<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://schemas.microsoft.com/packaging/2015/06/nuspec.xsd">
  <metadata>
    <id>obsctl</id>
    <version>$VERSION</version>
    <title>obsctl</title>
    <authors>obsctl Team</authors>
    <description>S3-compatible CLI tool with OpenTelemetry observability</description>
  </metadata>
  <files>
    <file src="tools\**" target="tools" />
    <file src="legal\**" target="legal" />
  </files>
</package>
EOF
    fi

    # Create install script from template
    if [[ -f "$PROJECT_ROOT/packaging/chocolatey/chocolateyinstall.ps1.template" ]]; then
        sed -e "s/{{VERSION}}/$VERSION/g" \
            -e "s/{{CHECKSUM}}/$checksum/g" \
            "$PROJECT_ROOT/packaging/chocolatey/chocolateyinstall.ps1.template" > "$choco_dir/tools/chocolateyinstall.ps1"
    else
        print_warning "Chocolatey install template not found, creating basic version"
        cat > "$choco_dir/tools/chocolateyinstall.ps1" << EOF
\$ErrorActionPreference = 'Stop'
\$packageArgs = @{
  packageName   = 'obsctl'
  unzipLocation = "\$(Split-Path -parent \$MyInvocation.MyCommand.Definition)"
  url64bit      = 'https://github.com/your-org/obsctl/releases/download/v$VERSION/obsctl-$VERSION-windows-x64.zip'
  checksum64    = '$checksum'
  checksumType64= 'sha256'
}
Install-ChocolateyZipPackage @packageArgs
EOF
    fi

    # Create uninstall script from template
    if [[ -f "$PROJECT_ROOT/packaging/chocolatey/chocolateyuninstall.ps1.template" ]]; then
        cp "$PROJECT_ROOT/packaging/chocolatey/chocolateyuninstall.ps1.template" "$choco_dir/tools/chocolateyuninstall.ps1"
    else
        print_warning "Chocolatey uninstall template not found, creating basic version"
        cat > "$choco_dir/tools/chocolateyuninstall.ps1" << EOF
\$ErrorActionPreference = 'Stop'
\$toolsDir = "\$(Split-Path -parent \$MyInvocation.MyCommand.Definition)"
\$obsctlPath = Join-Path \$toolsDir "windows-x64"
Uninstall-ChocolateyPath \$obsctlPath -PathType 'Machine'
EOF
    fi

    # Create verification file
    cat > "$choco_dir/legal/VERIFICATION.txt" << EOF
VERIFICATION
Verification is intended to assist the Chocolatey moderators and community
in verifying that this package's contents are trustworthy.

Package can be verified like this:

1. Download the following:
   x64: https://github.com/your-org/obsctl/releases/download/v$VERSION/obsctl-$VERSION-windows-x64.zip

2. You can use one of the following methods to obtain the SHA256 checksum:
   - Use powershell function 'Get-FileHash'
   - Use Chocolatey utility 'checksum.exe'

   checksum64: $checksum

Using AU:
   Get-RemoteChecksum https://github.com/your-org/obsctl/releases/download/v$VERSION/obsctl-$VERSION-windows-x64.zip

The file is also available for download from the software developer's official website.
EOF

    # Copy files for packaging
    mkdir -p "$choco_dir/tools/windows-x64"
    cp "$release_dir"/* "$choco_dir/tools/windows-x64/"

    print_success "Chocolatey package files created with checksum: $checksum"

    # Create the chocolatey package if choco is available
    if command -v choco >/dev/null 2>&1; then
        cd "$choco_dir"
        choco pack obsctl.nuspec --outputdirectory "$PACKAGE_DIR"
        cd "$PROJECT_ROOT"
        print_success "Chocolatey package (.nupkg) created"
    else
        print_warning "Chocolatey CLI not available - package files created but .nupkg not built"
        print_step "To build the package manually on Windows:"
        print_step "  cd $choco_dir"
        print_step "  choco pack obsctl.nuspec"
        print_step "  choco install obsctl -s . -f  # Test locally"
    fi
}

# Main build process
main() {
    echo -e "${BLUE}🚀 obsctl Multi-Platform Build System${NC}"
    echo "====================================="
    echo ""

    VERSION=$(get_version)
    print_step "Building obsctl v$VERSION for multiple platforms"
    echo ""

    # Clean and create directories
    rm -rf "$BUILD_DIR" "$PACKAGE_DIR"
    mkdir -p "$BUILD_DIR" "$PACKAGE_DIR"

    check_cross_tools
    install_targets

    echo ""
    print_step "Building binaries for all platforms..."

    local successful_builds=()
    local failed_builds=()

    for target in "${TARGETS[@]}"; do
        if build_target "$target"; then
            successful_builds+=("${PLATFORM_NAMES[$target]}")
        else
            failed_builds+=("${PLATFORM_NAMES[$target]}")
        fi
    done

    echo ""
    print_step "Build Results:"
    echo "✅ Successful: ${successful_builds[*]}"
    if [[ ${#failed_builds[@]} -gt 0 ]]; then
        echo "❌ Failed: ${failed_builds[*]}"
    fi

    echo ""
    create_macos_universal_binary
    create_debian_packages
    create_rpm_packages
    update_homebrew_formula
    create_chocolatey_packages
    generate_summary

    echo ""
    print_success "Multi-platform build complete!"
    echo ""
    echo "📁 Binary archives: $BUILD_DIR"
    echo "📦 Package files: $PACKAGE_DIR"
    echo "📋 Release summary: $PACKAGE_DIR/RELEASE_SUMMARY.md"
    echo ""
    echo "Next steps:"
    echo "1. Test packages on target platforms"
    echo "2. Upload archives to GitHub releases"
    echo "3. Update Homebrew formula with release URL and SHA256"
    echo "4. Submit packages to distribution repositories"
}

# Run main function
main "$@"
