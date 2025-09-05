#!/bin/bash

# Build Fuego Daemon Script
# This script builds the actual Fuego daemon from fuego-fresh

set -e

echo "🔨 Building Fuego Daemon from fuego-fresh"
echo "=========================================="

# Configuration
FUEGO_SOURCE_DIR="/Users/aejt/fuegowalletproof/fuego-fresh"
BUILD_DIR="$FUEGO_SOURCE_DIR/build/release"
BINARY_NAME="fuegod"
TARGET_DIR="./bin"

# Create target directory
mkdir -p "$TARGET_DIR"

echo "📁 Source directory: $FUEGO_SOURCE_DIR"
echo "📁 Build directory: $BUILD_DIR"
echo "📁 Target directory: $TARGET_DIR"

# Check if source directory exists
if [ ! -d "$FUEGO_SOURCE_DIR" ]; then
    echo "❌ Fuego source directory not found: $FUEGO_SOURCE_DIR"
    echo "Please ensure fuego-fresh is cloned in the correct location"
    exit 1
fi

# Navigate to fuego source directory
cd "$FUEGO_SOURCE_DIR"

echo "🔍 Checking dependencies..."

# Check for required dependencies
if ! command -v cmake &> /dev/null; then
    echo "❌ CMake not found. Please install CMake"
    exit 1
fi

if ! command -v make &> /dev/null; then
    echo "❌ Make not found. Please install Make"
    exit 1
fi

# Check for Boost libraries
if ! pkg-config --exists boost; then
    echo "⚠️  Boost libraries not found via pkg-config"
    echo "Attempting to install Boost via Homebrew..."
    
    if command -v brew &> /dev/null; then
        echo "🍺 Installing Boost via Homebrew..."
        brew install boost
    else
        echo "❌ Homebrew not found. Please install Boost manually"
        echo "On macOS: brew install boost"
        echo "On Ubuntu: sudo apt-get install libboost-all-dev"
        echo "On CentOS: sudo yum install boost-devel"
        exit 1
    fi
fi

echo "✅ Dependencies check passed"

# Clean previous build
echo "🧹 Cleaning previous build..."
rm -rf "$BUILD_DIR"

# Create build directory
echo "📁 Creating build directory..."
mkdir -p "$BUILD_DIR"

# Configure with CMake
echo "⚙️  Configuring with CMake..."
cd "$BUILD_DIR"

# Try different CMake configurations
if cmake ../.. -DCMAKE_BUILD_TYPE=Release; then
    echo "✅ CMake configuration successful"
else
    echo "⚠️  Standard CMake configuration failed, trying with Boost path..."
    
    # Try with explicit Boost path
    if command -v brew &> /dev/null; then
        BOOST_ROOT=$(brew --prefix boost)
        if cmake ../.. -DCMAKE_BUILD_TYPE=Release -DBOOST_ROOT="$BOOST_ROOT"; then
            echo "✅ CMake configuration with explicit Boost path successful"
        else
            echo "❌ CMake configuration failed"
            exit 1
        fi
    else
        echo "❌ CMake configuration failed and Homebrew not available"
        exit 1
    fi
fi

# Build the daemon
echo "🔨 Building Fuego daemon..."
if make -j$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4); then
    echo "✅ Build successful"
else
    echo "❌ Build failed"
    exit 1
fi

# Find the built binary
echo "🔍 Looking for built binary..."
BINARY_PATH=$(find . -name "$BINARY_NAME" -type f -executable 2>/dev/null | head -1)

if [ -z "$BINARY_PATH" ]; then
    echo "❌ Binary not found. Checking for alternative names..."
    
    # Try alternative binary names
    for alt_name in "fuego" "fuegoX" "fuego-daemon" "fuegoX-daemon"; do
        ALT_PATH=$(find . -name "$alt_name" -type f -executable 2>/dev/null | head -1)
        if [ -n "$ALT_PATH" ]; then
            echo "✅ Found alternative binary: $alt_name"
            BINARY_PATH="$ALT_PATH"
            BINARY_NAME="$alt_name"
            break
        fi
    done
    
    if [ -z "$BINARY_PATH" ]; then
        echo "❌ No executable binary found"
        echo "Available files:"
        find . -type f -executable | head -10
        exit 1
    fi
fi

echo "✅ Found binary: $BINARY_PATH"

# Copy binary to target directory
echo "📋 Copying binary to target directory..."
cp "$BINARY_PATH" "$TARGET_DIR/$BINARY_NAME"
chmod +x "$TARGET_DIR/$BINARY_NAME"

# Verify the binary
echo "🔍 Verifying binary..."
if "$TARGET_DIR/$BINARY_NAME" --version; then
    echo "✅ Binary verification successful"
else
    echo "⚠️  Binary verification failed, but binary exists"
fi

# Create symlink in project root
echo "🔗 Creating symlink in project root..."
cd "/Users/aejt/codl3-implementations/c0dl3-zksync"
ln -sf "$TARGET_DIR/$BINARY_NAME" "./$BINARY_NAME"

echo ""
echo "🎉 Fuego daemon build completed successfully!"
echo "📁 Binary location: $TARGET_DIR/$BINARY_NAME"
echo "🔗 Symlink created: ./$BINARY_NAME"
echo ""
echo "🚀 You can now run the unified daemon with:"
echo "   cargo run -- --unified-daemon true --fuego-binary-path ./$BINARY_NAME"
echo ""
echo "📊 Or use the startup script:"
echo "   ./scripts/start-unified-daemon.sh"
