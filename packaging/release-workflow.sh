#!/bin/bash
set -e

# Complete release workflow for obsctl
# This script orchestrates the entire release process

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

print_header() {
    echo -e "${PURPLE}$1${NC}"
    echo -e "${PURPLE}$(echo "$1" | sed 's/./=/g')${NC}"
}

print_step() {
    echo -e "${BLUE}🚀 $1${NC}"
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

# Check prerequisites
check_prerequisites() {
    print_step "Checking prerequisites..."

    local missing_tools=()

    # Essential tools
    if ! command -v cargo >/dev/null 2>&1; then
        missing_tools+=("cargo (Rust)")
    fi

    if ! command -v git >/dev/null 2>&1; then
        missing_tools+=("git")
    fi

    # Optional but recommended tools
    local optional_tools=()

    if ! command -v cross >/dev/null 2>&1; then
        optional_tools+=("cross (for easier cross-compilation)")
    fi

    if ! command -v dpkg-deb >/dev/null 2>&1; then
        optional_tools+=("dpkg-deb (for Debian packages)")
    fi

    if ! command -v rpmbuild >/dev/null 2>&1; then
        optional_tools+=("rpmbuild (for RPM packages)")
    fi

    if ! command -v brew >/dev/null 2>&1; then
        optional_tools+=("brew (for testing Homebrew formula)")
    fi

    if [[ ${#missing_tools[@]} -gt 0 ]]; then
        print_error "Missing required tools: ${missing_tools[*]}"
        echo "Please install the missing tools and try again."
        exit 1
    fi

    print_success "All required tools available"

    if [[ ${#optional_tools[@]} -gt 0 ]]; then
        print_warning "Optional tools not available: ${optional_tools[*]}"
        echo "Some packaging features may be limited."
    fi
}

# Clean previous builds
clean_builds() {
    print_step "Cleaning previous builds..."

    cd "$PROJECT_ROOT"

    # Clean Rust builds
    cargo clean

    # Clean release artifacts
    rm -rf target/releases target/packages

    print_success "Build directories cleaned"
}

# Run tests
run_tests() {
    print_step "Running tests..."

    cd "$PROJECT_ROOT"

    # Run unit tests
    cargo test --lib

    # Run integration tests if available
    if [[ -f "tests/integration/run_tests.sh" ]]; then
        echo "Running integration tests..."
        cd tests/integration
        ./run_tests.sh --quick
        cd "$PROJECT_ROOT"
    fi

    print_success "All tests passed"
}

# Build all platforms
build_all_platforms() {
    print_step "Building for all platforms..."

    if [[ -x "$SCRIPT_DIR/build-releases.sh" ]]; then
        "$SCRIPT_DIR/build-releases.sh"
    else
        print_error "Build script not found or not executable: $SCRIPT_DIR/build-releases.sh"
        exit 1
    fi

    print_success "Multi-platform build completed"
}

# Update Homebrew formula
update_homebrew() {
    print_step "Updating Homebrew formula..."

    if [[ -x "$SCRIPT_DIR/homebrew/update-formula-shas.sh" ]]; then
        "$SCRIPT_DIR/homebrew/update-formula-shas.sh"
    else
        print_warning "Homebrew SHA256 update script not found, skipping"
    fi
}

# Generate release notes
generate_release_notes() {
    local version="$1"

    print_step "Generating release notes..."

    local notes_file="$PROJECT_ROOT/target/packages/RELEASE_NOTES_v$version.md"

    cat > "$notes_file" << EOF
# obsctl v$version Release Notes

## 📦 Downloads

### Homebrew (Recommended)
\`\`\`bash
brew tap your-org/obsctl
brew install obsctl
\`\`\`

### Direct Downloads

#### macOS
- [macOS Intel (x64)](https://github.com/your-org/obsctl/releases/download/v$version/obsctl-$version-macos-intel.tar.gz)
- [macOS Apple Silicon (ARM64)](https://github.com/your-org/obsctl/releases/download/v$version/obsctl-$version-macos-arm64.tar.gz)

#### Linux
- [Linux x64](https://github.com/your-org/obsctl/releases/download/v$version/obsctl-$version-linux-x64.tar.gz)
- [Linux ARM64](https://github.com/your-org/obsctl/releases/download/v$version/obsctl-$version-linux-arm64.tar.gz)

#### Windows
- [Windows x64](https://github.com/your-org/obsctl/releases/download/v$version/obsctl-$version-windows-x64.zip)

### Package Managers

#### Debian/Ubuntu
\`\`\`bash
wget https://github.com/your-org/obsctl/releases/download/v$version/obsctl_${version}_amd64.deb
sudo dpkg -i obsctl_${version}_amd64.deb

# For ARM64
wget https://github.com/your-org/obsctl/releases/download/v$version/obsctl_${version}_arm64.deb
sudo dpkg -i obsctl_${version}_arm64.deb
\`\`\`

#### RHEL/CentOS/Fedora
\`\`\`bash
wget https://github.com/your-org/obsctl/releases/download/v$version/obsctl-${version}-1.x86_64.rpm
sudo rpm -i obsctl-${version}-1.x86_64.rpm

# For ARM64
wget https://github.com/your-org/obsctl/releases/download/v$version/obsctl-${version}-1.aarch64.rpm
sudo rpm -i obsctl-${version}-1.aarch64.rpm
\`\`\`

## 🎯 What's New

### Dashboard Management
- \`obsctl config dashboard install\` - Install Grafana dashboards automatically
- \`obsctl config dashboard list\` - List installed obsctl dashboards
- \`obsctl config dashboard info\` - Show dashboard information
- Security-focused: Only manages obsctl-specific dashboards

### Configuration Management
- \`obsctl config configure\` - Interactive AWS configuration setup
- \`obsctl config set <key> <value>\` - Set any configuration value
- \`obsctl config get <key>\` - Retrieve configuration values
- \`obsctl config list\` - View all configuration
- Full AWS profile support with \`--profile\` flag

### Package Integration
- Dashboard files included in all packages
- Automatic installation to standard locations
- Man page and bash completion included
- Post-install scripts for proper setup

## 🔧 Installation

### From Archive
1. Download the appropriate archive for your platform
2. Extract: \`tar -xzf obsctl-$version-<platform>.tar.gz\`
3. Copy binary to PATH: \`sudo cp obsctl /usr/local/bin/\`
4. Install man page: \`sudo cp obsctl.1 /usr/local/share/man/man1/\`
5. Install bash completion: \`sudo cp obsctl.bash-completion /usr/local/share/bash-completion/completions/obsctl\`

### Dashboard Setup
After installation:
\`\`\`bash
# Install dashboards to local Grafana
obsctl config dashboard install

# Install to remote Grafana
obsctl config dashboard install --url http://grafana.company.com:3000 --username admin --password secret

# List installed dashboards
obsctl config dashboard list
\`\`\`

## 🛠️ Configuration

### Quick Start
\`\`\`bash
# Interactive configuration
obsctl config configure

# Set individual values
obsctl config set region us-west-2
obsctl config set aws_access_key_id YOUR_KEY
obsctl config set endpoint_url http://localhost:9000

# Use profiles
obsctl config set region eu-west-1 --profile production
\`\`\`

## 📊 OpenTelemetry

obsctl includes built-in OpenTelemetry support:
- Metrics for all S3 operations
- Automatic instrumentation
- Grafana dashboard integration
- Configurable OTEL emission

## 🔗 Links

- [Documentation](https://github.com/your-org/obsctl/blob/main/README.md)
- [Configuration Guide](https://github.com/your-org/obsctl/blob/main/docs/index.md)
- [Dashboard Examples](https://github.com/your-org/obsctl/tree/main/packaging/dashboards)

## 📋 Checksums

All release files include SHA256 checksums for verification. See the release assets for \`.sha256\` files.

---

For issues or questions, please visit our [GitHub repository](https://github.com/your-org/obsctl).
EOF

    print_success "Release notes generated: $notes_file"
}

# Show release summary
show_summary() {
    local version="$1"

    print_header "Release Summary for obsctl v$version"
    echo ""

    # Show what was built
    if [[ -d "$PROJECT_ROOT/target/releases" ]]; then
        echo "📦 Binary Archives:"
        find "$PROJECT_ROOT/target/releases" -name "*.tar.gz" -o -name "*.zip" | while read -r file; do
            local filename=$(basename "$file")
            local size=$(du -h "$file" | cut -f1)
            echo "  ✅ $filename ($size)"
        done
        echo ""
    fi

    # Show packages
    if [[ -d "$PROJECT_ROOT/target/packages" ]]; then
        echo "📦 Package Files:"
        find "$PROJECT_ROOT/target/packages" -name "*.deb" -o -name "*.rpm" | while read -r file; do
            local filename=$(basename "$file")
            local size=$(du -h "$file" | cut -f1)
            echo "  ✅ $filename ($size)"
        done
        echo ""
    fi

    # Show next steps
    echo "🚀 Next Steps:"
    echo "1. Review all generated files in target/releases/ and target/packages/"
    echo "2. Test packages on target platforms"
    echo "3. Create GitHub release and upload archives"
    echo "4. Update Homebrew formula URLs if needed"
    echo "5. Submit packages to distribution repositories"
    echo "6. Update documentation with new features"
    echo ""

    echo "📋 Release Artifacts:"
    echo "  - Binary archives: target/releases/"
    echo "  - Package files: target/packages/"
    echo "  - Release notes: target/packages/RELEASE_NOTES_v$version.md"
    echo "  - Release summary: target/packages/RELEASE_SUMMARY.md"
    if [[ -f "$SCRIPT_DIR/homebrew/obsctl.rb" ]]; then
        echo "  - Homebrew formula: packaging/homebrew/obsctl.rb"
    fi
}

# Main workflow
main() {
    print_header "obsctl Release Workflow"
    echo ""

    local version=$(get_version)
    echo "📦 Building obsctl v$version"
    echo "📁 Project: $PROJECT_ROOT"
    echo ""

    # Confirm before proceeding
    read -p "❓ Continue with release build for v$version? (y/N): " confirm
    if [[ ! "$confirm" =~ ^[Yy]$ ]]; then
        echo "Release cancelled."
        exit 0
    fi

    echo ""

    # Execute workflow steps
    check_prerequisites
    echo ""

    clean_builds
    echo ""

    run_tests
    echo ""

    build_all_platforms
    echo ""

    update_homebrew
    echo ""

    generate_release_notes "$version"
    echo ""

    show_summary "$version"

    print_success "Release workflow completed successfully!"
}

# Handle command line arguments
case "${1:-}" in
    --help|-h)
        echo "obsctl Release Workflow"
        echo ""
        echo "Usage: $0 [options]"
        echo ""
        echo "Options:"
        echo "  --help, -h     Show this help message"
        echo "  --no-test      Skip running tests"
        echo "  --no-clean     Skip cleaning previous builds"
        echo ""
        echo "This script will:"
        echo "1. Check prerequisites"
        echo "2. Clean previous builds"
        echo "3. Run tests"
        echo "4. Build for all platforms"
        echo "5. Create packages (deb, rpm)"
        echo "6. Update Homebrew formula"
        echo "7. Generate release notes"
        echo ""
        exit 0
        ;;
    --no-test)
        run_tests() { print_warning "Skipping tests (--no-test)"; }
        ;;
    --no-clean)
        clean_builds() { print_warning "Skipping clean (--no-clean)"; }
        ;;
esac

# Run main workflow
main "$@"
