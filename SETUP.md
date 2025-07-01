# Setup Instructions

The Tauri MCP server is now complete and ready to be pushed to GitHub.

## To push to GitHub:

```bash
# The repository has been initialized and the remote has been added
# You just need to push:
git push -u origin main
```

If you haven't created the repository on GitHub yet:

1. Go to https://github.com/new
2. Create a new repository named `tauri-mcp`
3. Don't initialize with README, .gitignore, or license (we already have these)
4. Then run the push command above

## Building the project:

```bash
# Build in debug mode
cargo build

# Build in release mode
cargo build --release

# Run tests
cargo test

# Install locally
cargo install --path .
```

## Running the server:

```bash
# Run directly
cargo run -- serve

# Or after installation
tauri-mcp serve
```

The server will listen on stdin/stdout for JSON-RPC requests, making it compatible with the MCP protocol.