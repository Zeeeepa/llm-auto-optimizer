#!/bin/bash

set -e

echo "Preparing npm packages..."

# Build the CLI binary first
echo "Building Rust CLI binary..."
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release -p llm-optimizer-cli

# Create packages directory
mkdir -p npm/packages

# For Linux x64 (current platform)
PLATFORM_DIR="npm/packages/cli-linux-x64"
mkdir -p "$PLATFORM_DIR/bin"

# Copy the binary
cp target/release/llm-optimizer "$PLATFORM_DIR/bin/"
chmod +x "$PLATFORM_DIR/bin/llm-optimizer"

echo "✓ Binary copied to $PLATFORM_DIR/bin/"

echo ""
echo "✓ Platform package prepared for linux-x64"
echo ""
echo "Next steps:"
echo "1. The build script created package.json files for all platforms"
echo "2. Linux x64 binary is ready in: $PLATFORM_DIR/bin/"
echo "3. For other platforms, you'll need to:"
echo "   - Build on the target platform, OR"
echo "   - Use GitHub Actions with cross-compilation"
echo "4. Once all binaries are ready, publish:"
echo "   cd npm/packages/cli-linux-x64 && npm publish --access public"
echo "   cd npm && npm publish --access public"
