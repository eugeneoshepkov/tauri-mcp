# Claude Desktop Compatibility Notes

This document explains the compatibility issues between Rust-based MCP servers and Claude Desktop, and how tauri-mcp works around these limitations.

## The Issue

Claude Desktop has known compatibility issues with non-Node.js MCP servers. When attempting to use a Rust binary directly:

1. The server initializes successfully
2. Claude Desktop immediately disconnects
3. Error logs show: "Server disconnected with code 0 and signal null"

This affects all Rust-based MCP servers, not just tauri-mcp. The issue has been reported and tracked at:
- https://github.com/dirvine/tauri-mcp/issues/1
- https://github.com/modelcontextprotocol/servers/issues/885

## The Solution: Node.js Wrapper

To work around this limitation, tauri-mcp provides a Node.js wrapper that:

1. Acts as a bridge between Claude Desktop and the Rust binary
2. Implements the MCP protocol using the official Node.js SDK
3. Spawns the Rust binary in "tool mode" for each tool invocation
4. Returns properly formatted MCP responses

## Architecture

```
Claude Desktop <--> Node.js MCP Server <--> Rust Binary (tool mode)
```

### Node.js Server (`server/index.js`)

- Uses `@modelcontextprotocol/sdk` for MCP protocol implementation
- Handles all MCP communication with Claude Desktop
- Spawns Rust binary with specific tool commands

### Rust Binary Tool Mode

When called with the `tool` subcommand:
```bash
tauri-mcp tool <tool_name> <json_args>
```

The Rust binary:
- Executes the specific tool
- Returns JSON result to stdout
- Exits immediately

## Building the Node.js Wrapper DXT

The `build-dxt-node.sh` script creates a DXT package that includes:

1. The compiled Rust binary
2. Node.js wrapper server
3. All required dependencies
4. Proper manifest configuration

### Key Build Considerations

1. **Directory Extraction Issue**: Use `--no-dir-entries` flag when creating the zip to avoid [DXT extraction issues](https://github.com/anthropics/dxt/issues/18)

2. **Binary Permissions**: Ensure the Rust binary is executable before packaging

3. **Path Resolution**: The Node.js wrapper searches multiple locations for the Rust binary

## Performance Impact

The Node.js wrapper approach has minimal performance impact:

- Initial server startup: ~500ms (Node.js initialization)
- Per-tool invocation: ~50-100ms overhead (process spawn)
- Memory usage: ~30MB for Node.js process

## Future Improvements

Once Claude Desktop adds support for non-Node.js MCP servers:

1. The Rust binary can be used directly
2. The Node.js wrapper will become optional
3. Performance will improve slightly
4. Deployment will be simpler

## Testing Compatibility

To test if Claude Desktop has added Rust support:

1. Try using the cargo-installed version directly:
   ```json
   {
     "mcpServers": {
       "tauri-mcp": {
         "command": "tauri-mcp",
         "args": ["serve"]
       }
     }
   }
   ```

2. Check Claude Desktop logs for connection status

3. If it works, you can use the simpler Rust-only DXT package

Until then, the Node.js wrapper ensures reliable operation with Claude Desktop while maintaining full functionality.