#!/bin/bash
# Test cross-compilation for obsctl
# This script tests cross-compilation locally before pushing to CI

set -e

echo "🔧 Testing Cross-Compilation for obsctl"
echo "========================================"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if cross is installed
if ! command -v cross &> /dev/null; then
    print_warning "Cross tool not found. Installing..."
    cargo install cross --git https://github.com/cross-rs/cross
fi

# Define targets to test
TARGETS=(
    "x86_64-unknown-linux-gnu"      # Native Linux
    "aarch64-unknown-linux-gnu"     # ARM64 Linux
    "armv7-unknown-linux-gnueabihf" # ARM v7 Linux
    "x86_64-pc-windows-gnu"         # Windows
    "x86_64-apple-darwin"           # macOS Intel (if on macOS)
    "aarch64-apple-darwin"          # macOS ARM64 (if on macOS)
)

# Test native build first
print_status "Testing native build..."
if cargo build --release; then
    print_success "Native build successful"
else
    print_error "Native build failed"
    exit 1
fi

# Test cross-compilation for each target
for target in "${TARGETS[@]}"; do
    print_status "Testing cross-compilation for $target..."

    # Skip macOS targets if not on macOS
    if [[ "$target" == *"apple-darwin"* ]] && [[ "$OSTYPE" != "darwin"* ]]; then
        print_warning "Skipping $target (not on macOS)"
        continue
    fi

    # Add target if not already installed
    if ! rustup target list --installed | grep -q "$target"; then
        print_status "Installing target $target..."
        rustup target add "$target"
    fi

    # Try cross-compilation
    if [[ "$target" == "x86_64-unknown-linux-gnu" ]]; then
        # Native build for x86_64 Linux
        if cargo build --target "$target" --release; then
            print_success "✅ $target: Build successful"
        else
            print_error "❌ $target: Build failed"
        fi
    elif [[ "$target" == *"apple-darwin"* ]] && [[ "$OSTYPE" == "darwin"* ]]; then
        # Native macOS builds
        if cargo build --target "$target" --release; then
            print_success "✅ $target: Build successful"
        else
            print_error "❌ $target: Build failed"
        fi
    else
        # Cross-compilation builds
        if cross build --target "$target" --release; then
            print_success "✅ $target: Cross-compilation successful"
        else
            print_error "❌ $target: Cross-compilation failed"
            print_warning "This target may need additional setup or may fail in CI"
        fi
    fi

    echo ""
done

# Summary
echo "🎯 Cross-Compilation Test Summary"
echo "================================="
print_status "All available targets tested"
print_status "Check output above for any failures"
print_status "Failed targets may need:"
print_status "  - Additional system dependencies"
print_status "  - Docker setup for cross tool"
print_status "  - Platform-specific configuration"

echo ""
print_success "Cross-compilation test completed!"
print_status "You can now push to trigger CI builds"
