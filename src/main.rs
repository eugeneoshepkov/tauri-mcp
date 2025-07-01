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
        .compact()
        .init();
    
    println!("\n🚀 Tauri MCP Server v{}", env!("CARGO_PKG_VERSION"));
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    let config_exists = args.config.exists();
    let server = TauriMcpServer::new(args.config).await?;
    
    match args.command {
        Some(Command::Serve { host, port }) => {
            println!("\n📡 Starting MCP server...");
            println!("   Mode: JSON-RPC over stdio");
            println!("   Config: {}", if config_exists { "loaded" } else { "using defaults" });
            println!("\n🔧 Available Tools:");
            println!("   • launch_app          - Launch Tauri applications");
            println!("   • stop_app            - Stop running apps");
            println!("   • take_screenshot     - Capture app windows");
            println!("   • send_keyboard_input - Simulate keyboard input");
            println!("   • send_mouse_click    - Simulate mouse clicks");
            println!("   • execute_js          - Run JavaScript in webview");
            println!("   • monitor_resources   - Track CPU/memory usage");
            println!("   • ... and 5 more tools");
            println!("\n📖 Usage:");
            println!("   This server communicates via JSON-RPC on stdin/stdout.");
            println!("   It's designed to be used by AI assistants like Claude.");
            println!("\n   To use with Claude Desktop, add to your config:");
            println!("   {{");
            println!("     \"mcpServers\": {{");
            println!("       \"tauri-mcp\": {{");
            println!("         \"command\": \"tauri-mcp\",");
            println!("         \"args\": [\"serve\"]");
            println!("       }}");
            println!("     }}");
            println!("   }}");
            println!("\n✅ Server ready! Waiting for JSON-RPC requests...");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
            
            server.serve(&host, port).await?;
        }
        None => {
            if let Some(app_path) = args.app_path {
                println!("\n📡 Starting MCP server with app: {:?}", app_path);
            } else {
                println!("\n📡 Starting MCP server in default mode");
            }
            println!("   Mode: JSON-RPC over stdio");
            println!("\n💡 Tip: Run 'tauri-mcp --help' for usage information");
            println!("\n✅ Server ready! Waiting for JSON-RPC requests...");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
            
            server.serve("127.0.0.1", 3000).await?;
        }
    }
    
    Ok(())
}