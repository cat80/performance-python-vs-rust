#!/bin/bash
# Performance test script for Binance Basis Monitor

set -e

echo "📊 Performance Test: Python vs Rust"
echo "====================================="

# Backup current .env
if [ -f "../.env" ]; then
    cp ../.env ../.env.backup
    echo "✅ Backed up current .env file"
fi

# Create test configuration
cat > ../.env.test << 'EOF'
# Performance Test Configuration
SYMBOLS=BTCUSDT,ETHUSDT,SOLUSDT,BNBUSDT,ADAUSDT
WINDOW_INTERVAL=60
EMA_WINDOW=30
WS_RECONNECT_INTERVAL=5
WS_TIMEOUT=30
WS_MAX_RETRIES=10
QUEUE_MAX_SIZE=10000
QUEUE_WARNING_THRESHOLD=8000
UI_REFRESH_INTERVAL=2
LOG_OUTPUT_INTERVAL=10
METRICS_OUTPUT_INTERVAL=60
LOG_LEVEL=INFO
LOG_FILE=logs/test.log
LOG_MAX_SIZE=100MB
LOG_BACKUP_COUNT=10
PERFORMANCE_TEST_MODE=false
TEST_SYMBOL_COUNT=10
TEST_DURATION_HOURS=1
EOF

echo "📋 Test Configuration:"
echo "   - 5 symbols"
echo "   - 60-second window interval"
echo "   - 30 EMA window"
echo "   - 1 hour test duration"

# Function to run test
run_test() {
    local name=$1
    local script=$2

    echo ""
    echo "🧪 Testing $name..."
    echo "-------------------"

    # Copy test configuration
    cp ../.env.test ../.env

    # Start time
    local start_time=$(date +%s)

    # Run the application for 5 minutes
    timeout 300 bash "$script" || true

    # End time
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))

    echo "⏱️  $name ran for $duration seconds"

    # Collect logs
    if [ -f "logs/$name-perf.log" ]; then
        echo "📈 Performance summary for $name:"
        tail -20 "logs/$name-perf.log" | grep -E "(Performance|Rate|Latency|Queue)" | tail -5
    fi

    # Cleanup
    sleep 2
}

# Create test directories
mkdir -p logs

# Test Python
cd ../py-basis
run_test "Python" "../scripts/start-python.sh"

# Test Rust
cd ../rust-basis
run_test "Rust" "../scripts/start-rust.sh"

# Restore original .env
if [ -f "../.env.backup" ]; then
    mv ../.env.backup ../.env
    echo "✅ Restored original .env file"
fi

# Cleanup test config
rm -f ../.env.test

echo ""
echo "📊 Test Complete!"
echo "================="
echo "Check the logs directory for detailed performance data."
echo "Python logs: logs/py-basis.log"
echo "Rust logs: logs/rust-basis.log"