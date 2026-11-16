#!/bin/bash
set -e

# Build universal binary for macOS (Intel + Apple Silicon)
#
# Prerequisites:
#   - Rust installed via rustup (not Homebrew) with both targets:
#     rustup target add x86_64-apple-darwin aarch64-apple-darwin
#   - lipo (included in Xcode Command Line Tools)
#
# Note: Homebrew's Rust doesn't include cross-compilation targets.
# If you have Homebrew Rust, switch to rustup:
#   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

BINARY_NAME="onefolder"
UNIVERSAL_DIR="target/universal-release"

# Find rustup - check PATH first, then ~/.cargo/bin
if command -v rustup &> /dev/null; then
    RUSTUP="rustup"
    CARGO="cargo"
elif [ -f ~/.cargo/bin/rustup ]; then
    RUSTUP=~/.cargo/bin/rustup
    CARGO=~/.cargo/bin/cargo
else
    echo "❌ Error: rustup not found. This script requires Rust installed via rustup."
    echo
    echo "To install rustup and set up cross-compilation targets:"
    echo "  1. Install rustup: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    echo "  2. Add targets: rustup target add x86_64-apple-darwin aarch64-apple-darwin"
    echo
    echo "If you have Homebrew Rust, you'll need to switch to rustup."
    exit 1
fi

# Check if targets are installed
echo "Checking Rust targets..."
if ! $RUSTUP target list | grep -q "x86_64-apple-darwin (installed)"; then
    echo "Installing x86_64-apple-darwin target..."
    $RUSTUP target add x86_64-apple-darwin
fi

if ! $RUSTUP target list | grep -q "aarch64-apple-darwin (installed)"; then
    echo "Installing aarch64-apple-darwin target..."
    $RUSTUP target add aarch64-apple-darwin
fi

echo "🔨 Building universal binary for macOS (Intel + Apple Silicon)"
echo

# Create output directory
mkdir -p "$UNIVERSAL_DIR"

# Build for x86_64 (Intel)
echo "📦 Building for x86_64-apple-darwin (Intel)..."
$CARGO build --release --target x86_64-apple-darwin

# Build for aarch64 (Apple Silicon)
echo "📦 Building for aarch64-apple-darwin (Apple Silicon)..."
$CARGO build --release --target aarch64-apple-darwin

# Create universal binary
INTEL_BINARY="target/x86_64-apple-darwin/release/$BINARY_NAME"
ARM_BINARY="target/aarch64-apple-darwin/release/$BINARY_NAME"
UNIVERSAL_BINARY="$UNIVERSAL_DIR/$BINARY_NAME"

echo
echo "🔗 Creating universal binary with lipo..."
lipo -create "$INTEL_BINARY" "$ARM_BINARY" -output "$UNIVERSAL_BINARY"

# Verify the universal binary
echo "✓ Universal binary created: $UNIVERSAL_BINARY"
echo
lipo -info "$UNIVERSAL_BINARY"

# Show file sizes
echo
echo "📊 Binary sizes:"
printf "  Intel (x86_64):        "
ls -lh "$INTEL_BINARY" | awk '{print $5}'
printf "  Apple Silicon (arm64): "
ls -lh "$ARM_BINARY" | awk '{print $5}'
printf "  Universal (combined):  "
ls -lh "$UNIVERSAL_BINARY" | awk '{print $5}'

echo
echo "✅ Done! Universal binary ready at: $UNIVERSAL_BINARY"
echo
echo "To install: sudo cp $UNIVERSAL_BINARY /usr/local/bin/"

