#!/bin/bash
# Build script for pvenet CLI

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "Building pvenet CLI..."

# Change to project root
cd "$PROJECT_ROOT"

# Build the CLI binary
echo "Building binary..."
cargo build --release --bin pvenet

# Check if build was successful
if [ ! -f "target/release/pvenet" ]; then
    echo "Error: Build failed - binary not found"
    exit 1
fi

echo "✓ Binary built successfully: target/release/pvenet"

# Run basic tests
echo "Running tests..."
cargo test -p pvenet --lib

echo "✓ Tests passed"

# Check binary functionality
echo "Testing binary functionality..."
./target/release/pvenet --version
./target/release/pvenet --help > /dev/null

echo "✓ Binary functionality verified"

# Make compatibility script executable
chmod +x scripts/pvenet-compat.sh

echo "✓ Compatibility script prepared"

echo ""
echo "Build completed successfully!"
echo ""
echo "To install:"
echo "  sudo cp target/release/pvenet /usr/bin/"
echo "  sudo cp docs/pvenet.1 /usr/share/man/man1/"
echo "  sudo mandb"
echo ""
echo "To test:"
echo "  ./target/release/pvenet validate --help"
echo "  ./target/release/pvenet status"
echo "  ./scripts/pvenet-compat.sh --help"