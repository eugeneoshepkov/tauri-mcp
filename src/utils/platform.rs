use crate::{Result, TauriMcpError};

#[cfg(target_os = "macos")]
pub fn get_window_by_pid(pid: u32) -> Result<Option<u64>> {
    Ok(None)
}

#[cfg(target_os = "windows")]
struct EnumData {
    target_pid: u32,
    window: Option<windows::Win32::Foundation::HWND>,
}

#[cfg(target_os = "windows")]
pub fn get_window_by_pid(pid: u32) -> Result<Option<windows::Win32::Foundation::HWND>> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::EnumWindows;
    use std::sync::Mutex;
    
    let data = Mutex::new(EnumData {
        target_pid: pid,
        window: None,
    });
    
    unsafe {
        EnumWindows(
            Some(enum_window_callback),
            &data as *const _ as isize,
        );
    }
    
    let result = data.lock().unwrap();
    Ok(result.window)
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn enum_window_callback(hwnd: HWND, lparam: isize) -> i32 {
    use std::sync::Mutex;
    use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
    
    let data = &*(lparam as *const Mutex<EnumData>);
    let mut process_id = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut process_id));
    
    let mut enum_data = data.lock().unwrap();
    if process_id == enum_data.target_pid {
        enum_data.window = Some(hwnd);
        return 0;
    }
    1
}

#[cfg(target_os = "linux")]
pub fn get_window_by_pid(pid: u32) -> Result<Option<u64>> {
    Ok(None)
}

pub fn is_tauri_app(path: &str) -> bool {
    path.contains("tauri") || path.ends_with(".app") || path.ends_with(".exe") || path.ends_with(".AppImage")
}

pub fn find_tauri_apps_in_directory(dir: &std::path::Path) -> Result<Vec<std::path::PathBuf>> {
    let mut apps = Vec::new();
    
    if !dir.exists() {
        return Ok(apps);
    }
    
    for entry in std::fs::read_dir(dir).map_err(|e| TauriMcpError::IoError(e))? {
        let entry = entry.map_err(|e| TauriMcpError::IoError(e))?;
        let path = entry.path();
        
        if path.is_file() && is_tauri_app(&path.to_string_lossy()) {
            apps.push(path);
        } else if path.is_dir() {
            if let Some(name) = path.file_name() {
                let name_str = name.to_string_lossy();
                if name_str == "target" || name_str == "src-tauri" {
                    if let Ok(mut sub_apps) = find_tauri_apps_in_directory(&path) {
                        apps.append(&mut sub_apps);
                    }
                }
            }
        }
    }
    
    Ok(apps)
}

#[cfg(target_os = "macos")]
pub fn activate_window(window_id: u64) -> Result<()> {
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn activate_window(hwnd: windows::Win32::Foundation::HWND) -> Result<()> {
    use windows::Win32::UI::WindowsAndMessaging::{SetForegroundWindow, ShowWindow, SW_RESTORE};
    
    unsafe {
        ShowWindow(hwnd, SW_RESTORE);
        SetForegroundWindow(hwnd);
    }
    
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn activate_window(window_id: u64) -> Result<()> {
    Ok(())
}