#!/bin/bash

# Professional Visual CLI Startup Script
# The most sleek and visually pleasing interactive console known to man

set -e

echo "🎨 Starting Professional Visual CLI Interface"
echo "═══════════════════════════════════════════════════════════════"

# Configuration
DATA_DIR="./data"
FUEGO_DATA_DIR="./fuego-data"
LOG_LEVEL="info"
C0DL3_RPC_URL="http://localhost:9944"
FUEGO_RPC_URL="http://localhost:8546"
VISUAL_MODE="true"
ANIMATIONS_ENABLED="true"
THEME="professional"

# Create data directories if they don't exist
mkdir -p "$DATA_DIR"
mkdir -p "$FUEGO_DATA_DIR"

echo "📊 Configuration:"
echo "  - Data Directory: $DATA_DIR"
echo "  - Fuego Data Directory: $FUEGO_DATA_DIR"
echo "  - Log Level: $LOG_LEVEL"
echo "  - C0DL3 RPC: $C0DL3_RPC_URL"
echo "  - Fuego RPC: $FUEGO_RPC_URL"
echo "  - Visual Mode: $VISUAL_MODE"
echo "  - Animations: $ANIMATIONS_ENABLED"
echo "  - Theme: $THEME"
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

echo "🚀 Starting Professional Visual CLI Interface..."
echo "   This will provide the most stunning visual interface for:"
echo "   - 🎨 Professional visual design and animations"
echo "   - ⛏️ Advanced mining management with progress bars"
echo "   - 🛡️ Beautiful validator management interface"
echo "   - 📊 Real-time status monitoring with live updates"
echo "   - 🎮 Interactive menus and dialogs"
echo "   - 🌟 The most sleek console interface known to man!"
echo ""

# Start visual CLI
cargo run -- \
    --visual-mode true \
    --log-level "$LOG_LEVEL" \
    --data-dir "$DATA_DIR" \
    --fuego-data-dir "$FUEGO_DATA_DIR" \
    --l2-rpc-url "$C0DL3_RPC_URL" \
    --fuego-rpc-url "$FUEGO_RPC_URL" \
    --animations-enabled "$ANIMATIONS_ENABLED" \
    --theme "$THEME"

echo "🛑 Professional Visual CLI stopped"
echo "🌟 Thank you for experiencing the future of blockchain management! 🌟"
