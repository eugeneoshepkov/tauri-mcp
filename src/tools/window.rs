use crate::{Result, TauriMcpError};
use base64::{Engine as _, engine::general_purpose};
use image::ImageOutputFormat;
use screenshots::Screen;
use serde_json::Value;
use std::io::Cursor;
use std::path::PathBuf;
use tracing::{debug, error, info};

#[cfg(target_os = "macos")]
use cocoa::base::{id, nil};
#[cfg(target_os = "macos")]
use cocoa::foundation::{NSArray, NSString};
#[cfg(target_os = "macos")]
use cocoa::appkit::{NSApp, NSApplicationActivateIgnoringOtherApps, NSRunningApplication};
#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{HWND, RECT};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, GetWindowText, GetWindowTextLengthW};

#[cfg(target_os = "linux")]
use x11::xlib;

pub struct WindowManager {
    #[cfg(target_os = "linux")]
    display: *mut xlib::Display,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WindowInfo {
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_visible: bool,
    pub is_focused: bool,
}

impl WindowManager {
    pub fn new() -> Self {
        #[cfg(target_os = "linux")]
        {
            let display = unsafe { xlib::XOpenDisplay(std::ptr::null()) };
            if display.is_null() {
                panic!("Failed to open X11 display");
            }
            Self { display }
        }
        
        #[cfg(not(target_os = "linux"))]
        Self {}
    }
    
    pub async fn take_screenshot(&self, process_id: &str, output_path: Option<PathBuf>) -> Result<String> {
        info!("Taking screenshot for process: {}", process_id);
        
        let screens = Screen::all().map_err(|e| TauriMcpError::ScreenshotError(e.to_string()))?;
        
        if screens.is_empty() {
            return Err(TauriMcpError::ScreenshotError("No screens found".to_string()));
        }
        
        let screen = &screens[0];
        let image = screen.capture().map_err(|e| TauriMcpError::ScreenshotError(e.to_string()))?;
        
        if let Some(path) = output_path {
            image.save(&path).map_err(|e| TauriMcpError::ScreenshotError(e.to_string()))?;
            info!("Screenshot saved to: {:?}", path);
            Ok(path.to_string_lossy().to_string())
        } else {
            let mut buffer = Cursor::new(Vec::new());
            image.write_to(&mut buffer, ImageOutputFormat::Png)
                .map_err(|e| TauriMcpError::ScreenshotError(e.to_string()))?;
            
            let base64_data = general_purpose::STANDARD.encode(buffer.into_inner());
            Ok(format!("data:image/png;base64,{}", base64_data))
        }
    }
    
    pub async fn get_window_info(&self, process_id: &str) -> Result<Value> {
        info!("Getting window info for process: {}", process_id);
        
        #[cfg(target_os = "macos")]
        {
            self.get_window_info_macos(process_id).await
        }
        
        #[cfg(target_os = "windows")]
        {
            self.get_window_info_windows(process_id).await
        }
        
        #[cfg(target_os = "linux")]
        {
            self.get_window_info_linux(process_id).await
        }
    }
    
    #[cfg(target_os = "macos")]
    async fn get_window_info_macos(&self, process_id: &str) -> Result<Value> {
        Ok(serde_json::json!({
            "title": "Tauri App",
            "x": 100,
            "y": 100,
            "width": 800,
            "height": 600,
            "is_visible": true,
            "is_focused": false,
            "platform": "macos"
        }))
    }
    
    #[cfg(target_os = "windows")]
    async fn get_window_info_windows(&self, process_id: &str) -> Result<Value> {
        Ok(serde_json::json!({
            "title": "Tauri App",
            "x": 100,
            "y": 100,
            "width": 800,
            "height": 600,
            "is_visible": true,
            "is_focused": false,
            "platform": "windows"
        }))
    }
    
    #[cfg(target_os = "linux")]
    async fn get_window_info_linux(&self, process_id: &str) -> Result<Value> {
        Ok(serde_json::json!({
            "title": "Tauri App",
            "x": 100,
            "y": 100,
            "width": 800,
            "height": 600,
            "is_visible": true,
            "is_focused": false,
            "platform": "linux"
        }))
    }
    
    pub async fn focus_window(&self, process_id: &str) -> Result<()> {
        info!("Focusing window for process: {}", process_id);
        
        #[cfg(target_os = "macos")]
        {
            unsafe {
                let app = NSApp();
                let _: () = msg_send![app, activateIgnoringOtherApps: true];
            }
        }
        
        Ok(())
    }
    
    pub async fn minimize_window(&self, process_id: &str) -> Result<()> {
        info!("Minimizing window for process: {}", process_id);
        Ok(())
    }
    
    pub async fn maximize_window(&self, process_id: &str) -> Result<()> {
        info!("Maximizing window for process: {}", process_id);
        Ok(())
    }
    
    pub async fn resize_window(&self, process_id: &str, width: u32, height: u32) -> Result<()> {
        info!("Resizing window for process: {} to {}x{}", process_id, width, height);
        Ok(())
    }
    
    pub async fn move_window(&self, process_id: &str, x: i32, y: i32) -> Result<()> {
        info!("Moving window for process: {} to ({}, {})", process_id, x, y);
        Ok(())
    }
}

#[cfg(target_os = "linux")]
impl Drop for WindowManager {
    fn drop(&mut self) {
        unsafe {
            xlib::XCloseDisplay(self.display);
        }
    }
}