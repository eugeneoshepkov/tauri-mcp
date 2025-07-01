use crate::{Result, TauriMcpError};
use image::{DynamicImage, ImageFormat};
use screenshots::{Screen, Image};
use std::io::Cursor;

pub fn capture_screen_area(x: i32, y: i32, width: u32, height: u32) -> Result<DynamicImage> {
    let screens = Screen::all()
        .map_err(|e| TauriMcpError::ScreenshotError(format!("Failed to get screens: {}", e)))?;
    
    for screen in screens {
        let display_info = screen.display_info;
        if x >= display_info.x && y >= display_info.y &&
           x + width as i32 <= display_info.x + display_info.width as i32 &&
           y + height as i32 <= display_info.y + display_info.height as i32 {
            
            let image = screen.capture_area(x, y, width, height)
                .map_err(|e| TauriMcpError::ScreenshotError(format!("Failed to capture area: {}", e)))?;
            
            return Ok(image_to_dynamic(image)?);
        }
    }
    
    Err(TauriMcpError::ScreenshotError("No screen contains the specified area".to_string()))
}

pub fn capture_full_screen() -> Result<DynamicImage> {
    let screens = Screen::all()
        .map_err(|e| TauriMcpError::ScreenshotError(format!("Failed to get screens: {}", e)))?;
    
    if screens.is_empty() {
        return Err(TauriMcpError::ScreenshotError("No screens found".to_string()));
    }
    
    let image = screens[0].capture()
        .map_err(|e| TauriMcpError::ScreenshotError(format!("Failed to capture screen: {}", e)))?;
    
    image_to_dynamic(image)
}

fn image_to_dynamic(image: Image) -> Result<DynamicImage> {
    let buffer = image.to_png()
        .map_err(|e| TauriMcpError::ScreenshotError(format!("Failed to convert to PNG: {}", e)))?;
    
    let img = image::load_from_memory(&buffer)
        .map_err(|e| TauriMcpError::ScreenshotError(format!("Failed to load image: {}", e)))?;
    
    Ok(img)
}

pub fn image_to_base64(image: &DynamicImage) -> Result<String> {
    let mut buffer = Cursor::new(Vec::new());
    image.write_to(&mut buffer, ImageFormat::Png)
        .map_err(|e| TauriMcpError::ScreenshotError(format!("Failed to write image: {}", e)))?;
    
    use base64::{Engine as _, engine::general_purpose};
    Ok(general_purpose::STANDARD.encode(buffer.into_inner()))
}