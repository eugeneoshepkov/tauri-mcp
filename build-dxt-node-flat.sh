#!/bin/bash

# Build DXT package for tauri-mcp with Node.js wrapper (flat structure)

set -e

VERSION="0.1.8"

echo "Building tauri-mcp DXT package v${VERSION} with Node.js wrapper (flat structure)..."

# Ensure we have a release build
echo "Building release binary..."
cargo build --release

# Create clean dxt-package directory
rm -rf dxt-package-flat/
mkdir -p dxt-package-flat/

# Copy the binary
echo "Copying binary..."
cp target/release/tauri-mcp dxt-package-flat/tauri-mcp
chmod +x dxt-package-flat/tauri-mcp

# Copy Node.js files to root
echo "Copying Node.js files..."
cp server/index.js dxt-package-flat/
cp server/package.json dxt-package-flat/

# Install dependencies
echo "Installing Node.js dependencies..."
cd dxt-package-flat/
npm install --production
cd ..

# Copy manifest
cp manifest-node-flat.json dxt-package-flat/manifest.json

# Create the DXT archive
echo "Creating DXT archive..."
cd dxt-package-flat
zip -r "../tauri-mcp-node-${VERSION}.dxt" .
cd ..

echo "DXT package created: tauri-mcp-node-${VERSION}.dxt"
echo ""
echo "The package contains:"
ls -la dxt-package-flat/