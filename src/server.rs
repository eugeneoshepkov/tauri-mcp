use crate::{Result, TauriMcpError};
use crate::tools::{
    process::ProcessManager,
    window::WindowManager,
    input::InputSimulator,
    debug::DebugTools,
    ipc::IpcManager,
};
use jsonrpc_core::{IoHandler, Params, Value, Error as RpcError};
use jsonrpc_derive::rpc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, error, info};

pub struct TauriMcpServer {
    process_manager: Arc<RwLock<ProcessManager>>,
    window_manager: Arc<WindowManager>,
    input_simulator: Arc<InputSimulator>,
    debug_tools: Arc<DebugTools>,
    ipc_manager: Arc<IpcManager>,
    config: ServerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub auto_discover: bool,
    pub session_management: bool,
    pub event_streaming: bool,
    pub performance_profiling: bool,
    pub network_interception: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            auto_discover: true,
            session_management: true,
            event_streaming: false,
            performance_profiling: false,
            network_interception: false,
        }
    }
}

#[rpc]
pub trait TauriMcp {
    #[rpc(name = "initialize", returns = "Value")]
    fn initialize(&self, protocol_version: String, capabilities: Value) -> jsonrpc_core::Result<Value>;
    
    #[rpc(name = "shutdown", returns = "Value")]
    fn shutdown(&self) -> jsonrpc_core::Result<Value>;
    
    #[rpc(name = "launch_app", returns = "Value")]
    fn launch_app(&self, app_path: String, args: Option<Vec<String>>) -> jsonrpc_core::Result<Value>;
    
    #[rpc(name = "stop_app", returns = "Value")]
    fn stop_app(&self, process_id: String) -> jsonrpc_core::Result<Value>;
    
    #[rpc(name = "get_app_logs", returns = "Value")]
    fn get_app_logs(&self, process_id: String, lines: Option<usize>) -> jsonrpc_core::Result<Value>;
    
    #[rpc(name = "take_screenshot", returns = "Value")]
    fn take_screenshot(&self, process_id: String, output_path: Option<String>) -> jsonrpc_core::Result<Value>;
    
    #[rpc(name = "get_window_info", returns = "Value")]
    fn get_window_info(&self, process_id: String) -> jsonrpc_core::Result<Value>;
    
    #[rpc(name = "send_keyboard_input", returns = "Value")]
    fn send_keyboard_input(&self, process_id: String, keys: String) -> jsonrpc_core::Result<Value>;
    
    #[rpc(name = "send_mouse_click", returns = "Value")]
    fn send_mouse_click(&self, process_id: String, x: i32, y: i32, button: Option<String>) -> jsonrpc_core::Result<Value>;
    
    #[rpc(name = "execute_js", returns = "Value")]
    fn execute_js(&self, process_id: String, javascript_code: String) -> jsonrpc_core::Result<Value>;
    
    #[rpc(name = "get_devtools_info", returns = "Value")]
    fn get_devtools_info(&self, process_id: String) -> jsonrpc_core::Result<Value>;
    
    #[rpc(name = "monitor_resources", returns = "Value")]
    fn monitor_resources(&self, process_id: String) -> jsonrpc_core::Result<Value>;
    
    #[rpc(name = "list_ipc_handlers", returns = "Value")]
    fn list_ipc_handlers(&self, process_id: String) -> jsonrpc_core::Result<Value>;
    
    #[rpc(name = "call_ipc_command", returns = "Value")]
    fn call_ipc_command(&self, process_id: String, command_name: String, args: Option<Value>) -> jsonrpc_core::Result<Value>;
}

impl TauriMcpServer {
    pub async fn new(config_path: PathBuf) -> Result<Self> {
        let config = if config_path.exists() {
            let config_str = tokio::fs::read_to_string(&config_path).await?;
            toml::from_str(&config_str).map_err(|e| TauriMcpError::ConfigError(e.to_string()))?
        } else {
            ServerConfig::default()
        };
        
        debug!("Initializing Tauri MCP server with config: {:?}", config);
        
        Ok(Self {
            process_manager: Arc::new(RwLock::new(ProcessManager::new())),
            window_manager: Arc::new(WindowManager::new()),
            input_simulator: Arc::new(InputSimulator::new()),
            debug_tools: Arc::new(DebugTools::new()),
            ipc_manager: Arc::new(IpcManager::new()),
            config,
        })
    }
    
