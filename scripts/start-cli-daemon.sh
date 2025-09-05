#!/bin/bash

# Unified CLI Daemon Startup Script
# Interactive wrapper for both zkC0DL3 and Fuego daemons

set -e

echo "🎮 Starting Unified CLI Daemon"
echo "=================================="

# Configuration
DATA_DIR="./data"
FUEGO_DATA_DIR="./fuego-data"
LOG_LEVEL="info"
C0DL3_RPC_URL="http://localhost:9944"
FUEGO_RPC_URL="http://localhost:8546"
INTERACTIVE_MODE="true"
STATUS_REFRESH_INTERVAL=5

# Create data directories if they don't exist
mkdir -p "$DATA_DIR"
mkdir -p "$FUEGO_DATA_DIR"

echo "📊 Configuration:"
echo "  - Data Directory: $DATA_DIR"
echo "  - Fuego Data Directory: $FUEGO_DATA_DIR"
echo "  - Log Level: $LOG_LEVEL"
echo "  - C0DL3 RPC: $C0DL3_RPC_URL"
echo "  - Fuego RPC: $FUEGO_RPC_URL"
echo "  - Interactive Mode: $INTERACTIVE_MODE"
echo "  - Status Refresh: ${STATUS_REFRESH_INTERVAL}s"
echo ""

# Check if Fuego binary exists
if [ ! -f "./fuegod" ]; then
    echo "⚠️  Fuego binary not found at ./fuegod"
    echo "🔨 Attempting to build Fuego daemon..."
    
    if [ -f "./scripts/build-fuego-daemon.sh" ]; then
        ./scripts/build-fuego-daemon.sh
        echo "✅ Fuego daemon build completed"
    else
        echo "❌ Build script not found. Please build Fuego daemon manually"
        echo "   or ensure the binary exists at ./fuegod"
        exit 1
    fi
else
    echo "✅ Fuego binary found at ./fuegod"
fi
echo ""

echo "🚀 Starting CLI daemon..."
echo "   This will provide an interactive interface for:"
echo "   - Mining management (C0DL3 + Fuego)"
echo "   - Validator monitoring (Eldorados + Elderfiers)"
echo "   - Status monitoring and statistics"
echo "   - Daemon management"
echo ""

# Start CLI daemon
cargo run -- \
    --cli-mode true \
    --log-level "$LOG_LEVEL" \
    --data-dir "$DATA_DIR" \
    --fuego-data-dir "$FUEGO_DATA_DIR" \
    --l2-rpc-url "$C0DL3_RPC_URL" \
    --fuego-rpc-url "$FUEGO_RPC_URL" \
    --interactive-mode "$INTERACTIVE_MODE" \
    --status-refresh-interval "$STATUS_REFRESH_INTERVAL"

echo "🛑 CLI daemon stopped"
