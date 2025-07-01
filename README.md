# Tauri MCP Server

A Model Context Protocol (MCP) server for testing and interacting with Tauri v2 applications. This tool enables AI assistants to better understand, debug, and interact with Tauri apps during development.

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

### From Source

```bash
# Clone the repository
git clone https://github.com/dirvine/tauri-mcp.git
cd tauri-mcp

# Build and install
cargo install --path .
```

### Using Cargo

```bash
cargo install tauri-mcp
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

Add to your Claude Desktop configuration:

```json
{
  "mcpServers": {
    "tauri-mcp": {
      "command": "tauri-mcp",
      "args": ["serve"],
      "env": {
        "TAURI_MCP_LOG_LEVEL": "info"
      }
    }
  }
}
```

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

- Built with the [MCP SDK](https://github.com/anthropics/mcp-sdk)
- Designed for [Tauri v2](https://tauri.app/) applications
- Input simulation powered by [enigo](https://github.com/enigo-rs/enigo)
- Screenshot functionality via [screenshots-rs](https://github.com/nashaofu/screenshots-rs)