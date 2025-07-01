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
        
        // MCP server ready, waiting for JSON-RPC requests on stdin
        
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
        
        // Support both MCP protocol versions
        if protocol_version != "1.0" && protocol_version != "2024-11-05" {
            return Err(RpcError::invalid_params(format!("Unsupported protocol version: {}", protocol_version)));
        }
        
        // Extract client capabilities if provided
        let _client_capabilities = capabilities;
        
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
                "resources": null,
                "prompts": null,
                "logging": null
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
            _ => Err(RpcError::method_not_found())
        }
    }
}