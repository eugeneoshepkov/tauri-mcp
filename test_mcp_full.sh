#\!/bin/bash
echo "Testing tauri-mcp server..." >&2

# Send initialize request
echo '{"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}},"jsonrpc":"2.0","id":1}'

# Wait for response
sleep 0.1

# Send initialized notification
echo '{"method":"initialized","params":{},"jsonrpc":"2.0"}'

# Wait a bit
sleep 0.1

# Send tools/list request
echo '{"method":"tools/list","params":{},"jsonrpc":"2.0","id":2}'

# Keep connection open for a bit
sleep 1

echo "Test complete" >&2
