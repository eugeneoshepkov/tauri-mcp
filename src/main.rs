use clap::Parser;
use std::path::PathBuf;
use tauri_mcp::{server::TauriMcpServer, Result};
use tracing::Level;
use tracing_subscriber::{self, EnvFilter};

#[derive(Parser, Debug)]
#[command(name = "tauri-mcp")]
#[command(version, about, long_about = None)]
#[command(about = "MCP server for testing and interacting with Tauri v2 applications")]
#[command(long_about = "
Tauri MCP Server - Model Context Protocol server for Tauri v2 apps

This server provides AI assistants with tools to interact with Tauri applications,
including process management, window manipulation, input simulation, and debugging.

EXAMPLES:
    # Start the MCP server (for use with AI assistants)
    tauri-mcp serve
    
    # Use with Claude Desktop by adding to config:
    {
      \"mcpServers\": {
        \"tauri-mcp\": {
          \"command\": \"tauri-mcp\",
          \"args\": [\"serve\"]
        }
      }
    }
    
    # With custom config file
    tauri-mcp --config my-config.toml serve
    
    # With debug logging
    tauri-mcp --log-level debug serve

AVAILABLE TOOLS:
    • launch_app       - Launch a Tauri application
    • stop_app         - Stop a running app
    • get_app_logs     - Get stdout/stderr logs
    • take_screenshot  - Capture app window
    • get_window_info  - Get window dimensions and state
    • send_keyboard_input - Send keyboard input
    • send_mouse_click - Send mouse clicks
    • execute_js       - Execute JavaScript in webview
    • get_devtools_info - Get DevTools connection info
    • monitor_resources - Monitor CPU/memory usage
    • list_ipc_handlers - List Tauri IPC commands
    • call_ipc_command - Call Tauri IPC commands
")]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
    
    #[arg(long, value_name = "PATH", help = "Path to a Tauri application to launch on startup")]
    app_path: Option<PathBuf>,
    
    #[arg(long, value_name = "FILE", default_value = "tauri-mcp.toml", help = "Configuration file path")]
    config: PathBuf,
    
    #[arg(long, default_value = "info", help = "Log level (trace, debug, info, warn, error)")]
    log_level: String,
}

#[derive(clap::Subcommand, Debug)]
enum Command {
    #[command(about = "Start the MCP server")]
    Serve {
        #[arg(long, default_value = "127.0.0.1", help = "Host to bind to")]
        host: String,
        
        #[arg(long, default_value = "3000", help = "Port to bind to")]
        port: u16,
    },
    #[command(about = "Execute a specific tool (for Node.js wrapper)")]
    Tool {
        #[arg(help = "Tool name to execute")]
        name: String,
        
        #[arg(help = "JSON arguments for the tool")]
        args: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    let log_level = match args.log_level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };
    
    let filter = EnvFilter::new(format!("tauri_mcp={},mcp={}", log_level, log_level));
    
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .with_writer(std::io::stderr)
        .compact()
        .init();
    
    let server = TauriMcpServer::new(args.config).await?;
    
    match args.command {
        Some(Command::Serve { host, port }) => {
            // In serve mode, don't print anything to stdout - it's used for JSON-RPC
            server.serve(&host, port).await?;
        }
        Some(Command::Tool { name, args }) => {
            // Tool mode - execute a specific tool and return JSON result
            let result = server.execute_tool(&name, &args).await?;
            // Print JSON result to stdout for Node.js wrapper
            println!("{}", serde_json::to_string(&result)?);
        }
        None => {
            // Default to serve mode without printing anything
            server.serve("127.0.0.1", 3000).await?;
        }
    }
    
    Ok(())
}