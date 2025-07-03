#!/bin/bash
set -e

# Script to update Homebrew formula with SHA256 values from built archives
# This should be run after building releases with build-releases.sh

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
BUILD_DIR="$PROJECT_ROOT/target/releases"
FORMULA_FILE="$SCRIPT_DIR/obsctl.rb"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

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

# Calculate SHA256 for a file
calculate_sha256() {
    local file="$1"
    if [[ -f "$file" ]]; then
        shasum -a 256 "$file" | cut -d' ' -f1
    else
        echo ""
    fi
}

# Update formula with SHA256 values
update_formula() {
    local version="$1"

    print_step "Updating Homebrew formula with SHA256 values..."

    # Check if formula file exists
    if [[ ! -f "$FORMULA_FILE" ]]; then
        print_error "Formula file not found: $FORMULA_FILE"
        exit 1
    fi

    # Archive file patterns
    local macos_intel_archive="$BUILD_DIR/obsctl-$version-macos-intel.tar.gz"
    local macos_arm64_archive="$BUILD_DIR/obsctl-$version-macos-arm64.tar.gz"
    local linux_x64_archive="$BUILD_DIR/obsctl-$version-linux-x64.tar.gz"
    local linux_arm64_archive="$BUILD_DIR/obsctl-$version-linux-arm64.tar.gz"

    # Calculate SHA256 values
    local macos_intel_sha256=$(calculate_sha256 "$macos_intel_archive")
    local macos_arm64_sha256=$(calculate_sha256 "$macos_arm64_archive")
    local linux_x64_sha256=$(calculate_sha256 "$linux_x64_archive")
    local linux_arm64_sha256=$(calculate_sha256 "$linux_arm64_archive")

    # Display found archives and SHA256 values
    echo ""
    print_step "Found archives and SHA256 values:"

    if [[ -n "$macos_intel_sha256" ]]; then
        echo "✅ macOS Intel: $macos_intel_sha256"
    else
        print_warning "macOS Intel archive not found: $macos_intel_archive"
    fi

    if [[ -n "$macos_arm64_sha256" ]]; then
        echo "✅ macOS ARM64: $macos_arm64_sha256"
    else
        print_warning "macOS ARM64 archive not found: $macos_arm64_archive"
    fi

    if [[ -n "$linux_x64_sha256" ]]; then
        echo "✅ Linux x64: $linux_x64_sha256"
    else
        print_warning "Linux x64 archive not found: $linux_x64_archive"
    fi

    if [[ -n "$linux_arm64_sha256" ]]; then
        echo "✅ Linux ARM64: $linux_arm64_sha256"
    else
        print_warning "Linux ARM64 archive not found: $linux_arm64_archive"
    fi

    echo ""

    # Create backup
    cp "$FORMULA_FILE" "$FORMULA_FILE.backup"
    print_step "Created backup: $FORMULA_FILE.backup"

    # Update SHA256 values in formula
    local temp_file=$(mktemp)

    # Process the formula file line by line
    while IFS= read -r line; do
        case "$line" in
            *"REPLACE_WITH_INTEL_SHA256"*)
                if [[ -n "$macos_intel_sha256" ]]; then
                    echo "      sha256 \"$macos_intel_sha256\""
                else
                    echo "$line"
                fi
                ;;
            *"REPLACE_WITH_ARM64_SHA256"*)
                if [[ -n "$macos_arm64_sha256" ]]; then
                    echo "      sha256 \"$macos_arm64_sha256\""
                else
                    echo "$line"
                fi
                ;;
            *"REPLACE_WITH_LINUX_X64_SHA256"*)
                if [[ -n "$linux_x64_sha256" ]]; then
                    echo "      sha256 \"$linux_x64_sha256\""
                else
                    echo "$line"
                fi
                ;;
            *"REPLACE_WITH_LINUX_ARM64_SHA256"*)
                if [[ -n "$linux_arm64_sha256" ]]; then
                    echo "      sha256 \"$linux_arm64_sha256\""
                else
                    echo "$line"
                fi
                ;;
            *)
                echo "$line"
                ;;
        esac
    done < "$FORMULA_FILE" > "$temp_file"

    # Replace the original file
    mv "$temp_file" "$FORMULA_FILE"

    print_success "Formula updated with SHA256 values"
}

