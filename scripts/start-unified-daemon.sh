#!/bin/bash

# Unified C0DL3-Fuego Daemon Startup Script
# This script starts both C0DL3 and Fuego mining in a single daemon

set -e

echo "🚀 Starting Unified C0DL3-Fuego Daemon"
echo "========================================"

# Configuration
DATA_DIR="./data"
LOG_LEVEL="info"
P2P_PORT=30333
RPC_PORT=9944
FUEGO_RPC_URL="http://localhost:8546"
FUEGO_WALLET_ADDRESS="0x1111111111111111111111111111111111111111"
AUX_POW_TAG="C0DL3-MERGE-MINING"
CN_UPX2_DIFFICULTY=1000
C0DL3_BLOCK_TIME=30
FUEGO_BLOCK_TIME=480
FUEGO_BINARY_PATH="./fuegod"
FUEGO_DATA_DIR="./fuego-data"
FUEGO_P2P_PORT=10808

# Create data directory if it doesn't exist
mkdir -p "$DATA_DIR"

# Create Fuego data directory if it doesn't exist
mkdir -p "$FUEGO_DATA_DIR"

echo "📊 Configuration:"
echo "  - Data Directory: $DATA_DIR"
echo "  - Log Level: $LOG_LEVEL"
echo "  - P2P Port: $P2P_PORT"
echo "  - RPC Port: $RPC_PORT"
echo "  - C0DL3 Block Time: ${C0DL3_BLOCK_TIME}s"
echo "  - Fuego Block Time: ${FUEGO_BLOCK_TIME}s"
echo "  - Fuego RPC: $FUEGO_RPC_URL"
echo "  - Fuego Binary: $FUEGO_BINARY_PATH"
echo "  - Fuego Data Dir: $FUEGO_DATA_DIR"
echo "  - Fuego P2P Port: $FUEGO_P2P_PORT"
echo "  - AuxPoW Tag: $AUX_POW_TAG"
echo "  - CN-UPX/2 Difficulty: $CN_UPX2_DIFFICULTY"
echo ""

# Check if Fuego binary exists
if [ ! -f "$FUEGO_BINARY_PATH" ]; then
    echo "⚠️  Fuego binary not found at $FUEGO_BINARY_PATH"
    echo "🔨 Attempting to build Fuego daemon..."
    
    if [ -f "./scripts/build-fuego-daemon.sh" ]; then
        ./scripts/build-fuego-daemon.sh
        echo "✅ Fuego daemon build completed"
    else
        echo "❌ Build script not found. Please build Fuego daemon manually"
        echo "   or ensure the binary exists at $FUEGO_BINARY_PATH"
        exit 1
    fi
else
    echo "✅ Fuego binary found at $FUEGO_BINARY_PATH"
fi
echo ""

# Start unified daemon
cargo run -- \
    --log-level "$LOG_LEVEL" \
    --data-dir "$DATA_DIR" \
    --p2p-port "$P2P_PORT" \
    --rpc-port "$RPC_PORT" \
    --target-block-time "$C0DL3_BLOCK_TIME" \
    --merge-mining-enabled true \
    --fuego-rpc-url "$FUEGO_RPC_URL" \
    --fuego-wallet-address "$FUEGO_WALLET_ADDRESS" \
    --aux-pow-tag "$AUX_POW_TAG" \
    --cn-upx2-difficulty "$CN_UPX2_DIFFICULTY" \
    --fuego-block-time "$FUEGO_BLOCK_TIME" \
    --fuego-binary-path "$FUEGO_BINARY_PATH" \
    --fuego-data-dir "$FUEGO_DATA_DIR" \
    --fuego-p2p-port "$FUEGO_P2P_PORT" \
    --unified-daemon true

echo "🛑 Unified daemon stopped"
