#!/bin/bash

# Build DXT package for tauri-mcp with proper structure

set -e

VERSION="0.1.5"

echo "Building tauri-mcp DXT package v${VERSION}..."

# Ensure we have a release build
echo "Building release binary..."
cargo build --release

# Create clean dxt-package directory
rm -rf dxt-package/
mkdir -p dxt-package/

# Copy the binary
echo "Copying binary..."
cp target/release/tauri-mcp dxt-package/
chmod +x dxt-package/tauri-mcp

# Create manifest.json
echo "Creating manifest..."
cat > dxt-package/manifest.json << EOF
{
  "dxt_version": "0.1",
  "name": "tauri-mcp",
  "version": "${VERSION}",
  "description": "MCP server for testing and interacting with Tauri v2 applications",
  "author": {
    "name": "David Irvine",
    "email": "david.irvine@maidsafe.net"
  },
  "server": {
    "type": "binary",
    "entry_point": "tauri-mcp",
    "mcp_config": {
      "command": "\${__dirname}/tauri-mcp",
      "args": [],
      "env": {
        "TAURI_MCP_LOG_LEVEL": "info"
      }
    }
  },
  "license": "MIT OR Apache-2.0"
}
EOF

# Create the DXT archive
echo "Creating DXT archive..."
cd dxt-package
zip -r "../tauri-mcp-${VERSION}.dxt" *
cd ..

echo "DXT package created: tauri-mcp-${VERSION}.dxt"
echo ""
echo "The package contains:"
ls -la dxt-package/
echo ""
echo "To test the binary:"
echo "  ./dxt-package/tauri-mcp --help"