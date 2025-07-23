use crate::{Result, TauriMcpError};
use crate::tools::{
    process::ProcessManager,
    window::WindowManager,
    input::InputSimulator,
    debug::DebugTools,
    ipc::IpcManager,
};
use jsonrpc_core::{IoHandler, Params, Value, Error as RpcError};
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
        
        // Register all methods manually to handle MCP's named parameters
        let server_clone = server.clone();
        io.add_method("initialize", move |params: Params| {
            let server = server_clone.clone();
            async move {
                match params {
                    Params::Map(mut map) => {
                        let protocol_version = map.remove("protocolVersion")
                            .and_then(|v| v.as_str().map(String::from))
                            .unwrap_or_else(|| "1.0".to_string());
                        
                        let capabilities = map.remove("capabilities").unwrap_or(Value::Null);
                        
                        server.initialize(protocol_version, capabilities)
                    }
                    _ => Err(RpcError::invalid_params("Expected object parameters"))
                }
            }
        });
        
        // Add initialized notification handler (no response expected)
        let _server_clone = server.clone();
        io.add_notification("notifications/initialized", move |_params: Params| {
            tracing::info!("Received initialized notification from client");
        });
        
        let server_clone = server.clone();
        io.add_method("shutdown", move |_params: Params| {
            let server = server_clone.clone();
            async move { server.shutdown() }
        });
        
        let server_clone = server.clone();
        io.add_method("tools/list", move |_params: Params| {
            let server = server_clone.clone();
            async move { server.list_tools() }
        });
        
        let server_clone = server.clone();
        io.add_method("tools/call", move |params: Params| {
            let server = server_clone.clone();
            async move {
                match params {
                    Params::Map(map) => server.call_tool(Value::Object(map)),
                    _ => Err(RpcError::invalid_params("Expected object parameters"))
                }
            }
        });
        
        // Register all other tool methods
        let tool_methods = vec![
            ("launch_app", "app_path", "args"),
            ("stop_app", "process_id", ""),
            ("get_app_logs", "process_id", "lines"),
            ("take_screenshot", "process_id", "output_path"),
            ("get_window_info", "process_id", ""),
            ("send_keyboard_input", "process_id", "keys"),
            ("send_mouse_click", "process_id", "x,y,button"),
            ("execute_js", "process_id", "javascript_code"),
            ("get_devtools_info", "process_id", ""),
            ("monitor_resources", "process_id", ""),
            ("list_ipc_handlers", "process_id", ""),
            ("call_ipc_command", "process_id", "command_name,args"),
            ("find_running_apps", "", ""),
            ("attach_to_app", "pid", ""),
        ];
        
        for (method_name, _, _) in tool_methods {
            let server_clone = server.clone();
            io.add_method(method_name, move |params: Params| {
                let server = server_clone.clone();
                let method_name = method_name.to_string();
                async move {
                    match params {
                        Params::Map(map) => {
                            server.call_tool(json!({
                                "name": method_name,
                                "arguments": Value::Object(map)
                            }))
                        }
                        _ => Err(RpcError::invalid_params("Expected object parameters"))
                    }
                }
            });
        }
        
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin);
        let mut stdout = stdout;
        
        // Ensure stdout is not buffered for real-time communication
        use std::io::{self, Write};
        let _ = io::stdout().flush();
        
        // MCP server ready, waiting for JSON-RPC requests on stdin
        tracing::info!("MCP server started, waiting for requests on stdin");
        
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    tracing::warn!("EOF reached on stdin, server shutting down");
                    break;
                }
                Ok(n) => {
                    tracing::debug!("Read {} bytes from stdin", n);
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    
                    tracing::info!("Received request: {}", line);
                    
                    match io.handle_request(&line).await {
                        Some(response) => {
                            tracing::info!("Sending response: {}", response);
                            stdout.write_all(response.as_bytes()).await?;
                            stdout.write_all(b"\n").await?;
                            stdout.flush().await?;
                            tracing::debug!("Response sent and flushed");
                        }
                        None => {
                            // Check if this is a notification (no id field means it's a notification)
                            if let Ok(json) = serde_json::from_str::<Value>(&line) {
                                if json.get("id").is_none() && json.get("method").is_some() {
                                    tracing::debug!("Processed notification: {}", json.get("method").unwrap());
                                } else {
                                    tracing::error!("No response generated for request: {}", line);
                                }
                            } else {
                                tracing::error!("Failed to parse JSON request: {}", line);
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Error reading from stdin: {}", e);
                    break;
                }
            }
        }
        
        Ok(())
    }
    
    pub async fn execute_tool(&self, tool_name: &str, args_json: &str) -> Result<Value> {
        let arguments: Value = serde_json::from_str(args_json)
            .map_err(|e| TauriMcpError::Other(format!("Invalid JSON arguments: {}", e)))?;
        
        // Execute tools directly in async context
        match tool_name {
            "launch_app" => {
                let app_path = arguments.get("app_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| TauriMcpError::Other("Missing app_path".to_string()))?
                    .to_string();
                
                let args = arguments.get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                
                let mut manager = self.process_manager.write().await;
                let process_id = manager.launch_app(&app_path, args).await
                    .map_err(|e| TauriMcpError::Other(e.to_string()))?;
                
                Ok(json!({
                    "process_id": process_id,
                    "status": "launched"
                }))
            },
            "stop_app" => {
                let process_id = arguments.get("process_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| TauriMcpError::Other("Missing process_id".to_string()))?
                    .to_string();
                
                let mut manager = self.process_manager.write().await;
                manager.stop_app(&process_id).await
                    .map_err(|e| TauriMcpError::Other(e.to_string()))?;
                
                Ok(json!({
                    "status": "stopped"
                }))
            },
            "get_app_logs" => {
                let process_id = arguments.get("process_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| TauriMcpError::Other("Missing process_id".to_string()))?
                    .to_string();
                
                let lines = arguments.get("lines")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);
                
                let manager = self.process_manager.read().await;
                let logs = manager.get_app_logs(&process_id, lines).await
                    .map_err(|e| TauriMcpError::Other(e.to_string()))?;
                
                Ok(json!({
                    "logs": logs
                }))
            },
            "take_screenshot" => {
                let process_id = arguments.get("process_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| TauriMcpError::Other("Missing process_id".to_string()))?
                    .to_string();
                
                let output_path = arguments.get("output_path")
                    .and_then(|v| v.as_str())
                    .map(|p| PathBuf::from(p));
                
                let screenshot_data = self.window_manager.take_screenshot(&process_id, output_path).await
                    .map_err(|e| TauriMcpError::Other(e.to_string()))?;
                
                Ok(json!({
                    "screenshot": screenshot_data
                }))
            },
            "get_window_info" => {
                let process_id = arguments.get("process_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| TauriMcpError::Other("Missing process_id".to_string()))?
                    .to_string();
                
                let info = self.window_manager.get_window_info(&process_id).await
                    .map_err(|e| TauriMcpError::Other(e.to_string()))?;
                
                Ok(info)
            },
            "send_keyboard_input" => {
                let process_id = arguments.get("process_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| TauriMcpError::Other("Missing process_id".to_string()))?
                    .to_string();
                
                let keys = arguments.get("keys")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| TauriMcpError::Other("Missing keys".to_string()))?
                    .to_string();
                
                self.input_simulator.send_keyboard_input(&process_id, &keys).await
                    .map_err(|e| TauriMcpError::Other(e.to_string()))?;
                
                Ok(json!({
                    "status": "sent"
                }))
            },
            "send_mouse_click" => {
                let process_id = arguments.get("process_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| TauriMcpError::Other("Missing process_id".to_string()))?
                    .to_string();
                
                let x = arguments.get("x")
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| TauriMcpError::Other("Missing x coordinate".to_string()))? as i32;
                
                let y = arguments.get("y")
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| TauriMcpError::Other("Missing y coordinate".to_string()))? as i32;
                
                let button = arguments.get("button")
                    .and_then(|v| v.as_str())
                    .unwrap_or("left")
                    .to_string();
                
                self.input_simulator.send_mouse_click(&process_id, x, y, &button).await
                    .map_err(|e| TauriMcpError::Other(e.to_string()))?;
                
                Ok(json!({
                    "status": "clicked"
                }))
            },
            "execute_js" => {
                let process_id = arguments.get("process_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| TauriMcpError::Other("Missing process_id".to_string()))?
                    .to_string();
                
                let javascript_code = arguments.get("javascript_code")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| TauriMcpError::Other("Missing javascript_code".to_string()))?
                    .to_string();
                
                let result = self.debug_tools.execute_js(&process_id, &javascript_code).await
                    .map_err(|e| TauriMcpError::Other(e.to_string()))?;
                
                Ok(json!({
                    "result": result
                }))
            },
            "get_devtools_info" => {
                let process_id = arguments.get("process_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| TauriMcpError::Other("Missing process_id".to_string()))?
                    .to_string();
                
                let info = self.debug_tools.get_devtools_info(&process_id).await
                    .map_err(|e| TauriMcpError::Other(e.to_string()))?;
                
                Ok(info)
            },
            "monitor_resources" => {
                let process_id = arguments.get("process_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| TauriMcpError::Other("Missing process_id".to_string()))?
                    .to_string();
                
                let manager = self.process_manager.read().await;
                let resources = manager.monitor_resources(&process_id).await
                    .map_err(|e| TauriMcpError::Other(e.to_string()))?;
                
                Ok(resources)
            },
            "list_ipc_handlers" => {
                let process_id = arguments.get("process_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| TauriMcpError::Other("Missing process_id".to_string()))?
                    .to_string();
                
                let handlers = self.ipc_manager.list_ipc_handlers(&process_id).await
                    .map_err(|e| TauriMcpError::Other(e.to_string()))?;
                
                Ok(json!({
                    "handlers": handlers
                }))
            },
            "call_ipc_command" => {
                let process_id = arguments.get("process_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| TauriMcpError::Other("Missing process_id".to_string()))?
                    .to_string();
                
                let command_name = arguments.get("command_name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| TauriMcpError::Other("Missing command_name".to_string()))?
                    .to_string();
                
                let args = arguments.get("args")
                    .cloned()
                    .unwrap_or(Value::Null);
                
                let result = self.ipc_manager.call_ipc_command(&process_id, &command_name, args).await
                    .map_err(|e| TauriMcpError::Other(e.to_string()))?;
                
                Ok(result)
            },
            "find_running_apps" => {
                let manager = self.process_manager.read().await;
                let apps = manager.find_running_apps()
                    .map_err(|e| TauriMcpError::Other(e.to_string()))?;
                
                Ok(json!({
                    "apps": apps
                }))
            },
            "attach_to_app" => {
                let pid = arguments.get("pid")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| TauriMcpError::Other("Missing pid".to_string()))? as u32;
                
                let mut manager = self.process_manager.write().await;
                let process_id = manager.attach_to_app(pid).await
                    .map_err(|e| TauriMcpError::Other(e.to_string()))?;
                
                Ok(json!({
                    "process_id": process_id,
                    "status": "attached"
                }))
            },
            _ => Err(TauriMcpError::Other(format!("Unknown tool: {}", tool_name)))
        }
    }
}

#[derive(Clone)]
struct McpServerImpl {
    process_manager: Arc<RwLock<ProcessManager>>,
    window_manager: Arc<WindowManager>,
    input_simulator: Arc<InputSimulator>,
    debug_tools: Arc<DebugTools>,
    ipc_manager: Arc<IpcManager>,
}

impl McpServerImpl {
    fn initialize(&self, protocol_version: String, capabilities: Value) -> jsonrpc_core::Result<Value> {
        
        // List of supported protocol versions
        const SUPPORTED_VERSIONS: &[&str] = &["1.0", "2024-11-05"];
        
        // Check if the requested version is supported
        let version_supported = SUPPORTED_VERSIONS.contains(&protocol_version.as_str());
        
        // If not supported, try to be backward compatible if it's a date-based version
        // This allows for future protocol versions that follow the YYYY-MM-DD pattern
        let is_date_version = protocol_version.len() == 10 
            && protocol_version.chars().nth(4) == Some('-')
            && protocol_version.chars().nth(7) == Some('-');
        
        if !version_supported && !is_date_version {
            // For truly unsupported versions, return an error with helpful information
            return Err(RpcError::invalid_params(format!(
                "Unsupported protocol version: {}. Supported versions: {:?}. Date-based versions (YYYY-MM-DD) are also accepted.",
                protocol_version, SUPPORTED_VERSIONS
            )));
        }
        
        // Log the protocol version being used
        tracing::info!("MCP client connected with protocol version: {}", protocol_version);
        
        // Extract client capabilities if provided
        let _client_capabilities = capabilities;
        
        // Return the same protocol version the client requested
        // This ensures compatibility with both current and future clients
        Ok(json!({
            "protocolVersion": protocol_version,
            "serverInfo": {
                "name": "tauri-mcp",
                "version": env!("CARGO_PKG_VERSION"),
                "description": "MCP server for testing and interacting with Tauri v2 applications"
            },
            "capabilities": {
                "tools": {
                    "listTools": true
                },
                "resources": {},
                "prompts": {},
                "logging": {}
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
    
    fn find_running_apps(&self) -> jsonrpc_core::Result<Value> {
        let process_manager = Arc::clone(&self.process_manager);
        
        let runtime = tokio::runtime::Handle::current();
        let result = runtime.block_on(async {
            let manager = process_manager.read().await;
            manager.find_running_apps()
        });
        
        match result {
            Ok(apps) => Ok(json!({
                "apps": apps
            })),
            Err(e) => Err(RpcError::invalid_params(e.to_string())),
        }
    }
    
    fn attach_to_app(&self, pid: u32) -> jsonrpc_core::Result<Value> {
        let process_manager = Arc::clone(&self.process_manager);
        
        let runtime = tokio::runtime::Handle::current();
        let result = runtime.block_on(async {
            let mut manager = process_manager.write().await;
            manager.attach_to_app(pid).await
        });
        
        match result {
            Ok(process_id) => Ok(json!({
                "process_id": process_id,
                "status": "attached"
            })),
            Err(e) => Err(RpcError::invalid_params(e.to_string())),
        }
    }
    
    fn list_tools(&self) -> jsonrpc_core::Result<Value> {
        Ok(json!({
            "tools": [
                {
                    "name": "launch_app",
                    "description": "Launch a Tauri application",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "app_path": { "type": "string", "description": "Path to the Tauri application" },
                            "args": { "type": "array", "items": { "type": "string" }, "description": "Optional launch arguments" }
                        },
                        "required": ["app_path"]
                    }
                },
                {
                    "name": "stop_app",
                    "description": "Stop a running Tauri application",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "process_id": { "type": "string", "description": "Process ID of the app to stop" }
                        },
                        "required": ["process_id"]
                    }
                },
                {
                    "name": "get_app_logs",
                    "description": "Get stdout/stderr logs from a running app",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "process_id": { "type": "string", "description": "Process ID of the app" },
                            "lines": { "type": "number", "description": "Number of recent lines to return" }
                        },
                        "required": ["process_id"]
                    }
                },
                {
                    "name": "take_screenshot",
                    "description": "Take a screenshot of the app window",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "process_id": { "type": "string", "description": "Process ID of the app" },
                            "output_path": { "type": "string", "description": "Optional path to save the screenshot" }
                        },
                        "required": ["process_id"]
                    }
                },
                {
                    "name": "get_window_info",
                    "description": "Get window dimensions, position, and state",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "process_id": { "type": "string", "description": "Process ID of the app" }
                        },
                        "required": ["process_id"]
                    }
                },
                {
                    "name": "send_keyboard_input",
                    "description": "Send keyboard input to the app",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "process_id": { "type": "string", "description": "Process ID of the app" },
                            "keys": { "type": "string", "description": "Keys to send" }
                        },
                        "required": ["process_id", "keys"]
                    }
                },
                {
                    "name": "send_mouse_click",
                    "description": "Send mouse click to specific coordinates",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "process_id": { "type": "string", "description": "Process ID of the app" },
                            "x": { "type": "number", "description": "X coordinate" },
                            "y": { "type": "number", "description": "Y coordinate" },
                            "button": { "type": "string", "enum": ["left", "right", "middle"], "description": "Mouse button" }
                        },
                        "required": ["process_id", "x", "y"]
                    }
                },
                {
                    "name": "execute_js",
                    "description": "Execute JavaScript in the app's webview",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "process_id": { "type": "string", "description": "Process ID of the app" },
                            "javascript_code": { "type": "string", "description": "JavaScript code to execute" }
                        },
                        "required": ["process_id", "javascript_code"]
                    }
                },
                {
                    "name": "get_devtools_info",
                    "description": "Get DevTools connection information",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "process_id": { "type": "string", "description": "Process ID of the app" }
                        },
                        "required": ["process_id"]
                    }
                },
                {
                    "name": "monitor_resources",
                    "description": "Monitor CPU, memory, and other resource usage",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "process_id": { "type": "string", "description": "Process ID of the app" }
                        },
                        "required": ["process_id"]
                    }
                },
                {
                    "name": "list_ipc_handlers",
                    "description": "List all registered Tauri IPC commands",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "process_id": { "type": "string", "description": "Process ID of the app" }
                        },
                        "required": ["process_id"]
                    }
                },
                {
                    "name": "call_ipc_command",
                    "description": "Call a Tauri IPC command",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "process_id": { "type": "string", "description": "Process ID of the app" },
                            "command_name": { "type": "string", "description": "Name of the IPC command" },
                            "args": { "type": "object", "description": "Arguments to pass to the command" }
                        },
                        "required": ["process_id", "command_name"]
                    }
                },
                {
                    "name": "find_running_apps",
                    "description": "Find running Tauri applications on the system",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "attach_to_app",
                    "description": "Attach to an already running Tauri application by PID",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "pid": { "type": "number", "description": "Process ID of the running app" }
                        },
                        "required": ["pid"]
                    }
                }
            ]
        }))
    }
    
    fn call_tool(&self, params: Value) -> jsonrpc_core::Result<Value> {
        let tool_name = params.get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RpcError::invalid_params("Missing tool name"))?;
        
        let arguments = params.get("arguments")
            .cloned()
            .unwrap_or(json!({}));
        
        match tool_name {
            "launch_app" => {
                let app_path = arguments.get("app_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| RpcError::invalid_params("Missing app_path"))?
                    .to_string();
                
                let args = arguments.get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());
                
                self.launch_app(app_path, args)
            },
            "stop_app" => {
                let process_id = arguments.get("process_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| RpcError::invalid_params("Missing process_id"))?
                    .to_string();
                
                self.stop_app(process_id)
            },
            "get_app_logs" => {
                let process_id = arguments.get("process_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| RpcError::invalid_params("Missing process_id"))?
                    .to_string();
                
                let lines = arguments.get("lines")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);
                
                self.get_app_logs(process_id, lines)
            },
            "take_screenshot" => {
                let process_id = arguments.get("process_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| RpcError::invalid_params("Missing process_id"))?
                    .to_string();
                
                let output_path = arguments.get("output_path")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                
                self.take_screenshot(process_id, output_path)
            },
            "get_window_info" => {
                let process_id = arguments.get("process_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| RpcError::invalid_params("Missing process_id"))?
                    .to_string();
                
                self.get_window_info(process_id)
            },
            "send_keyboard_input" => {
                let process_id = arguments.get("process_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| RpcError::invalid_params("Missing process_id"))?
                    .to_string();
                
                let keys = arguments.get("keys")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| RpcError::invalid_params("Missing keys"))?
                    .to_string();
                
                self.send_keyboard_input(process_id, keys)
            },
            "send_mouse_click" => {
                let process_id = arguments.get("process_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| RpcError::invalid_params("Missing process_id"))?
                    .to_string();
                
                let x = arguments.get("x")
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| RpcError::invalid_params("Missing x coordinate"))? as i32;
                
                let y = arguments.get("y")
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| RpcError::invalid_params("Missing y coordinate"))? as i32;
                
                let button = arguments.get("button")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                
                self.send_mouse_click(process_id, x, y, button)
            },
            "execute_js" => {
                let process_id = arguments.get("process_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| RpcError::invalid_params("Missing process_id"))?
                    .to_string();
                
                let javascript_code = arguments.get("javascript_code")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| RpcError::invalid_params("Missing javascript_code"))?
                    .to_string();
                
                self.execute_js(process_id, javascript_code)
            },
            "get_devtools_info" => {
                let process_id = arguments.get("process_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| RpcError::invalid_params("Missing process_id"))?
                    .to_string();
                
                self.get_devtools_info(process_id)
            },
            "monitor_resources" => {
                let process_id = arguments.get("process_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| RpcError::invalid_params("Missing process_id"))?
                    .to_string();
                
                self.monitor_resources(process_id)
            },
            "list_ipc_handlers" => {
                let process_id = arguments.get("process_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| RpcError::invalid_params("Missing process_id"))?
                    .to_string();
                
                self.list_ipc_handlers(process_id)
            },
            "call_ipc_command" => {
                let process_id = arguments.get("process_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| RpcError::invalid_params("Missing process_id"))?
                    .to_string();
                
                let command_name = arguments.get("command_name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| RpcError::invalid_params("Missing command_name"))?
                    .to_string();
                
                let args = arguments.get("args").cloned();
                
                self.call_ipc_command(process_id, command_name, args)
            },
            "find_running_apps" => {
                self.find_running_apps()
            },
            "attach_to_app" => {
                let pid = arguments.get("pid")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| RpcError::invalid_params("Missing pid"))? as u32;
                
                self.attach_to_app(pid)
            },
            _ => Err(RpcError::method_not_found())
        }
    }
}