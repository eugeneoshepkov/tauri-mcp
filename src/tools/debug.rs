use crate::{Result, TauriMcpError};
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, error, info};

pub struct DebugTools {
    client: Client,
    webdriver_sessions: HashMap<String, WebDriverSession>,
}

struct WebDriverSession {
    session_id: String,
    debug_port: u16,
}

impl DebugTools {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
        
        Self {
            client,
            webdriver_sessions: HashMap::new(),
        }
    }
    
    pub async fn execute_js(&self, process_id: &str, javascript_code: &str) -> Result<Value> {
        info!("Executing JavaScript for process {}: {}", process_id, javascript_code);
        
        if let Some(session) = self.webdriver_sessions.get(process_id) {
            self.execute_via_webdriver(session, javascript_code).await
        } else {
            self.execute_via_devtools(process_id, javascript_code).await
        }
    }
    
    pub async fn get_devtools_info(&self, process_id: &str) -> Result<Value> {
        info!("Getting DevTools info for process: {}", process_id);
        
        let debug_port = self.find_debug_port(process_id).await?;
        
        let url = format!("http://localhost:{}/json/version", debug_port);
        let response = self.client.get(&url)
            .send()
            .await
            .map_err(|e| TauriMcpError::WebDriverError(format!("Failed to get DevTools info: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(TauriMcpError::WebDriverError(format!("DevTools returned error: {}", response.status())));
        }
        
        let info: Value = response.json().await
            .map_err(|e| TauriMcpError::WebDriverError(format!("Failed to parse DevTools response: {}", e)))?;
        
        Ok(serde_json::json!({
            "debug_port": debug_port,
            "devtools_url": format!("http://localhost:{}", debug_port),
            "version_info": info,
        }))
    }
    
    pub async fn connect_webdriver(&mut self, process_id: &str, debug_port: u16) -> Result<()> {
        info!("Connecting WebDriver for process {} on port {}", process_id, debug_port);
        
        let capabilities = serde_json::json!({
            "capabilities": {
                "alwaysMatch": {
                    "browserName": "chrome",
                    "goog:chromeOptions": {
                        "debuggerAddress": format!("localhost:{}", debug_port)
                    }
                }
            }
        });
        
        let url = format!("http://localhost:9515/session");
        let response = self.client.post(&url)
            .json(&capabilities)
            .send()
            .await
            .map_err(|e| TauriMcpError::WebDriverError(format!("Failed to connect WebDriver: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(TauriMcpError::WebDriverError(format!("WebDriver connection failed: {}", response.status())));
        }
        
        let session_data: Value = response.json().await
            .map_err(|e| TauriMcpError::WebDriverError(format!("Failed to parse WebDriver response: {}", e)))?;
        
        let session_id = session_data["value"]["sessionId"].as_str()
            .ok_or_else(|| TauriMcpError::WebDriverError("No session ID in response".to_string()))?
            .to_string();
        
        self.webdriver_sessions.insert(process_id.to_string(), WebDriverSession {
            session_id,
            debug_port,
        });
        
        Ok(())
    }
    
    pub async fn get_page_source(&self, process_id: &str) -> Result<String> {
        info!("Getting page source for process: {}", process_id);
        
        if let Some(session) = self.webdriver_sessions.get(process_id) {
            let url = format!("http://localhost:9515/session/{}/source", session.session_id);
            let response = self.client.get(&url)
                .send()
                .await
                .map_err(|e| TauriMcpError::WebDriverError(format!("Failed to get page source: {}", e)))?;
            
            if !response.status().is_success() {
                return Err(TauriMcpError::WebDriverError(format!("Failed to get page source: {}", response.status())));
            }
            
            let data: Value = response.json().await
                .map_err(|e| TauriMcpError::WebDriverError(format!("Failed to parse response: {}", e)))?;
            
            Ok(data["value"].as_str().unwrap_or("").to_string())
        } else {
            Err(TauriMcpError::WebDriverError("No WebDriver session found".to_string()))
        }
    }
    
    pub async fn get_console_logs(&self, process_id: &str) -> Result<Vec<Value>> {
        info!("Getting console logs for process: {}", process_id);
        
        if let Some(session) = self.webdriver_sessions.get(process_id) {
            let url = format!("http://localhost:9515/session/{}/se/log", session.session_id);
            let body = serde_json::json!({
                "type": "browser"
            });
            
            let response = self.client.post(&url)
                .json(&body)
                .send()
                .await
                .map_err(|e| TauriMcpError::WebDriverError(format!("Failed to get console logs: {}", e)))?;
            
            if !response.status().is_success() {
                return Err(TauriMcpError::WebDriverError(format!("Failed to get console logs: {}", response.status())));
            }
            
            let data: Value = response.json().await
                .map_err(|e| TauriMcpError::WebDriverError(format!("Failed to parse response: {}", e)))?;
            
            Ok(data["value"].as_array().cloned().unwrap_or_default())
        } else {
            Err(TauriMcpError::WebDriverError("No WebDriver session found".to_string()))
        }
    }
    
    pub async fn take_element_screenshot(&self, process_id: &str, selector: &str) -> Result<String> {
        info!("Taking element screenshot for process {}, selector: {}", process_id, selector);
        
        if let Some(session) = self.webdriver_sessions.get(process_id) {
            let find_url = format!("http://localhost:9515/session/{}/element", session.session_id);
            let find_body = serde_json::json!({
                "using": "css selector",
                "value": selector
            });
            
            let find_response = self.client.post(&find_url)
                .json(&find_body)
                .send()
                .await
                .map_err(|e| TauriMcpError::WebDriverError(format!("Failed to find element: {}", e)))?;
            
            if !find_response.status().is_success() {
                return Err(TauriMcpError::WebDriverError(format!("Element not found: {}", selector)));
            }
            
            let element_data: Value = find_response.json().await
                .map_err(|e| TauriMcpError::WebDriverError(format!("Failed to parse response: {}", e)))?;
            
            let element_id = element_data["value"]["element-6066-11e4-a52e-4f735466cecf"].as_str()
                .or_else(|| element_data["value"]["ELEMENT"].as_str())
                .ok_or_else(|| TauriMcpError::WebDriverError("No element ID in response".to_string()))?;
            
            let screenshot_url = format!("http://localhost:9515/session/{}/element/{}/screenshot", 
                                       session.session_id, element_id);
            
            let screenshot_response = self.client.get(&screenshot_url)
                .send()
                .await
                .map_err(|e| TauriMcpError::WebDriverError(format!("Failed to take screenshot: {}", e)))?;
            
            if !screenshot_response.status().is_success() {
                return Err(TauriMcpError::WebDriverError(format!("Failed to take screenshot: {}", screenshot_response.status())));
            }
            
            let screenshot_data: Value = screenshot_response.json().await
                .map_err(|e| TauriMcpError::WebDriverError(format!("Failed to parse response: {}", e)))?;
            
            Ok(format!("data:image/png;base64,{}", screenshot_data["value"].as_str().unwrap_or("")))
        } else {
            Err(TauriMcpError::WebDriverError("No WebDriver session found".to_string()))
        }
    }
    
    async fn execute_via_webdriver(&self, session: &WebDriverSession, javascript_code: &str) -> Result<Value> {
        let url = format!("http://localhost:9515/session/{}/execute/sync", session.session_id);
        let body = serde_json::json!({
            "script": javascript_code,
            "args": []
        });
        
        let response = self.client.post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| TauriMcpError::WebDriverError(format!("Failed to execute JavaScript: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(TauriMcpError::WebDriverError(format!("JavaScript execution failed: {}", response.status())));
        }
        
        let data: Value = response.json().await
            .map_err(|e| TauriMcpError::WebDriverError(format!("Failed to parse response: {}", e)))?;
        
        Ok(data["value"].clone())
    }
    
    async fn execute_via_devtools(&self, process_id: &str, javascript_code: &str) -> Result<Value> {
        let debug_port = self.find_debug_port(process_id).await?;
        
        let list_url = format!("http://localhost:{}/json/list", debug_port);
        let response = self.client.get(&list_url)
            .send()
            .await
            .map_err(|e| TauriMcpError::WebDriverError(format!("Failed to list pages: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(TauriMcpError::WebDriverError(format!("Failed to list pages: {}", response.status())));
        }
        
        let pages: Vec<Value> = response.json().await
            .map_err(|e| TauriMcpError::WebDriverError(format!("Failed to parse pages: {}", e)))?;
        
        if pages.is_empty() {
            return Err(TauriMcpError::WebDriverError("No pages found".to_string()));
        }
        
        let page_id = pages[0]["id"].as_str()
            .ok_or_else(|| TauriMcpError::WebDriverError("No page ID found".to_string()))?;
        
        Ok(serde_json::json!({
            "status": "Would execute JavaScript via DevTools",
            "code": javascript_code,
            "page_id": page_id,
        }))
    }
    
    async fn find_debug_port(&self, _process_id: &str) -> Result<u16> {
        for port in 9222..9250 {
            let url = format!("http://localhost:{}/json/version", port);
            if let Ok(response) = self.client.get(&url).send().await {
                if response.status().is_success() {
                    return Ok(port);
                }
            }
        }
        
        Err(TauriMcpError::WebDriverError("No debug port found".to_string()))
    }
}