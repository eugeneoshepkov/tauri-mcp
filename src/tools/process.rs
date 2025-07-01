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
    child: Child,
    pid: u32,
    log_receiver: Receiver<String>,
    log_handle: JoinHandle<()>,
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
            child,
            pid,
            log_receiver,
            log_handle,
        };
        
        self.processes.insert(process_id.clone(), process_info);
        
        info!("App launched successfully with process ID: {} (PID: {})", process_id, pid);
        
        Ok(process_id)
    }
    
    pub async fn stop_app(&mut self, process_id: &str) -> Result<()> {
        let mut process_info = self.processes.remove(process_id)
            .ok_or_else(|| TauriMcpError::ProcessError(format!("Process not found: {}", process_id)))?;
        
        info!("Stopping app with process ID: {}", process_id);
        
        process_info.child.kill().await
            .map_err(|e| TauriMcpError::ProcessError(format!("Failed to kill process: {}", e)))?;
        
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
}