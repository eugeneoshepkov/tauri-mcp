use tauri_mcp::{server::TauriMcpServer, Result};
use std::path::PathBuf;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn test_server_creation() -> Result<()> {
    let config_path = PathBuf::from("test-config.toml");
    let server = TauriMcpServer::new(config_path).await?;
    Ok(())
}

#[cfg(test)]
mod process_tests {
    use super::*;
    use tauri_mcp::tools::process::ProcessManager;
    
    #[tokio::test]
    #[serial]
    async fn test_process_manager_creation() {
        let manager = ProcessManager::new();
        let processes = manager.get_running_processes();
        assert_eq!(processes.len(), 0);
    }
}

#[cfg(test)]
mod window_tests {
    use super::*;
    use tauri_mcp::tools::window::WindowManager;
    
    #[test]
    fn test_window_manager_creation() {
        let _manager = WindowManager::new();
    }
}

#[cfg(test)]
mod input_tests {
    use super::*;
    use tauri_mcp::tools::input::InputSimulator;
    
    #[test]
    fn test_input_simulator_creation() {
        let _simulator = InputSimulator::new();
    }
}

#[cfg(test)]
mod debug_tests {
    use super::*;
    use tauri_mcp::tools::debug::DebugTools;
    
    #[test]
    fn test_debug_tools_creation() {
        let _tools = DebugTools::new();
    }
}

#[cfg(test)]
mod ipc_tests {
    use super::*;
    use tauri_mcp::tools::ipc::IpcManager;
    
    #[tokio::test]
    #[serial]
    async fn test_ipc_manager_list_handlers() -> Result<()> {
        let manager = IpcManager::new();
        let handlers = manager.list_ipc_handlers("test-process").await?;
        assert!(!handlers.is_empty());
        Ok(())
    }
}