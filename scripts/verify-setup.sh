#!/bin/bash
# Verify project setup

set -e

echo "🔍 Verifying Binance Basis Monitor Setup..."
echo "==========================================="

# Check directory structure
echo "📁 Checking directory structure..."
required_dirs=("py-basis" "rust-basis" "scripts")
for dir in "${required_dirs[@]}"; do
    if [ -d "$dir" ]; then
        echo "  ✅ $dir"
    else
        echo "  ❌ $dir (missing)"
        exit 1
    fi
done

# Check configuration files
echo ""
echo "📄 Checking configuration files..."
if [ -f ".env" ]; then
    echo "  ✅ .env"
    # Show basic config
    echo "  📋 Current configuration:"
    grep -E "^(SYMBOLS|WINDOW_INTERVAL|EMA_WINDOW)=" .env | while read line; do
        echo "    $line"
    done
else
    echo "  ⚠️ .env (missing, creating from example)"
    if [ -f ".env.example" ]; then
        cp .env.example .env
        echo "  ✅ Created .env from example"
    else
        echo "  ❌ .env.example also missing"
        exit 1
    fi
fi

# Check Python project
echo ""
echo "🐍 Checking Python project..."
cd py-basis

# Check pyproject.toml
if [ -f "pyproject.toml" ]; then
    echo "  ✅ pyproject.toml"
else
    echo "  ❌ pyproject.toml (missing)"
    exit 1
fi

# Check main Python files
required_py_files=("main.py" "src/config.py" "src/websocket/client.py" "src/calculator/basis.py" "src/ui/dashboard.py")
for file in "${required_py_files[@]}"; do
    if [ -f "$file" ]; then
        echo "  ✅ $file"
    else
        echo "  ❌ $file (missing)"
        exit 1
    fi
done

cd ..

# Check Rust project
echo ""
echo "🦀 Checking Rust project..."
cd rust-basis

# Check Cargo.toml
if [ -f "Cargo.toml" ]; then
    echo "  ✅ Cargo.toml"
else
    echo "  ❌ Cargo.toml (missing)"
    exit 1
fi

# Check main Rust files
required_rs_files=("src/main.rs" "src/config/settings.rs" "src/websocket/client.rs" "src/calculator/basis.rs" "src/ui/dashboard.rs")
for file in "${required_rs_files[@]}"; do
    if [ -f "$file" ]; then
        echo "  ✅ $file"
    else
        echo "  ❌ $file (missing)"
        exit 1
    fi
done

cd ..

# Check scripts
echo ""
echo "📜 Checking scripts..."
required_scripts=("scripts/start-python.sh" "scripts/start-rust.sh" "scripts/run-performance-test.sh")
for script in "${required_scripts[@]}"; do
    if [ -f "$script" ]; then
        echo "  ✅ $script"
        chmod +x "$script" 2>/dev/null || true
    else
        echo "  ⚠️ $script (missing)"
    fi
done

# Check documentation
echo ""
echo "📚 Checking documentation..."
required_docs=("README.md" "pd-v0.2.md" "PROJECT_COMPLETION.md")
for doc in "${required_docs[@]}"; do
    if [ -f "$doc" ]; then
        echo "  ✅ $doc"
    else
        echo "  ⚠️ $doc (missing)"
    fi
done

# Environment check
echo ""
echo "🌍 Checking environment..."

# Check Python
if command -v python3 &> /dev/null; then
    python_version=$(python3 --version 2>&1)
    echo "  ✅ Python: $python_version"
else
    echo "  ❌ Python3 not found"
fi

# Check Rust
if command -v rustc &> /dev/null; then
    rust_version=$(rustc --version 2>&1)
    echo "  ✅ Rust: $rust_version"
else
    echo "  ⚠️ Rust not found (needed for rust-basis)"
fi

# Check uv
if command -v uv &> /dev/null; then
    echo "  ✅ uv (Python package manager)"
else
    echo "  ⚠️ uv not found (recommended for Python project)"
fi

# Network check
echo ""
echo "🌐 Checking network connectivity..."
if curl -s --head https://binance.com | grep "200 OK" > /dev/null; then
    echo "  ✅ Can reach Binance"
else
    echo "  ⚠️ Cannot reach Binance (check network/firewall)"
fi

echo ""
echo "==========================================="
echo "✅ Setup verification complete!"
echo ""
echo "Next steps:"
echo "1. Review configuration in .env file"
echo "2. Start Python version: ./scripts/start-python.sh"
echo "3. Start Rust version: ./scripts/start-rust.sh"
echo "4. Run performance test: ./scripts/run-performance-test.sh"
echo ""
echo "Note: Rust project may need additional debugging for compilation."
echo "Python project should be ready to run immediately."