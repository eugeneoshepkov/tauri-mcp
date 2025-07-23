use crate::{Result, TauriMcpError};
use crossbeam_channel::{bounded, Receiver, Sender};
use parking_lot::RwLock;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use sysinfo::{System, Pid};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

pub struct ProcessManager {
    processes: HashMap<String, ProcessInfo>,
    system: Arc<RwLock<System>>,
}

struct ProcessInfo {
    id: String,
    child: Option<Child>,
    pid: u32,
    log_receiver: Receiver<String>,
    log_handle: JoinHandle<()>,
    is_attached: bool,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
            system: Arc::new(RwLock::new(System::new_all())),
        }
    }
    
    pub async fn launch_app(&mut self, app_path: &str, args: Vec<String>) -> Result<String> {
        let path = Path::new(app_path);
        if !path.exists() {
            return Err(TauriMcpError::ProcessError(format!("App path does not exist: {}", app_path)));
        }
        
        info!("Launching Tauri app: {} with args: {:?}", app_path, args);
        
        let mut cmd = Command::new(app_path);
        cmd.args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());
        
        let mut child = cmd.spawn()
            .map_err(|e| TauriMcpError::ProcessError(format!("Failed to launch app: {}", e)))?;
        
        let pid = child.id()
            .ok_or_else(|| TauriMcpError::ProcessError("Failed to get process ID".to_string()))?;
        
        let process_id = Uuid::new_v4().to_string();
        
        let (log_sender, log_receiver) = bounded(1000);
        
        let stdout = child.stdout.take()
            .ok_or_else(|| TauriMcpError::ProcessError("Failed to capture stdout".to_string()))?;
        let stderr = child.stderr.take()
            .ok_or_else(|| TauriMcpError::ProcessError("Failed to capture stderr".to_string()))?;
        
        let log_handle = tokio::spawn(Self::log_reader(stdout, stderr, log_sender));
        
        let process_info = ProcessInfo {
            id: process_id.clone(),
            child: Some(child),
            pid,
            log_receiver,
            log_handle,
            is_attached: false,
        };
        
        self.processes.insert(process_id.clone(), process_info);
        
        info!("App launched successfully with process ID: {} (PID: {})", process_id, pid);
        
        Ok(process_id)
    }
    
    pub async fn stop_app(&mut self, process_id: &str) -> Result<()> {
        let mut process_info = self.processes.remove(process_id)
            .ok_or_else(|| TauriMcpError::ProcessError(format!("Process not found: {}", process_id)))?;
        
        info!("Stopping app with process ID: {}", process_id);
        
        if let Some(mut child) = process_info.child {
            child.kill().await
                .map_err(|e| TauriMcpError::ProcessError(format!("Failed to kill process: {}", e)))?;
        } else if process_info.is_attached {
            // For attached processes, we can't kill them directly
            warn!("Cannot stop attached process {}, it was not launched by us", process_id);
            return Err(TauriMcpError::ProcessError("Cannot stop externally launched process".to_string()));
        }
        
        process_info.log_handle.abort();
        
        Ok(())
    }
    
    pub async fn get_app_logs(&self, process_id: &str, lines: Option<usize>) -> Result<Vec<String>> {
        let process_info = self.processes.get(process_id)
            .ok_or_else(|| TauriMcpError::ProcessError(format!("Process not found: {}", process_id)))?;
        
        let mut logs = Vec::new();
        
        while let Ok(log) = process_info.log_receiver.try_recv() {
            logs.push(log);
        }
        
        if let Some(line_count) = lines {
            let start = logs.len().saturating_sub(line_count);
            logs = logs[start..].to_vec();
        }
        
        Ok(logs)
    }
    
    pub async fn monitor_resources(&self, process_id: &str) -> Result<Value> {
        let process_info = self.processes.get(process_id)
            .ok_or_else(|| TauriMcpError::ProcessError(format!("Process not found: {}", process_id)))?;
        
        let mut system = self.system.write();
        system.refresh_processes();
        
        if let Some(process) = system.process(Pid::from_u32(process_info.pid)) {
            Ok(serde_json::json!({
                "cpu_usage": process.cpu_usage(),
                "memory_usage": process.memory(),
                "virtual_memory": process.virtual_memory(),
                "disk_usage": {
                    "read_bytes": process.disk_usage().read_bytes,
                    "written_bytes": process.disk_usage().written_bytes,
                },
                "status": format!("{:?}", process.status()),
                "start_time": process.start_time(),
                "run_time": process.run_time(),
            }))
        } else {
            Err(TauriMcpError::ProcessError("Failed to get process info".to_string()))
        }
    }
    
    async fn log_reader(
        stdout: tokio::process::ChildStdout,
        stderr: tokio::process::ChildStderr,
        sender: Sender<String>,
    ) {
        let stdout_reader = BufReader::new(stdout);
        let stderr_reader = BufReader::new(stderr);
        
        let stdout_sender = sender.clone();
        let stderr_sender = sender;
        
        let stdout_handle = tokio::spawn(async move {
            let mut lines = stdout_reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let log_line = format!("[stdout] {}", line);
                if stdout_sender.send(log_line).is_err() {
                    break;
                }
            }
        });
        
        let stderr_handle = tokio::spawn(async move {
            let mut lines = stderr_reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let log_line = format!("[stderr] {}", line);
                if stderr_sender.send(log_line).is_err() {
                    break;
                }
            }
        });
        
        let _ = tokio::join!(stdout_handle, stderr_handle);
    }
    
    pub fn get_running_processes(&self) -> Vec<String> {
        self.processes.keys().cloned().collect()
    }
    
    pub fn find_running_apps(&self) -> Result<Vec<Value>> {
        let mut system = self.system.write();
        system.refresh_processes();
        
        let mut tauri_apps = Vec::new();
        
        for (pid, process) in system.processes() {
            let name = process.name();
            let cmd = process.cmd();
            
            // Look for processes that might be Tauri apps
            if name.contains("archestra") || 
               cmd.iter().any(|arg| arg.contains("tauri") || arg.contains("archestra")) {
                tauri_apps.push(serde_json::json!({
                    "pid": pid.as_u32(),
                    "name": name,
                    "cmd": cmd.join(" "),
                    "memory": process.memory(),
                    "cpu_usage": process.cpu_usage(),
                    "status": format!("{:?}", process.status()),
                }));
            }
        }
        
        Ok(tauri_apps)
    }
    
    pub async fn attach_to_app(&mut self, pid: u32) -> Result<String> {
        let mut system = self.system.write();
        system.refresh_processes();
        
        if let Some(_process) = system.process(Pid::from_u32(pid)) {
            let process_id = Uuid::new_v4().to_string();
            
            info!("Attaching to existing process with PID: {}", pid);
            
            // Create a dummy child process info for tracking
            // Note: We won't have stdout/stderr for already running processes
            let (_log_sender, log_receiver) = bounded(1000);
            
            // Create a dummy log handle that does nothing
            let log_handle = tokio::spawn(async move {
                // This task does nothing as we can't capture logs from external processes
                tokio::time::sleep(tokio::time::Duration::from_secs(u64::MAX)).await;
            });
            
            let process_info = ProcessInfo {
                id: process_id.clone(),
                child: None,
                pid,
                log_receiver,
                log_handle,
                is_attached: true,
            };
            
            self.processes.insert(process_id.clone(), process_info);
            
            info!("Successfully attached to process with PID: {}", pid);
            
            Ok(process_id)
        } else {
            Err(TauriMcpError::ProcessError(format!("Process with PID {} not found", pid)))
        }
    }
}