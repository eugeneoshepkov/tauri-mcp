#!/bin/bash

# Build DXT package for tauri-mcp with Node.js wrapper

set -e

VERSION="0.1.8"

echo "Building tauri-mcp DXT package v${VERSION} with Node.js wrapper..."

# Ensure we have a release build
echo "Building release binary..."
cargo build --release

# Create clean dxt-package directory
rm -rf dxt-package-node/
mkdir -p dxt-package-node/server

# Copy the binary
echo "Copying binary..."
cp target/release/tauri-mcp dxt-package-node/tauri-mcp
chmod +x dxt-package-node/tauri-mcp

# Copy Node.js server files
echo "Copying Node.js server..."
cp server/package.json dxt-package-node/server/
cp server/index.js dxt-package-node/server/

# Install dependencies
echo "Installing Node.js dependencies..."
cd dxt-package-node/server
npm install --production
cd ../..

# Copy manifest
cp manifest-node.json dxt-package-node/manifest.json

# Create the DXT archive
echo "Creating DXT archive..."
cd dxt-package-node
# Create archive with --no-dir-entries flag to fix Claude Desktop extraction issue
zip -r "../tauri-mcp-node-${VERSION}.dxt" . --no-dir-entries
cd ..

echo "DXT package created: tauri-mcp-node-${VERSION}.dxt"
echo ""
echo "The package contains:"
ls -la dxt-package-node/
echo ""
echo "To test locally:"
echo "  cd dxt-package-node/server && node index.js"