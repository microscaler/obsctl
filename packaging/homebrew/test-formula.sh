#!/bin/bash
set -e

# Test script for obsctl Homebrew formula
# This script helps validate the formula before submission

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FORMULA_FILE="$SCRIPT_DIR/obsctl.rb"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo "🍺 Testing obsctl Homebrew Formula"
echo "=================================="
echo ""

# Check if we're in the right directory
if [[ ! -f "$FORMULA_FILE" ]]; then
    echo "❌ Error: Formula file not found at $FORMULA_FILE"
    exit 1
fi

if [[ ! -f "$PROJECT_ROOT/Cargo.toml" ]]; then
    echo "❌ Error: Not in obsctl project root"
    exit 1
fi

echo "📍 Project root: $PROJECT_ROOT"
echo "📍 Formula file: $FORMULA_FILE"
echo ""

# Step 1: Audit the formula
echo "🔍 Step 1: Auditing formula..."
if command -v brew >/dev/null 2>&1; then
    brew audit --strict "$FORMULA_FILE" || {
        echo "⚠️  Formula audit found issues (this may be expected for local testing)"
    }
else
    echo "⚠️  Homebrew not installed, skipping audit"
fi
echo ""

# Step 2: Check required files exist
echo "📁 Step 2: Checking required files..."
required_files=(
    "packaging/obsctl.1"
    "packaging/obsctl.bash-completion"
    "packaging/dashboards/obsctl-unified.json"
    "packaging/debian/config"
)

for file in "${required_files[@]}"; do
    if [[ -f "$PROJECT_ROOT/$file" ]]; then
        echo "✅ $file"
    else
        echo "❌ Missing: $file"
        exit 1
    fi
done
echo ""

# Step 3: Build the project to ensure it compiles
echo "🔨 Step 3: Building project..."
cd "$PROJECT_ROOT"
cargo build --release
echo "✅ Build successful"
echo ""

# Step 4: Test binary functionality
echo "🧪 Step 4: Testing binary..."
BINARY="$PROJECT_ROOT/target/release/obsctl"
if [[ -f "$BINARY" ]]; then
    echo "Testing --version:"
    "$BINARY" --version
    echo ""

    echo "Testing --help:"
    "$BINARY" --help | head -5
    echo ""

    echo "Testing config command:"
    "$BINARY" config --help | head -5
    echo ""

    echo "✅ Binary tests passed"
else
    echo "❌ Binary not found at $BINARY"
    exit 1
fi
echo ""

# Step 5: Validate JSON dashboard
echo "📊 Step 5: Validating dashboard JSON..."
DASHBOARD="$PROJECT_ROOT/packaging/dashboards/obsctl-unified.json"
if command -v jq >/dev/null 2>&1; then
    if jq empty "$DASHBOARD" 2>/dev/null; then
        echo "✅ Dashboard JSON is valid"

        # Check for required dashboard fields
        title=$(jq -r '.title // empty' "$DASHBOARD")
        uid=$(jq -r '.uid // empty' "$DASHBOARD")

        if [[ -n "$title" && -n "$uid" ]]; then
            echo "✅ Dashboard has title: $title"
            echo "✅ Dashboard has UID: $uid"
        else
            echo "⚠️  Dashboard missing title or UID"
        fi
    else
        echo "❌ Dashboard JSON is invalid"
        exit 1
    fi
else
    echo "⚠️  jq not installed, skipping JSON validation"
fi
echo ""

# Step 6: Check formula syntax
echo "🔧 Step 6: Checking formula syntax..."
if command -v ruby >/dev/null 2>&1; then
    ruby -c "$FORMULA_FILE" >/dev/null && echo "✅ Formula syntax is valid"
else
    echo "⚠️  Ruby not installed, skipping syntax check"
fi
echo ""

# Step 7: Generate installation instructions
echo "📋 Step 7: Installation instructions"
echo "===================================="
echo ""
echo "To test this formula locally:"
echo ""
echo "1. Install directly (recommended for testing):"
echo "   brew install --build-from-source '$FORMULA_FILE'"
echo ""
echo "2. Create a local tap:"
echo "   brew tap-new your-org/obsctl"
echo "   cp '$FORMULA_FILE' \$(brew --repository your-org/obsctl)/Formula/"
echo "   brew install your-org/obsctl/obsctl"
echo ""
echo "3. After installation, test with:"
echo "   obsctl --version"
echo "   obsctl config dashboard info"
echo "   ls \$(brew --prefix)/share/obsctl/dashboards/"
echo ""

echo "🎉 Formula validation complete!"
echo ""
echo "Next steps:"
echo "- Test the actual installation using one of the methods above"
echo "- Update the URL and SHA256 in the formula for release"
echo "- Submit to your Homebrew tap or Homebrew core"
