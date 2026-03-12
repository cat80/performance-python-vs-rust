#!/bin/bash
# Start script for Python Binance Basis Monitor

set -e

echo "🚀 Starting Python Binance Basis Monitor..."

# Change to project directory
cd "$(dirname "$0")/../py-basis"

# Check if uv is installed
if ! command -v uv &> /dev/null; then
    echo "❌ uv is not installed. Installing..."
    curl -LsSf https://astral.sh/uv/install.sh | sh
    source "$HOME/.cargo/env"
fi

# Create virtual environment if it doesn't exist
if [ ! -d ".venv" ]; then
    echo "📦 Creating virtual environment..."
    uv venv
fi

# Activate virtual environment
if [ -f ".venv/bin/activate" ]; then
    source .venv/bin/activate
elif [ -f ".venv/Scripts/activate" ]; then
    source .venv/Scripts/activate
else
    echo "❌ Could not find virtual environment activation script"
    exit 1
fi

# Install dependencies
echo "📦 Installing dependencies..."
uv pip install -e .

# Create logs directory
mkdir -p logs

# Run the application
echo "🚀 Starting application..."
echo "📋 Configuration:"
echo "   Symbols: $(grep SYMBOLS ../.env | cut -d= -f2)"
echo "   Window Interval: $(grep WINDOW_INTERVAL ../.env | cut -d= -f2)s"
echo "   EMA Window: $(grep EMA_WINDOW ../.env | cut -d= -f2)"

python main.py "$@"