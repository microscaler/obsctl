#!/bin/bash

# Fix Clippy Warnings Script
# This script automatically fixes clippy warnings and checks for remaining issues

set -e

echo "🔧 Fixing clippy warnings..."

# Apply automatic fixes
cargo clippy --fix --allow-dirty --allow-staged --all-targets --all-features

# Check if there are any remaining warnings/errors
echo "🔍 Checking for remaining clippy issues..."
if ! cargo clippy --all-targets --all-features -- -D warnings; then
    echo ""
    echo "❌ Manual fixes required!"
    echo "Some clippy issues cannot be automatically fixed."
    echo "Please review the warnings above and fix them manually."
    exit 1
else
    echo "✅ All clippy issues resolved!"
    echo "Ready to commit."
fi
