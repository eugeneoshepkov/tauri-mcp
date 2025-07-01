# Tauri MCP Server

A [Model Context Protocol (MCP)](https://modelcontextprotocol.io/introduction) server for testing and interacting with Tauri v2 applications. This tool enables AI assistants to better understand, debug, and interact with Tauri apps during development.

The MCP protocol provides a standardized way for AI assistants to interact with external tools and systems. This server implements the MCP specification to expose Tauri application testing and debugging capabilities.

## MCP Compliance

This server is fully compliant with the [Model Context Protocol specification](https://modelcontextprotocol.io/introduction). It implements:

- ✅ **JSON-RPC 2.0** transport over stdio
- ✅ **Initialize/shutdown** handshake
- ✅ **Tools** capability with 12 specialized Tauri testing tools
- ✅ **Proper error handling** with descriptive messages
- ✅ **Tool schemas** using JSON Schema format
- ✅ **Protocol version compatibility** - Supports both "1.0" and date-based versions (e.g., "2024-11-05")

For more details about MCP:
- [MCP Introduction](https://modelcontextprotocol.io/introduction)
- [Building MCP Servers](https://modelcontextprotocol.io/docs/concepts/servers)
- [Tool Definitions](https://modelcontextprotocol.io/docs/concepts/tools)

## Features

### Core Tools

- **Process Management**
  - `launch_app` - Launch Tauri applications with arguments
  - `stop_app` - Gracefully stop running apps
  - `get_app_logs` - Capture stdout/stderr output
  - `monitor_resources` - Track CPU, memory, and disk usage

- **Window Manipulation**
  - `take_screenshot` - Capture app window screenshots
  - `get_window_info` - Get window dimensions, position, and state

- **Input Simulation**
  - `send_keyboard_input` - Simulate keyboard input
  - `send_mouse_click` - Simulate mouse clicks
  - Mouse movement, dragging, and scrolling support

- **Debugging Tools**
  - `execute_js` - Execute JavaScript in the webview
  - `get_devtools_info` - Get DevTools connection info
  - WebDriver integration for advanced testing
  - Console log capture

- **IPC Interaction**
  - `list_ipc_handlers` - List registered Tauri commands
  - `call_ipc_command` - Call Tauri IPC commands
  - Event emission and listening

## Installation

### Using DXT (Recommended for Claude Desktop)

The easiest way to install tauri-mcp for use with Claude Desktop is via DXT (Desktop Extension):

1. Download the latest `tauri-mcp-node-*.dxt` file from the [releases page](https://github.com/dirvine/tauri-mcp/releases)
2. Double-click the `.dxt` file to install (or drag it onto Claude Desktop)
3. The server will be automatically configured and ready to use

**Note**: Due to a [known issue](https://github.com/anthropics/dxt/issues/18) with Claude Desktop's DXT extraction, we provide a Node.js wrapper version (`tauri-mcp-node-*.dxt`) that works around this limitation.

### Using Cargo

```bash
cargo install tauri-mcp
```

**Important Note for Rust-based MCP Servers**: Claude Desktop currently has [compatibility issues](https://github.com/dirvine/tauri-mcp/issues/1) with Rust-based MCP servers, causing immediate disconnections after initialization. This is a known issue affecting all non-Node.js MCP servers. The cargo installation is suitable for:
- Direct CLI usage
- Integration with other tools
- Development and testing

For use with Claude Desktop, please use the DXT package which includes a Node.js wrapper.

### From Source

```bash
# Clone the repository
git clone https://github.com/dirvine/tauri-mcp.git
cd tauri-mcp

# Build and install
cargo install --path .
```

## Usage

### As a Standalone Server

```bash
# Start the MCP server
tauri-mcp serve

# With custom host and port
tauri-mcp serve --host 127.0.0.1 --port 3000

# With a specific Tauri app
tauri-mcp --app-path ./my-tauri-app
```

### Configuration

Create a `tauri-mcp.toml` file for configuration:

```toml
auto_discover = true
session_management = true
event_streaming = false
performance_profiling = false
network_interception = false
```

### Environment Variables

- `TAURI_MCP_LOG_LEVEL` - Set log level (trace, debug, info, warn, error)
- `TAURI_MCP_CONFIG` - Path to config file (default: tauri-mcp.toml)

## MCP Integration

### Claude Desktop Configuration

#### Option 1: Using DXT Package (Recommended)

Install the `tauri-mcp-node-*.dxt` package by double-clicking it. The server will be automatically configured.

#### Option 2: Manual Configuration

If you need to configure manually or use the development version, add to your Claude Desktop configuration:

```json
{
  "mcpServers": {
    "tauri-mcp": {
      "command": "node",
      "args": ["/path/to/tauri-mcp/server/index.js"],
      "env": {
        "TAURI_MCP_LOG_LEVEL": "info"
      }
    }
  }
}
```

**Note**: Direct Rust binary configuration (`"command": "tauri-mcp"`) will not work with Claude Desktop due to compatibility issues. Use the Node.js wrapper approach shown above.

### Available MCP Tools

All tools are exposed through the MCP protocol and can be called by AI assistants:

```javascript
// Launch a Tauri app
await use_mcp_tool("tauri-mcp", "launch_app", {
  app_path: "/path/to/tauri-app",
  args: ["--debug"]
});

// Take a screenshot
await use_mcp_tool("tauri-mcp", "take_screenshot", {
  process_id: "uuid-here",
  output_path: "./screenshot.png"
});

// Execute JavaScript
await use_mcp_tool("tauri-mcp", "execute_js", {
  process_id: "uuid-here",
  javascript_code: "window.location.href"
});

// Send keyboard input
await use_mcp_tool("tauri-mcp", "send_keyboard_input", {
  process_id: "uuid-here",
  keys: "cmd+a"
});
```

## Platform Support

- **macOS** - Full support including window management
- **Windows** - Full support with native window APIs
- **Linux** - X11 support (Wayland in progress)

## Development

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test
```

### Building DXT Packages

The project includes build scripts for creating DXT (Desktop Extension) packages for different platforms.

#### Prerequisites

- Rust toolchain installed
- Node.js 18+ (for the Node.js wrapper)
- `zip` command available in PATH

#### Building on macOS/Linux

```bash
# Build the Node.js wrapper version (recommended)
./build-dxt-node.sh

# The DXT package will be created as tauri-mcp-node-*.dxt
```

#### Building on Windows

```powershell
# Create a Windows build script or use WSL
# Ensure you have zip.exe available or use PowerShell's Compress-Archive

# Build release binary
cargo build --release

# Create package directory structure
mkdir dxt-package-node
mkdir dxt-package-node\server

# Copy files
copy target\release\tauri-mcp.exe dxt-package-node\
copy server\*.* dxt-package-node\server\
copy manifest-node.json dxt-package-node\manifest.json

# Install Node dependencies
cd dxt-package-node\server
npm install --production
cd ..\..

# Create DXT archive (using PowerShell)
Compress-Archive -Path dxt-package-node\* -DestinationPath tauri-mcp-node.zip
Rename-Item tauri-mcp-node.zip tauri-mcp-node-0.1.8.dxt
```

#### DXT Package Structure

The DXT package includes:
- `manifest.json` - Extension metadata and configuration
- `tauri-mcp` - The compiled Rust binary
- `server/` - Node.js wrapper and dependencies
  - `index.js` - Node.js MCP server that spawns the Rust binary
  - `package.json` - Node.js dependencies
  - `node_modules/` - MCP SDK and dependencies

#### Important Notes

1. **Use `--no-dir-entries` flag**: When creating the zip file, use the `--no-dir-entries` flag to avoid [extraction issues](https://github.com/anthropics/dxt/issues/18):
   ```bash
   zip -r package.dxt . --no-dir-entries
   ```

2. **Binary permissions**: Ensure the Rust binary has executable permissions before packaging:
   ```bash
   chmod +x tauri-mcp
   ```

3. **Cross-platform builds**: For distributing to other platforms, you'll need to build the Rust binary on each target platform or use cross-compilation.

### Project Structure

```
tauri-mcp/
├── src/
│   ├── main.rs          # Entry point
│   ├── server.rs        # MCP server implementation
│   ├── tools/           # Tool implementations
│   │   ├── process.rs   # Process management
│   │   ├── window.rs    # Window manipulation
│   │   ├── input.rs     # Input simulation
│   │   ├── debug.rs     # Debugging tools
│   │   └── ipc.rs       # IPC interaction
│   └── utils/           # Utility modules
├── examples/            # Example Tauri apps
└── tests/              # Integration tests
```

## Examples

### Testing a Tauri App

```bash
# Launch the server
tauri-mcp serve

# In your AI assistant:
# 1. Launch the app
# 2. Take a screenshot
# 3. Send some input
# 4. Check the logs
# 5. Stop the app
```

### Automated Testing Script

```python
import asyncio
from mcp import Client

async def test_tauri_app():
    client = Client("tauri-mcp")
    
    # Launch app
    result = await client.call_tool("launch_app", {
        "app_path": "./my-app",
        "args": ["--test-mode"]
    })
    process_id = result["process_id"]
    
    # Wait for app to start
    await asyncio.sleep(2)
    
    # Take screenshot
    await client.call_tool("take_screenshot", {
        "process_id": process_id,
        "output_path": "./test-screenshot.png"
    })
    
    # Send input
    await client.call_tool("send_keyboard_input", {
        "process_id": process_id,
        "keys": "Hello, Tauri!"
    })
    
    # Get logs
    logs = await client.call_tool("get_app_logs", {
        "process_id": process_id,
        "lines": 50
    })
    print("App logs:", logs)
    
    # Stop app
    await client.call_tool("stop_app", {
        "process_id": process_id
    })

asyncio.run(test_tauri_app())
```

## Troubleshooting

### Common Issues

1. **Permission Denied on macOS**
   - Grant accessibility permissions in System Preferences
   - Required for input simulation

2. **Screenshot Fails**
   - Ensure screen recording permissions are granted
   - Check if the app window is visible

3. **WebDriver Connection Failed**
   - Ensure the Tauri app has DevTools enabled
   - Check if ChromeDriver is installed for WebDriver support

### Debug Mode

Enable debug logging:

```bash
TAURI_MCP_LOG_LEVEL=debug tauri-mcp serve
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- Implements the [Model Context Protocol](https://modelcontextprotocol.io/) specification
- Designed for [Tauri v2](https://tauri.app/) applications
- Input simulation powered by [enigo](https://github.com/enigo-rs/enigo)
- Screenshot functionality via [screenshots-rs](https://github.com/nashaofu/screenshots-rs)