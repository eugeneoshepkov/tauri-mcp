pub mod server;
pub mod tools;
pub mod utils;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum TauriMcpError {
    #[error("Process error: {0}")]
    ProcessError(String),
    
    #[error("Window error: {0}")]
    WindowError(String),
    
    #[error("Screenshot error: {0}")]
    ScreenshotError(String),
    
    #[error("Input simulation error: {0}")]
    InputError(String),
    
    #[error("IPC error: {0}")]
    IpcError(String),
    
    #[error("WebDriver error: {0}")]
    WebDriverError(String),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    
    #[error("Other error: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, TauriMcpError>;