# Validate the updated formula
validate_formula() {
    print_step "Validating updated formula..."

    # Check Ruby syntax
    if command -v ruby >/dev/null 2>&1; then
        if ruby -c "$FORMULA_FILE" >/dev/null 2>&1; then
            print_success "Formula syntax is valid"
        else
            print_error "Formula syntax error"
            ruby -c "$FORMULA_FILE"
            exit 1
        fi
    else
        print_warning "Ruby not available, skipping syntax check"
    fi

    # Check if any placeholders remain
    local remaining_placeholders=$(grep -c "REPLACE_WITH_.*_SHA256" "$FORMULA_FILE" || true)

    if [[ "$remaining_placeholders" -gt 0 ]]; then
        print_warning "$remaining_placeholders placeholder(s) still remain in formula"
        echo "This may be expected if some platform builds failed."
        echo ""
        grep "REPLACE_WITH_.*_SHA256" "$FORMULA_FILE" || true
    else
        print_success "All SHA256 placeholders have been replaced"
    fi
}

# Generate GitHub release URLs
generate_release_info() {
    local version="$1"

    print_step "GitHub Release Information"
    echo "=========================="
    echo ""
    echo "When creating the GitHub release, use these URLs in the formula:"
    echo ""

    local base_url="https://github.com/your-org/obsctl/releases/download/v$version"

    echo "macOS Intel:"
    echo "  URL: $base_url/obsctl-$version-macos-intel.tar.gz"
    if [[ -f "$BUILD_DIR/obsctl-$version-macos-intel.tar.gz" ]]; then
        local sha256=$(calculate_sha256 "$BUILD_DIR/obsctl-$version-macos-intel.tar.gz")
        echo "  SHA256: $sha256"
    fi
    echo ""

    echo "macOS ARM64:"
    echo "  URL: $base_url/obsctl-$version-macos-arm64.tar.gz"
    if [[ -f "$BUILD_DIR/obsctl-$version-macos-arm64.tar.gz" ]]; then
        local sha256=$(calculate_sha256 "$BUILD_DIR/obsctl-$version-macos-arm64.tar.gz")
        echo "  SHA256: $sha256"
    fi
    echo ""

    echo "Linux x64:"
    echo "  URL: $base_url/obsctl-$version-linux-x64.tar.gz"
    if [[ -f "$BUILD_DIR/obsctl-$version-linux-x64.tar.gz" ]]; then
        local sha256=$(calculate_sha256 "$BUILD_DIR/obsctl-$version-linux-x64.tar.gz")
        echo "  SHA256: $sha256"
    fi
    echo ""

    echo "Linux ARM64:"
    echo "  URL: $base_url/obsctl-$version-linux-arm64.tar.gz"
    if [[ -f "$BUILD_DIR/obsctl-$version-linux-arm64.tar.gz" ]]; then
        local sha256=$(calculate_sha256 "$BUILD_DIR/obsctl-$version-linux-arm64.tar.gz")
        echo "  SHA256: $sha256"
    fi
    echo ""
}

# Main function
main() {
    echo -e "${BLUE}🍺 Homebrew Formula SHA256 Updater${NC}"
    echo "===================================="
    echo ""

    # Check if build directory exists
    if [[ ! -d "$BUILD_DIR" ]]; then
        print_error "Build directory not found: $BUILD_DIR"
        echo "Please run packaging/build-releases.sh first to build the archives."
        exit 1
    fi

    local version=$(get_version)
    echo "📦 Version: $version"
    echo "📁 Build directory: $BUILD_DIR"
    echo "📄 Formula file: $FORMULA_FILE"
    echo ""

    update_formula "$version"
    validate_formula
    generate_release_info "$version"

    echo ""
    print_success "Formula update complete!"
    echo ""
    echo "Next steps:"
    echo "1. Review the updated formula: $FORMULA_FILE"
    echo "2. Test the formula: ./packaging/homebrew/test-formula.sh"
    echo "3. Create GitHub release with the built archives"
    echo "4. Update the repository URLs in the formula if needed"
    echo "5. Submit to your Homebrew tap"
    echo ""
    echo "Backup saved as: $FORMULA_FILE.backup"
}

# Run main function
main "$@"
