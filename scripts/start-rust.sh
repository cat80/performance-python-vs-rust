#!/bin/bash
# Start script for Rust Binance Basis Monitor

set -e

echo "🚀 Starting Rust Binance Basis Monitor..."

# Change to project directory
cd "$(dirname "$0")/../rust-basis"

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust is not installed. Installing..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

# Create logs directory
mkdir -p logs

# Build in release mode for performance
echo "🔨 Building in release mode..."
cargo build --release

# Run the application
echo "🚀 Starting application..."
echo "📋 Configuration:"
echo "   Symbols: $(grep SYMBOLS ../.env | cut -d= -f2)"
echo "   Window Interval: $(grep WINDOW_INTERVAL ../.env | cut -d= -f2)s"
echo "   EMA Window: $(grep EMA_WINDOW ../.env | cut -d= -f2)"

./target/release/rust-basis