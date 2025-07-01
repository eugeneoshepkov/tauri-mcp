use crate::{Result, TauriMcpError};
use enigo::{Enigo, Key, Settings, Button, Coordinate, Direction::Click, Keyboard, Mouse, Axis};
use std::thread;
use std::time::Duration;
use tracing::{debug, info};

pub struct InputSimulator {}

impl InputSimulator {
    pub fn new() -> Self {
        Self {}
    }
    
    pub async fn send_keyboard_input(&self, process_id: &str, keys: &str) -> Result<()> {
        info!("Sending keyboard input to process {}: {}", process_id, keys);
        
        let keys_to_send = keys.to_string();
        let settings = Settings::default();
        
        tokio::task::spawn_blocking(move || {
            let mut enigo = Enigo::new(&settings)
                .map_err(|e| TauriMcpError::InputError(format!("Failed to create Enigo: {:?}", e)))?;
            
            if keys_to_send.starts_with("cmd+") || keys_to_send.starts_with("ctrl+") {
                Self::send_key_combination(&mut enigo, &keys_to_send)?;
            } else {
                for ch in keys_to_send.chars() {
                    enigo.text(&ch.to_string())
                        .map_err(|e| TauriMcpError::InputError(format!("Failed to send text: {:?}", e)))?;
                    thread::sleep(Duration::from_millis(10));
                }
            }
            
            Ok::<(), TauriMcpError>(())
        })
        .await
        .map_err(|e| TauriMcpError::InputError(format!("Failed to send keyboard input: {}", e)))??;
        
        Ok(())
    }
    
    pub async fn send_mouse_click(&self, process_id: &str, x: i32, y: i32, button: &str) -> Result<()> {
        info!("Sending mouse click to process {} at ({}, {}), button: {}", process_id, x, y, button);
        
        let button_to_click = match button.to_lowercase().as_str() {
            "left" => Button::Left,
            "right" => Button::Right,
            "middle" => Button::Middle,
            _ => return Err(TauriMcpError::InputError(format!("Invalid mouse button: {}", button))),
        };
        
        let settings = Settings::default();
        
        tokio::task::spawn_blocking(move || {
            let mut enigo = Enigo::new(&settings)
                .map_err(|e| TauriMcpError::InputError(format!("Failed to create Enigo: {:?}", e)))?;
            
            enigo.move_mouse(x, y, Coordinate::Abs)
                .map_err(|e| TauriMcpError::InputError(format!("Failed to move mouse: {:?}", e)))?;
            thread::sleep(Duration::from_millis(50));
            
            enigo.button(button_to_click, Click)
                .map_err(|e| TauriMcpError::InputError(format!("Failed to click: {:?}", e)))?;
            
            Ok::<(), TauriMcpError>(())
        })
        .await
        .map_err(|e| TauriMcpError::InputError(format!("Failed to send mouse click: {}", e)))??;
        
        Ok(())
    }
    
    pub async fn send_mouse_move(&self, process_id: &str, x: i32, y: i32) -> Result<()> {
        info!("Moving mouse for process {} to ({}, {})", process_id, x, y);
        
        let settings = Settings::default();
        
        tokio::task::spawn_blocking(move || {
            let mut enigo = Enigo::new(&settings)
                .map_err(|e| TauriMcpError::InputError(format!("Failed to create Enigo: {:?}", e)))?;
            enigo.move_mouse(x, y, Coordinate::Abs)
                .map_err(|e| TauriMcpError::InputError(format!("Failed to move mouse: {:?}", e)))?;
            Ok::<(), TauriMcpError>(())
        })
        .await
        .map_err(|e| TauriMcpError::InputError(format!("Failed to move mouse: {}", e)))??;
        
        Ok(())
    }
    
    pub async fn send_mouse_drag(&self, process_id: &str, start_x: i32, start_y: i32, end_x: i32, end_y: i32) -> Result<()> {
        info!("Dragging mouse for process {} from ({}, {}) to ({}, {})", 
              process_id, start_x, start_y, end_x, end_y);
        
        let settings = Settings::default();
        
        tokio::task::spawn_blocking(move || {
            let mut enigo = Enigo::new(&settings)
                .map_err(|e| TauriMcpError::InputError(format!("Failed to create Enigo: {:?}", e)))?;
            
            enigo.move_mouse(start_x, start_y, Coordinate::Abs)
                .map_err(|e| TauriMcpError::InputError(format!("Failed to move mouse: {:?}", e)))?;
            thread::sleep(Duration::from_millis(50));
            
            enigo.button(Button::Left, enigo::Direction::Press)
                .map_err(|e| TauriMcpError::InputError(format!("Failed to press button: {:?}", e)))?;
            thread::sleep(Duration::from_millis(50));
            
            let steps = 10;
            let dx = (end_x - start_x) as f32 / steps as f32;
            let dy = (end_y - start_y) as f32 / steps as f32;
            
            for i in 1..=steps {
                let x = start_x + (dx * i as f32) as i32;
                let y = start_y + (dy * i as f32) as i32;
                enigo.move_mouse(x, y, Coordinate::Abs)
                    .map_err(|e| TauriMcpError::InputError(format!("Failed to move mouse: {:?}", e)))?;
                thread::sleep(Duration::from_millis(20));
            }
            
            enigo.button(Button::Left, enigo::Direction::Release)
                .map_err(|e| TauriMcpError::InputError(format!("Failed to release button: {:?}", e)))?;
            
            Ok::<(), TauriMcpError>(())
        })
        .await
        .map_err(|e| TauriMcpError::InputError(format!("Failed to drag mouse: {}", e)))??;
        
        Ok(())
    }
    