    pub async fn serve(&self, host: &str, port: u16) -> Result<()> {
        debug!("Starting MCP server on {}:{}", host, port);
        
        let mut io = IoHandler::new();
        
        let server = McpServerImpl {
            process_manager: Arc::clone(&self.process_manager),
            window_manager: Arc::clone(&self.window_manager),
            input_simulator: Arc::clone(&self.input_simulator),
            debug_tools: Arc::clone(&self.debug_tools),
            ipc_manager: Arc::clone(&self.ipc_manager),
        };
        
        io.extend_with(server.to_delegate());
        
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin);
        let mut stdout = stdout;
        
        debug!("MCP server ready, waiting for JSON-RPC requests on stdin");
        
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    info!("EOF reached, shutting down");
                    break;
                }
                Ok(_) => {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    
                    debug!("Received request: {}", line);
                    
                    match io.handle_request(&line).await {
                        Some(response) => {
                            debug!("Sending response: {}", response);
                            stdout.write_all(response.as_bytes()).await?;
                            stdout.write_all(b"\n").await?;
                            stdout.flush().await?;
                        }
                        None => {
                            error!("No response generated for request: {}", line);
                        }
                    }
                }
                Err(e) => {
                    error!("Error reading from stdin: {}", e);
                    break;
                }
            }
        }
        
        Ok(())
    }
}

struct McpServerImpl {
    process_manager: Arc<RwLock<ProcessManager>>,
    window_manager: Arc<WindowManager>,
    input_simulator: Arc<InputSimulator>,
    debug_tools: Arc<DebugTools>,
    ipc_manager: Arc<IpcManager>,
}

impl TauriMcp for McpServerImpl {
    fn initialize(&self, protocol_version: String, capabilities: Value) -> jsonrpc_core::Result<Value> {
        if protocol_version != "1.0" {
            return Err(RpcError::invalid_params(format!("Unsupported protocol version: {}", protocol_version)));
        }
        
        Ok(json!({
            "protocol_version": "1.0",
            "server_info": {
                "name": "tauri-mcp",
                "version": env!("CARGO_PKG_VERSION"),
                "description": "MCP server for testing and interacting with Tauri v2 applications"
            },
            "capabilities": {
                "tools": true,
                "resources": false,
                "prompts": false,
                "logging": true,
                "progress": true
            }
        }))
    }
    
    fn shutdown(&self) -> jsonrpc_core::Result<Value> {
        // Cleanup would happen here
        Ok(json!({
            "status": "shutdown"
        }))
    }
    
    fn launch_app(&self, app_path: String, args: Option<Vec<String>>) -> jsonrpc_core::Result<Value> {
        let process_manager = Arc::clone(&self.process_manager);
        let args = args.unwrap_or_default();
        
        let runtime = tokio::runtime::Handle::current();
        let result = runtime.block_on(async {
            let mut manager = process_manager.write().await;
            manager.launch_app(&app_path, args).await
        });
        
        match result {
            Ok(process_id) => Ok(json!({
                "process_id": process_id,
                "status": "launched"
            })),
            Err(e) => Err(RpcError::invalid_params(e.to_string())),
        }
    }
    
    fn stop_app(&self, process_id: String) -> jsonrpc_core::Result<Value> {
        let process_manager = Arc::clone(&self.process_manager);
        
        let runtime = tokio::runtime::Handle::current();
        let result = runtime.block_on(async {
            let mut manager = process_manager.write().await;
            manager.stop_app(&process_id).await
        });
        
        match result {
            Ok(()) => Ok(json!({
                "status": "stopped"
            })),
            Err(e) => Err(RpcError::invalid_params(e.to_string())),
        }
    }
    
    fn get_app_logs(&self, process_id: String, lines: Option<usize>) -> jsonrpc_core::Result<Value> {
        let process_manager = Arc::clone(&self.process_manager);
        
        let runtime = tokio::runtime::Handle::current();
        let result = runtime.block_on(async {
            let manager = process_manager.read().await;
            manager.get_app_logs(&process_id, lines).await
        });
        
        match result {
            Ok(logs) => Ok(json!({
                "logs": logs
            })),
            Err(e) => Err(RpcError::invalid_params(e.to_string())),
        }
    }
    
    fn take_screenshot(&self, process_id: String, output_path: Option<String>) -> jsonrpc_core::Result<Value> {
        let window_manager = Arc::clone(&self.window_manager);
        let output_path = output_path.map(PathBuf::from);
        
        let runtime = tokio::runtime::Handle::current();
        let result = runtime.block_on(async {
            window_manager.take_screenshot(&process_id, output_path).await
        });
        
        match result {
            Ok(screenshot_data) => Ok(json!({
                "screenshot": screenshot_data
            })),
            Err(e) => Err(RpcError::invalid_params(e.to_string())),
        }
    }
    
