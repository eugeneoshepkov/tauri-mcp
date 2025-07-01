use clap::Parser;
use std::path::PathBuf;
use tauri_mcp::{server::TauriMcpServer, Result};
use tracing::{info, Level};
use tracing_subscriber::{self, EnvFilter};

#[derive(Parser, Debug)]
#[command(name = "tauri-mcp")]
#[command(about = "MCP server for testing and interacting with Tauri v2 applications")]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
    
    #[arg(long, value_name = "PATH")]
    app_path: Option<PathBuf>,
    
    #[arg(long, value_name = "FILE", default_value = "tauri-mcp.toml")]
    config: PathBuf,
    
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[derive(clap::Subcommand, Debug)]
enum Command {
    Serve {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        
        #[arg(long, default_value = "3000")]
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
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();
    
    info!("Starting Tauri MCP server");
    
    let server = TauriMcpServer::new(args.config).await?;
    
    match args.command {
        Some(Command::Serve { host, port }) => {
            info!("Starting MCP server on {}:{}", host, port);
            server.serve(&host, port).await?;
        }
        None => {
            if let Some(app_path) = args.app_path {
                info!("Running with app path: {:?}", app_path);
                server.serve("127.0.0.1", 3000).await?;
            } else {
                info!("Starting MCP server in default mode");
                server.serve("127.0.0.1", 3000).await?;
            }
        }
    }
    
    Ok(())
}