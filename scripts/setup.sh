#!/bin/bash
# LogStream Setup Script

set -e

echo "🚀 Setting up LogStream development environment..."

# Check if Rust is installed
if command -v rustc &> /dev/null; then
    echo "✓ Rust found: $(rustc --version)"
else
    echo "✗ Rust not found. Please install from https://rustup.rs/"
    exit 1
fi

# Create directories
echo "Creating directories..."
sudo mkdir -p /var/log/logstream
sudo chmod 755 /var/log/logstream

# Build project
echo "Building LogStream..."
cargo build --release

echo "✓ Setup completed!"
echo ""
echo "Quick start:"
echo "  cargo run --bin logstream-server"
echo "  cargo run --example client_example"
