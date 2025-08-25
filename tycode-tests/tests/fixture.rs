use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OutgoingMessage {
    Chat { message: String },
    SetSettings { path: String },
    Exit,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IncomingMessage {
    Ready {
        #[serde(default)]
        version: Option<String>,
    },
    Response {
        content: String,
    },
    Event {
        event: EventType,
        #[serde(default)]
        data: Option<EventData>,
    },
    ToolResult {
        tool_name: String,
        success: bool,
        #[serde(default)]
        error: Option<String>,
        #[serde(default)]
        result: Option<serde_json::Value>,
    },
    Error {
        error: String,
    },
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    RetryAttempt,
    System,
    #[serde(other)]
    Other,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EventData {
    RetryAttempt(RetryAttemptData),
    System(SystemEventData),
    Other(HashMap<String, serde_json::Value>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryAttemptData {
    pub attempt: u64,
    pub max_retries: u64,
    pub error: String,
    pub backoff_ms: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemEventData {
    pub content: String,
}

impl IncomingMessage {
    pub fn is_ready(&self) -> bool {
        matches!(self, IncomingMessage::Ready { .. })
    }

    pub fn is_response(&self) -> bool {
        matches!(self, IncomingMessage::Response { .. })
    }

    pub fn is_event(&self) -> bool {
        matches!(self, IncomingMessage::Event { .. })
    }

    pub fn is_retry_event(&self) -> bool {
        matches!(
            self,
            IncomingMessage::Event {
                event: EventType::RetryAttempt,
                ..
            }
        )
    }

    pub fn is_error(&self) -> bool {
        matches!(self, IncomingMessage::Error { .. })
    }

    pub fn is_tool_result(&self) -> bool {
        matches!(self, IncomingMessage::ToolResult { .. })
    }

    pub fn as_response(&self) -> Option<&str> {
        match self {
            IncomingMessage::Response { content } => Some(content.as_str()),
            _ => None,
        }
    }

    pub fn as_retry_event(&self) -> Option<&RetryAttemptData> {
        match self {
            IncomingMessage::Event {
                event: EventType::RetryAttempt,
                data: Some(EventData::RetryAttempt(retry_data)),
            } => Some(retry_data),
            _ => None,
        }
    }

    pub fn as_error(&self) -> Option<&str> {
        match self {
            IncomingMessage::Error { error } => Some(error.as_str()),
            _ => None,
        }
    }

    pub fn as_tool_result(&self) -> Option<(&str, bool, Option<&str>)> {
        match self {
            IncomingMessage::ToolResult {
                tool_name,
                success,
                error,
                ..
            } => Some((tool_name.as_str(), *success, error.as_deref())),
            _ => None,
        }
    }

    pub fn is_security_blocked(&self) -> bool {
        match self {
            IncomingMessage::ToolResult {
                success: false,
                error: Some(error),
                ..
            } => {
                error.contains("Security:")
                    || error.contains("denied by security")
                    || error.contains("Risk level:")
            }
            _ => false,
        }
    }

    pub fn extract_security_info(&self) -> Option<SecurityInfo> {
        if !self.is_security_blocked() {
            return None;
        }

        match self {
            IncomingMessage::ToolResult {
                tool_name,
                error: Some(error),
                ..
            } => {
                let mut risk_level = "unknown".to_string();
                let mut description = error.to_string();
                let mut risk_explanation = None;

                for line in error.lines() {
                    if let Some(level) = line.strip_prefix("Risk level: ") {
                        risk_level = level.to_string();
                    } else if let Some(desc) = line.strip_prefix("Security: ") {
                        description = desc.to_string();
                    } else if let Some(risk) = line.strip_prefix("Risk: ") {
                        risk_explanation = Some(risk.to_string());
                    }
                }

                Some(SecurityInfo {
                    tool_name: tool_name.clone(),
                    risk_level,
                    description,
                    risk_explanation,
                })
            }
            _ => None,
        }
    }
}

/// Common test fixture for subprocess integration tests
#[derive(Debug)]
pub struct SubprocessFixture {
    process: std::process::Child,
    stdin: std::process::ChildStdin,
    rx: mpsc::Receiver<String>,
    pub temp_dir: PathBuf,
    pub settings_path: PathBuf,
}

impl SubprocessFixture {
    pub fn new() -> anyhow::Result<Self> {
        Self::with_settings(None)
    }

    fn get_tycode_binary() -> anyhow::Result<PathBuf> {
        let workspace_root = Self::find_workspace_root()?;
        let binary_path = workspace_root.join("target").join("debug").join("tycode");
        
        // Check if binary exists and how old it is
        let should_rebuild = if binary_path.exists() {
            // Get binary age
            let metadata = std::fs::metadata(&binary_path)?;
            let modified = metadata.modified()?;
            let age = std::time::SystemTime::now().duration_since(modified)?;
            
            // Rebuild if older than 60 seconds (configurable via env var)
            let max_age_secs = std::env::var("TYCODE_TEST_BINARY_MAX_AGE")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(60);
            
            if age.as_secs() > max_age_secs {
                eprintln!("Binary is {} seconds old (max: {}), rebuilding...", 
                    age.as_secs(), max_age_secs);
                true
            } else {
                eprintln!("Using existing binary ({} seconds old)", age.as_secs());
                false
            }
        } else {
            eprintln!("Binary not found, building...");
            true
        };

        if !should_rebuild {
            return Ok(binary_path);
        }

        // Build the binary (cargo will handle incremental compilation)
        eprintln!("Rebuilding binary...");
        let output = Command::new("cargo")
            .args(&["build", "--bin", "tycode", "-p", "tycode-cli"])
            .current_dir(&workspace_root)
            .output()?;

        if !output.status.success() {
            anyhow::bail!(
                "Failed to build tycode binary: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        if binary_path.exists() {
            Ok(binary_path)
        } else {
            anyhow::bail!("Binary not found after build at: {:?}", binary_path)
        }
    }

    fn find_workspace_root() -> anyhow::Result<PathBuf> {
        let current_dir = std::env::current_dir()?;
        let mut search_dir = current_dir.as_path();

        for _ in 0..5 {
            let cargo_toml = search_dir.join("Cargo.toml");
            if cargo_toml.exists() {
                let contents = std::fs::read_to_string(&cargo_toml)?;
                if contents.contains("[workspace]") {
                    return Ok(search_dir.to_path_buf());
                }
            }

            search_dir = match search_dir.parent() {
                Some(parent) => parent,
                None => break,
            };
        }

        let current_dir = std::env::current_dir()?;

        if current_dir.ends_with("tests") {
            let parent = current_dir.parent();
            if let Some(p) = parent {
                if p.ends_with("tycode-cli") {
                    if let Some(workspace) = p.parent() {
                        return Ok(workspace.to_path_buf());
                    }
                }
            }
        }

        if current_dir.ends_with("tycode-cli") {
            if let Some(workspace) = current_dir.parent() {
                return Ok(workspace.to_path_buf());
            }
        }

        Ok(current_dir)
    }

    pub fn with_settings(initial_settings: Option<&str>) -> anyhow::Result<Self> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis();
        let random: u32 = std::process::id();
        let thread_id = format!("{:?}", std::thread::current().id())
            .replace("ThreadId(", "")
            .replace(")", "");
        let temp_dir =
            std::env::temp_dir().join(format!("tycode_test_{}_{}{}", timestamp, random, thread_id));
        std::fs::create_dir_all(&temp_dir)?;
        let settings_path = temp_dir.join("settings.toml");

        let settings_content = initial_settings.unwrap_or(
            r#"
active_provider = "default"

[providers.default]
type = "bedrock"
profile = "default"
region = "us-east-1"

[security]
mode = "auto"
"#,
        );
        std::fs::write(&settings_path, settings_content)?;

        let tycode_binary = Self::get_tycode_binary()?;
        println!("Found binary: {tycode_binary:?}");

        let mut process = Command::new(&tycode_binary)
            .args(&[
                "--subprocess",
                "--settings-path",
                settings_path.to_str().unwrap(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        println!("launched binary");

        let stdin = process.stdin.take().expect("Failed to get stdin");
        let stdout = process.stdout.take().expect("Failed to get stdout");

        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    if std::env::var("TEST_DEBUG").is_ok() {
                        eprintln!("RECV: {}", line);
                    }
                    let _ = tx.send(line);
                }
            }
        });

        Ok(SubprocessFixture {
            process,
            stdin,
            rx,
            temp_dir,
            settings_path,
        })
    }

    pub fn send_message(&mut self, msg: Value) -> anyhow::Result<()> {
        let json = serde_json::to_string(&msg)?;
        if std::env::var("TEST_DEBUG").is_ok() {
            eprintln!("SEND: {}", json);
        }
        writeln!(self.stdin, "{}", json)?;
        self.stdin.flush()?;
        Ok(())
    }

    pub fn send(&mut self, msg: OutgoingMessage) -> anyhow::Result<()> {
        let json = serde_json::to_string(&msg)?;
        if std::env::var("TEST_DEBUG").is_ok() {
            eprintln!("SEND: {}", json);
        }
        writeln!(self.stdin, "{}", json)?;
        self.stdin.flush()?;
        Ok(())
    }

    pub fn send_chat(&mut self, message: &str) -> anyhow::Result<()> {
        self.send(OutgoingMessage::Chat {
            message: message.to_string(),
        })
    }

    pub fn receive_message(&self) -> anyhow::Result<Value> {
        let line = self.rx.recv()?;
        Ok(serde_json::from_str(&line)?)
    }

    pub fn receive(&self) -> anyhow::Result<IncomingMessage> {
        let line = self.rx.recv()?;
        Ok(serde_json::from_str(&line)?)
    }

    pub fn try_receive(&self) -> Option<IncomingMessage> {
        match self.rx.try_recv() {
            Ok(line) => serde_json::from_str(&line).ok(),
            Err(_) => None,
        }
    }

    pub fn wait_for_ready(&self) -> anyhow::Result<Value> {
        loop {
            let msg = self.receive_message()?;
            if msg.get("type") == Some(&json!("Ready")) {
                return Ok(msg);
            }
        }
    }

    pub fn wait_for_ready_typed(&self) -> anyhow::Result<()> {
        loop {
            let msg = self.receive()?;
            if msg.is_ready() {
                return Ok(());
            }
        }
    }

    pub fn collect_retry_events_and_response(
        &self,
    ) -> anyhow::Result<(Vec<RetryAttemptData>, IncomingMessage)> {
        let mut retry_events = Vec::new();
        let max_messages = 20;

        for _ in 0..max_messages {
            let msg = self.receive()?;

            if let Some(retry_data) = msg.as_retry_event() {
                retry_events.push(retry_data.clone());
            } else if msg.is_response() || msg.is_error() {
                return Ok((retry_events, msg));
            }
        }

        anyhow::bail!(
            "Did not receive response or error after {} messages",
            max_messages
        )
    }

    pub fn wait_for_response_or_error(&self) -> anyhow::Result<IncomingMessage> {
        loop {
            let msg = self.receive()?;
            if msg.is_response() || msg.is_error() {
                return Ok(msg);
            }
        }
    }

    pub fn wait_for_message_type(&self, msg_type: &str) -> anyhow::Result<Value> {
        loop {
            let msg = self.receive_message()?;
            if msg.get("type") == Some(&json!(msg_type)) {
                return Ok(msg);
            }
            if std::env::var("TEST_DEBUG").is_ok() {
                eprintln!("Unexpected message type: {:?}", msg.get("type"));
            }
        }
    }

    pub fn try_receive_message(&self) -> Option<Value> {
        match self.rx.try_recv() {
            Ok(line) => serde_json::from_str(&line).ok(),
            Err(_) => None,
        }
    }

    pub fn drain_messages(&self) {
        while self.try_receive_message().is_some() {}
    }

    pub fn send_chat_message(&mut self, content: &str) -> anyhow::Result<Value> {
        self.send_message(json!({
            "type": "Chat",
            "message": content
        }))?;

        if content.starts_with("/") {
            loop {
                let msg = self.receive_message()?;
                if msg.get("type") == Some(&json!("Event")) {
                    if msg.get("event") != Some(&json!("system")) {
                        continue;
                    }
                    let data = match msg.get("data") {
                        Some(d) => d,
                        None => continue,
                    };
                    let content = match data.get("content").and_then(|c| c.as_str()) {
                        Some(c) => c,
                        None => continue,
                    };
                    return Ok(json!({
                        "type": "Response",
                        "content": content
                    }));
                } else if msg.get("type") == Some(&json!("Response")) {
                    return Ok(msg);
                } else if msg.get("type") == Some(&json!("ToolResult")) {
                    return Ok(msg);
                }
            }
        } else {
            self.wait_for_message_type("Response")
        }
    }

    pub fn create_test_file(&self, name: &str, content: &str) -> anyhow::Result<PathBuf> {
        let file_path = self.temp_dir.join(name);
        std::fs::write(&file_path, content)?;
        Ok(file_path)
    }

    pub fn file_exists(&self, name: &str) -> bool {
        self.temp_dir.join(name).exists()
    }

    pub fn read_file(&self, name: &str) -> anyhow::Result<String> {
        Ok(std::fs::read_to_string(self.temp_dir.join(name))?)
    }
}

impl Drop for SubprocessFixture {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();

        std::thread::sleep(std::time::Duration::from_millis(100));

        let _ = std::fs::remove_dir_all(&self.temp_dir);
    }
}

pub fn is_error_response(message: &Value) -> bool {
    if message.get("type") == Some(&json!("Error")) {
        return true;
    }

    if message.get("type") == Some(&json!("ToolResult")) {
        if message.get("success") == Some(&json!(false)) {
            return true;
        }
    }

    false
}

pub fn is_security_blocked(message: &Value) -> bool {
    if message.get("type") != Some(&json!("ToolResult")) {
        return false;
    }
    
    if message.get("success") != Some(&json!(false)) {
        return false;
    }
    
    match message.get("error").and_then(|e| e.as_str()) {
        Some(error) => {
            error.contains("Security:")
                || error.contains("denied by security")
                || error.contains("Risk level:")
        }
        None => false,
    }
}

pub fn extract_security_info(message: &Value) -> Option<SecurityInfo> {
    if !is_security_blocked(message) {
        return None;
    }

    let tool_name = message.get("tool_name")?.as_str()?.to_string();
    let error = message.get("error")?.as_str()?;

    let mut risk_level = "unknown".to_string();
    let mut description = error.to_string();
    let mut risk_explanation = None;

    for line in error.lines() {
        if let Some(level) = line.strip_prefix("Risk level: ") {
            risk_level = level.to_string();
        } else if let Some(desc) = line.strip_prefix("Security: ") {
            description = desc.to_string();
        } else if let Some(risk) = line.strip_prefix("Risk: ") {
            risk_explanation = Some(risk.to_string());
        }
    }

    Some(SecurityInfo {
        tool_name,
        risk_level,
        description,
        risk_explanation,
    })
}

#[derive(Debug, Clone)]
pub struct SecurityInfo {
    pub tool_name: String,
    pub risk_level: String,
    pub description: String,
    pub risk_explanation: Option<String>,
}

pub fn parse_security_status(message: &Value) -> Option<SecurityStatus> {
    if message.get("type") != Some(&json!("Response")) {
        return None;
    }

    let content = message.get("content")?.as_str()?;
    let mut mode = None;

    for line in content.lines() {
        let trimmed = line.trim();
        
        if let Some(mode_str) = trimmed.strip_prefix("Mode:") {
            mode = Some(mode_str.trim().to_string());
            break;
        }
        
        let lower = trimmed.to_lowercase();
        if !lower.contains("mode") {
            continue;
        }
        
        if let Some(colon_pos) = trimmed.find(':') {
            let mode_value = trimmed[colon_pos + 1..].trim();
            if !mode_value.is_empty() {
                mode = Some(mode_value.to_string());
                break;
            }
        }
    }

    if mode.is_none() && !content.contains('\n') {
        let trimmed = content.trim();
        if matches!(trimmed, "Auto" | "All" | "Critical" | "None") {
            mode = Some(trimmed.to_string());
        }
    }

    mode.map(|m| SecurityStatus { mode: m })
}

#[derive(Debug, Clone)]
pub struct SecurityStatus {
    pub mode: String,
}