    pub async fn send_mouse_scroll(&self, process_id: &str, x: i32, y: i32, delta: i32) -> Result<()> {
        info!("Scrolling mouse for process {} at ({}, {}), delta: {}", process_id, x, y, delta);
        
        let settings = Settings::default();
        
        tokio::task::spawn_blocking(move || {
            let mut enigo = Enigo::new(&settings)
                .map_err(|e| TauriMcpError::InputError(format!("Failed to create Enigo: {:?}", e)))?;
            
            enigo.move_mouse(x, y, Coordinate::Abs)
                .map_err(|e| TauriMcpError::InputError(format!("Failed to move mouse: {:?}", e)))?;
            thread::sleep(Duration::from_millis(50));
            
            enigo.scroll(delta, Axis::Vertical)
                .map_err(|e| TauriMcpError::InputError(format!("Failed to scroll: {:?}", e)))?;
            
            Ok::<(), TauriMcpError>(())
        })
        .await
        .map_err(|e| TauriMcpError::InputError(format!("Failed to scroll mouse: {}", e)))??;
        
        Ok(())
    }
    
    fn send_key_combination(enigo: &mut Enigo, combination: &str) -> Result<()> {
        let parts: Vec<&str> = combination.split('+').collect();
        if parts.len() < 2 {
            return Err(TauriMcpError::InputError(format!("Invalid key combination: {}", combination)));
        }
        
        let mut modifier_keys = Vec::new();
        let mut main_key = None;
        
        for (i, part) in parts.iter().enumerate() {
            let key_str = part.trim().to_lowercase();
            
            if i < parts.len() - 1 {
                match key_str.as_str() {
                    "cmd" | "meta" => modifier_keys.push(Key::Meta),
                    "ctrl" | "control" => modifier_keys.push(Key::Control),
                    "alt" | "option" => modifier_keys.push(Key::Alt),
                    "shift" => modifier_keys.push(Key::Shift),
                    _ => return Err(TauriMcpError::InputError(format!("Unknown modifier key: {}", key_str))),
                }
            } else {
                main_key = Some(Self::string_to_key(&key_str)?);
            }
        }
        
        for key in &modifier_keys {
            enigo.key(*key, enigo::Direction::Press)
                .map_err(|e| TauriMcpError::InputError(format!("Failed to press key: {:?}", e)))?;
            thread::sleep(Duration::from_millis(10));
        }
        
        if let Some(key) = main_key {
            enigo.key(key, Click)
                .map_err(|e| TauriMcpError::InputError(format!("Failed to click key: {:?}", e)))?;
            thread::sleep(Duration::from_millis(10));
        }
        
        for key in modifier_keys.iter().rev() {
            enigo.key(*key, enigo::Direction::Release)
                .map_err(|e| TauriMcpError::InputError(format!("Failed to release key: {:?}", e)))?;
            thread::sleep(Duration::from_millis(10));
        }
        
        Ok(())
    }
    
    fn string_to_key(s: &str) -> Result<Key> {
        match s {
            "a" => Ok(Key::Unicode('a')),
            "b" => Ok(Key::Unicode('b')),
            "c" => Ok(Key::Unicode('c')),
            "d" => Ok(Key::Unicode('d')),
            "e" => Ok(Key::Unicode('e')),
            "f" => Ok(Key::Unicode('f')),
            "g" => Ok(Key::Unicode('g')),
            "h" => Ok(Key::Unicode('h')),
            "i" => Ok(Key::Unicode('i')),
            "j" => Ok(Key::Unicode('j')),
            "k" => Ok(Key::Unicode('k')),
            "l" => Ok(Key::Unicode('l')),
            "m" => Ok(Key::Unicode('m')),
            "n" => Ok(Key::Unicode('n')),
            "o" => Ok(Key::Unicode('o')),
            "p" => Ok(Key::Unicode('p')),
            "q" => Ok(Key::Unicode('q')),
            "r" => Ok(Key::Unicode('r')),
            "s" => Ok(Key::Unicode('s')),
            "t" => Ok(Key::Unicode('t')),
            "u" => Ok(Key::Unicode('u')),
            "v" => Ok(Key::Unicode('v')),
            "w" => Ok(Key::Unicode('w')),
            "x" => Ok(Key::Unicode('x')),
            "y" => Ok(Key::Unicode('y')),
            "z" => Ok(Key::Unicode('z')),
            "enter" | "return" => Ok(Key::Return),
            "tab" => Ok(Key::Tab),
            "space" => Ok(Key::Space),
            "backspace" => Ok(Key::Backspace),
            "escape" | "esc" => Ok(Key::Escape),
            "delete" | "del" => Ok(Key::Delete),
            "home" => Ok(Key::Home),
            "end" => Ok(Key::End),
            "pageup" => Ok(Key::PageUp),
            "pagedown" => Ok(Key::PageDown),
            "left" => Ok(Key::LeftArrow),
            "right" => Ok(Key::RightArrow),
            "up" => Ok(Key::UpArrow),
            "down" => Ok(Key::DownArrow),
            "f1" => Ok(Key::F1),
            "f2" => Ok(Key::F2),
            "f3" => Ok(Key::F3),
            "f4" => Ok(Key::F4),
            "f5" => Ok(Key::F5),
            "f6" => Ok(Key::F6),
            "f7" => Ok(Key::F7),
            "f8" => Ok(Key::F8),
            "f9" => Ok(Key::F9),
            "f10" => Ok(Key::F10),
            "f11" => Ok(Key::F11),
            "f12" => Ok(Key::F12),
            _ => Err(TauriMcpError::InputError(format!("Unknown key: {}", s))),
        }
    }
}