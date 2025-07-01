#!/bin/bash

# Build DXT package for tauri-mcp

set -e

echo "Building tauri-mcp DXT package..."

# Clean previous builds
rm -rf dist/
mkdir -p dist/

# Build for all platforms
echo "Building binaries..."

# macOS
if [[ "$OSTYPE" == "darwin"* ]]; then
    echo "Building for macOS..."
    cargo build --release
    cp target/release/tauri-mcp dist/tauri-mcp-darwin
    
    # Cross-compile for Linux if possible
    if command -v cross &> /dev/null; then
        echo "Building for Linux..."
        cross build --target x86_64-unknown-linux-gnu --release
        cp target/x86_64-unknown-linux-gnu/release/tauri-mcp dist/tauri-mcp-linux
    fi
fi

# Linux
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    echo "Building for Linux..."
    cargo build --release
    cp target/release/tauri-mcp dist/tauri-mcp-linux
    
    # Cross-compile for macOS if possible
    if command -v cross &> /dev/null; then
        echo "Building for macOS..."
        cross build --target x86_64-apple-darwin --release
        cp target/x86_64-apple-darwin/release/tauri-mcp dist/tauri-mcp-darwin
    fi
fi

# Copy manifest
cp manifest.json dist/

# Create the DXT archive
cd dist
zip -r ../tauri-mcp.dxt *
cd ..

echo "DXT package created: tauri-mcp.dxt"
echo ""
echo "To install in a supporting application:"
echo "  - Drag and drop tauri-mcp.dxt into the application"
echo "  - Or use the application's extension manager"