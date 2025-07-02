#!/bin/bash
set -e

# Release script for obsctl Homebrew formula
# This script helps prepare the formula for release

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FORMULA_FILE="$SCRIPT_DIR/obsctl.rb"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🚀 obsctl Homebrew Formula Release Helper${NC}"
echo "=========================================="
echo ""

# Function to print colored output
print_step() {
    echo -e "${BLUE}$1${NC}"
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

# Check if required tools are installed
check_dependencies() {
    print_step "Checking dependencies..."

    local missing_deps=()

    if ! command -v curl >/dev/null 2>&1; then
        missing_deps+=("curl")
    fi

    if ! command -v shasum >/dev/null 2>&1; then
        missing_deps+=("shasum")
    fi

    if ! command -v git >/dev/null 2>&1; then
        missing_deps+=("git")
    fi

    if [[ ${#missing_deps[@]} -gt 0 ]]; then
        print_error "Missing dependencies: ${missing_deps[*]}"
        echo "Please install the missing tools and try again."
        exit 1
    fi

    print_success "All dependencies available"
}

# Get version from Cargo.toml
get_version() {
    if [[ -f "$PROJECT_ROOT/Cargo.toml" ]]; then
        grep '^version = ' "$PROJECT_ROOT/Cargo.toml" | sed 's/version = "\(.*\)"/\1/'
    else
        print_error "Cargo.toml not found"
        exit 1
    fi
}

# Calculate SHA256 for a URL
calculate_sha256() {
    local url="$1"
    print_step "Downloading and calculating SHA256 for $url..."

    local temp_file=$(mktemp)

    if curl -L -o "$temp_file" "$url" 2>/dev/null; then
        local sha256=$(shasum -a 256 "$temp_file" | cut -d' ' -f1)
        rm "$temp_file"
        echo "$sha256"
    else
        rm "$temp_file"
        print_error "Failed to download $url"
        return 1
    fi
}

# Update formula with new version and SHA256
update_formula() {
    local version="$1"
    local url="$2"
    local sha256="$3"

    print_step "Updating formula with version $version..."

    # Create backup
    cp "$FORMULA_FILE" "$FORMULA_FILE.backup"

    # Update version in URL
    sed -i.tmp "s|url \".*\"|url \"$url\"|g" "$FORMULA_FILE"

    # Update SHA256
    sed -i.tmp "s|sha256 \".*\"|sha256 \"$sha256\"|g" "$FORMULA_FILE"

    # Clean up temp files
    rm "$FORMULA_FILE.tmp"

    print_success "Formula updated"
}

# Main release process
main() {
    check_dependencies

    echo ""
    print_step "Current project information:"

    local version=$(get_version)
    echo "📦 Version: $version"
    echo "📁 Project: $PROJECT_ROOT"
    echo "📄 Formula: $FORMULA_FILE"
    echo ""

    # Ask user for release information
    echo "Please provide release information:"
    echo ""

    read -p "🏷️  Release version (current: $version): " new_version
    new_version=${new_version:-$version}

    read -p "🌐 GitHub repository (org/repo): " repo
    if [[ -z "$repo" ]]; then
        print_error "Repository is required"
        exit 1
    fi

    # Construct download URL
    local base_url="https://github.com/$repo"
    local archive_url="$base_url/archive/v$new_version.tar.gz"

    echo ""
    print_step "Release information:"
    echo "📦 Version: $new_version"
    echo "📁 Repository: $repo"
    echo "🔗 Archive URL: $archive_url"
    echo ""

    read -p "❓ Continue with this configuration? (y/N): " confirm
    if [[ ! "$confirm" =~ ^[Yy]$ ]]; then
        echo "Aborted."
        exit 0
    fi

    echo ""

    # Option 1: Calculate SHA256 from existing release
    echo "Choose SHA256 calculation method:"
    echo "1) Download from GitHub release (requires existing v$new_version tag)"
    echo "2) Manually provide SHA256"
    echo ""

    read -p "Choose option (1/2): " sha_method

    local sha256=""

    case "$sha_method" in
        1)
            sha256=$(calculate_sha256 "$archive_url")
            if [[ $? -ne 0 ]]; then
                print_error "Failed to calculate SHA256. Make sure the release tag v$new_version exists on GitHub."
                exit 1
            fi
            print_success "SHA256: $sha256"
            ;;
        2)
            read -p "Enter SHA256: " sha256
            if [[ -z "$sha256" || ${#sha256} -ne 64 ]]; then
                print_error "Invalid SHA256 (must be 64 characters)"
                exit 1
            fi
            ;;
        *)
            print_error "Invalid option"
            exit 1
            ;;
    esac

    echo ""

    # Update the formula
    update_formula "$new_version" "$archive_url" "$sha256"

    echo ""
    print_step "Updated formula content:"
    echo "------------------------"
    grep -A 2 -B 2 "url\|sha256" "$FORMULA_FILE"
    echo ""

    print_success "Formula updated successfully!"
    echo ""
    echo "Next steps:"
    echo "1. Review the updated formula: $FORMULA_FILE"
    echo "2. Test the formula: ./packaging/homebrew/test-formula.sh"
    echo "3. Commit the changes: git add $FORMULA_FILE && git commit -m 'Update Homebrew formula to v$new_version'"
    echo "4. Submit to your Homebrew tap or create a pull request to Homebrew core"
    echo ""
    echo "Backup saved as: $FORMULA_FILE.backup"
}

# Run main function
main "$@"