    fn get_window_info(&self, process_id: String) -> jsonrpc_core::Result<Value> {
        let window_manager = Arc::clone(&self.window_manager);
        
        let runtime = tokio::runtime::Handle::current();
        let result = runtime.block_on(async {
            window_manager.get_window_info(&process_id).await
        });
        
        match result {
            Ok(info) => Ok(info),
            Err(e) => Err(RpcError::invalid_params(e.to_string())),
        }
    }
    
    fn send_keyboard_input(&self, process_id: String, keys: String) -> jsonrpc_core::Result<Value> {
        let input_simulator = Arc::clone(&self.input_simulator);
        
        let runtime = tokio::runtime::Handle::current();
        let result = runtime.block_on(async {
            input_simulator.send_keyboard_input(&process_id, &keys).await
        });
        
        match result {
            Ok(()) => Ok(json!({
                "status": "sent"
            })),
            Err(e) => Err(RpcError::invalid_params(e.to_string())),
        }
    }
    
    fn send_mouse_click(&self, process_id: String, x: i32, y: i32, button: Option<String>) -> jsonrpc_core::Result<Value> {
        let input_simulator = Arc::clone(&self.input_simulator);
        let button = button.unwrap_or_else(|| "left".to_string());
        
        let runtime = tokio::runtime::Handle::current();
        let result = runtime.block_on(async {
            input_simulator.send_mouse_click(&process_id, x, y, &button).await
        });
        
        match result {
            Ok(()) => Ok(json!({
                "status": "clicked"
            })),
            Err(e) => Err(RpcError::invalid_params(e.to_string())),
        }
    }
    
    fn execute_js(&self, process_id: String, javascript_code: String) -> jsonrpc_core::Result<Value> {
        let debug_tools = Arc::clone(&self.debug_tools);
        
        let runtime = tokio::runtime::Handle::current();
        let result = runtime.block_on(async {
            debug_tools.execute_js(&process_id, &javascript_code).await
        });
        
        match result {
            Ok(result) => Ok(json!({
                "result": result
            })),
            Err(e) => Err(RpcError::invalid_params(e.to_string())),
        }
    }
    
    fn get_devtools_info(&self, process_id: String) -> jsonrpc_core::Result<Value> {
        let debug_tools = Arc::clone(&self.debug_tools);
        
        let runtime = tokio::runtime::Handle::current();
        let result = runtime.block_on(async {
            debug_tools.get_devtools_info(&process_id).await
        });
        
        match result {
            Ok(info) => Ok(info),
            Err(e) => Err(RpcError::invalid_params(e.to_string())),
        }
    }
    
    fn monitor_resources(&self, process_id: String) -> jsonrpc_core::Result<Value> {
        let process_manager = Arc::clone(&self.process_manager);
        
        let runtime = tokio::runtime::Handle::current();
        let result = runtime.block_on(async {
            let manager = process_manager.read().await;
            manager.monitor_resources(&process_id).await
        });
        
        match result {
            Ok(resources) => Ok(resources),
            Err(e) => Err(RpcError::invalid_params(e.to_string())),
        }
    }
    
    fn list_ipc_handlers(&self, process_id: String) -> jsonrpc_core::Result<Value> {
        let ipc_manager = Arc::clone(&self.ipc_manager);
        
        let runtime = tokio::runtime::Handle::current();
        let result = runtime.block_on(async {
            ipc_manager.list_ipc_handlers(&process_id).await
        });
        
        match result {
            Ok(handlers) => Ok(json!({
                "handlers": handlers
            })),
            Err(e) => Err(RpcError::invalid_params(e.to_string())),
        }
    }
    
    fn call_ipc_command(&self, process_id: String, command_name: String, args: Option<Value>) -> jsonrpc_core::Result<Value> {
        let ipc_manager = Arc::clone(&self.ipc_manager);
        let args = args.unwrap_or(Value::Null);
        
        let runtime = tokio::runtime::Handle::current();
        let result = runtime.block_on(async {
            ipc_manager.call_ipc_command(&process_id, &command_name, args).await
        });
        
        match result {
            Ok(result) => Ok(result),
            Err(e) => Err(RpcError::invalid_params(e.to_string())),
        }
    }
}