use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

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
    /// Create a new subprocess test fixture with default settings
    pub fn new() -> anyhow::Result<Self> {
        Self::with_settings(None)
    }

    /// Get the path to the tycode binary, building it if necessary
    fn get_tycode_binary() -> anyhow::Result<PathBuf> {
        // Strategy 1: Check CARGO_TARGET_DIR environment variable (set by cargo test)
        if let Ok(target_dir) = std::env::var("CARGO_TARGET_DIR") {
            let binary = Path::new(&target_dir).join("debug").join("tycode");
            if binary.exists() {
                return Ok(binary.canonicalize()?);
            }
        }

        // Strategy 2: Use the test binary location to find the target directory
        if let Ok(current_exe) = std::env::current_exe() {
            // The test binary is typically at target/debug/deps/test_name-hash
            // We need to go up to find the target directory
            if let Some(target_dir) = current_exe
                .parent() // deps
                .and_then(|p| p.parent()) // debug or release
                .and_then(|p| p.parent())
            // target
            {
                // Check both debug and release
                for profile in &["debug", "release"] {
                    let binary = target_dir.join(profile).join("tycode");
                    if binary.exists() {
                        return Ok(binary.canonicalize()?);
                    }
                }
            }
        }

        // Strategy 3: Search relative to current directory and parent directories
        let current_dir = std::env::current_dir()?;
        let mut search_dir = current_dir.as_path();

        // Search up to 3 levels up for target directory
        for _ in 0..3 {
            let target_dir = search_dir.join("target");
            if target_dir.exists() {
                for profile in &["debug", "release"] {
                    let binary = target_dir.join(profile).join("tycode");
                    if binary.exists() {
                        return Ok(binary.canonicalize()?);
                    }
                }
            }

            // Move up one directory
            if let Some(parent) = search_dir.parent() {
                search_dir = parent;
            } else {
                break;
            }
        }

        // Binary not found, try to build it
        eprintln!("tycode binary not found, attempting to build...");

        // Find the workspace root (directory containing root Cargo.toml with [workspace])
        let workspace_root = Self::find_workspace_root()?;

        // Build the binary
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

        // After building, try to find it again using the same strategies
        let target_dir = workspace_root.join("target");
        let binary = target_dir.join("debug").join("tycode");

        if binary.exists() {
            Ok(binary.canonicalize()?)
        } else {
            anyhow::bail!(
                "tycode binary not found after building. Expected at: {:?}",
                binary
            );
        }
    }

    /// Find the workspace root by looking for Cargo.toml with [workspace]
    fn find_workspace_root() -> anyhow::Result<PathBuf> {
        let current_dir = std::env::current_dir()?;
        let mut search_dir = current_dir.as_path();

        // Search up to 5 levels up for workspace root
        for _ in 0..5 {
            let cargo_toml = search_dir.join("Cargo.toml");
            if cargo_toml.exists() {
                // Check if this is the workspace root by looking for [workspace] section
                let contents = std::fs::read_to_string(&cargo_toml)?;
                if contents.contains("[workspace]") {
                    return Ok(search_dir.to_path_buf());
                }
            }

            // Move up one directory
            if let Some(parent) = search_dir.parent() {
                search_dir = parent;
            } else {
                break;
            }
        }

        // Fallback: assume we're in a subdirectory of the workspace
        // Look for tycode-cli directory as a sibling or parent
        let current_dir = std::env::current_dir()?;

        // If we're in tycode-cli/tests, go up two levels
        if current_dir.ends_with("tests")
            && current_dir
                .parent()
                .map_or(false, |p| p.ends_with("tycode-cli"))
        {
            if let Some(workspace) = current_dir.parent().and_then(|p| p.parent()) {
                return Ok(workspace.to_path_buf());
            }
        }

        // If we're in tycode-cli, go up one level
        if current_dir.ends_with("tycode-cli") {
            if let Some(workspace) = current_dir.parent() {
                return Ok(workspace.to_path_buf());
            }
        }

        // Last resort: use current directory
        Ok(current_dir)
    }

    /// Create a new subprocess test fixture with custom initial settings
    pub fn with_settings(initial_settings: Option<&str>) -> anyhow::Result<Self> {
        // Create a unique temp directory with random component
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

        // Create initial settings file
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

        // Get the tycode binary path
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

        // Spawn thread to read stdout
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    // Debug output for test development
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

    /// Send a JSON message to the subprocess
    pub fn send_message(&mut self, msg: Value) -> anyhow::Result<()> {
        let json = serde_json::to_string(&msg)?;
        if std::env::var("TEST_DEBUG").is_ok() {
            eprintln!("SEND: {}", json);
        }
        writeln!(self.stdin, "{}", json)?;
        self.stdin.flush()?;
        Ok(())
    }

    /// Receive a message
    pub fn receive_message(&self) -> anyhow::Result<Value> {
        let line = self.rx.recv()?;
        Ok(serde_json::from_str(&line)?)
    }

    /// Wait for the Ready message
    pub fn wait_for_ready(&self) -> anyhow::Result<Value> {
        loop {
            let msg = self.receive_message()?;
            if msg.get("type") == Some(&json!("Ready")) {
                return Ok(msg);
            }
        }
    }

    /// Wait for a specific message type
    pub fn wait_for_message_type(&self, msg_type: &str) -> anyhow::Result<Value> {
        loop {
            let msg = self.receive_message()?;
            if msg.get("type") == Some(&json!(msg_type)) {
                return Ok(msg);
            }
            // Log unexpected messages for debugging
            if std::env::var("TEST_DEBUG").is_ok() {
                eprintln!("Unexpected message type: {:?}", msg.get("type"));
            }
        }
    }

    /// Try to receive any pending message without blocking
    pub fn try_receive_message(&self) -> Option<Value> {
        match self.rx.try_recv() {
            Ok(line) => serde_json::from_str(&line).ok(),
            Err(_) => None,
        }
    }

    /// Drain all pending messages
    pub fn drain_messages(&self) {
        while self.try_receive_message().is_some() {
            // Keep draining
        }
    }

    /// Send a chat message and wait for response
    pub fn send_chat_message(&mut self, content: &str) -> anyhow::Result<Value> {
        self.send_message(json!({
            "type": "Chat",
            "message": content
        }))?;

        // Commands starting with "/" typically generate Event messages with event: "system"
        // Regular chat messages generate Response messages
        if content.starts_with("/") {
            // Wait for system Event message
            loop {
                let msg = self.receive_message()?;
                if msg.get("type") == Some(&json!("Event")) {
                    if msg.get("event") == Some(&json!("system")) {
                        // Convert Event format to Response format for compatibility
                        if let Some(data) = msg.get("data") {
                            if let Some(content) = data.get("content").and_then(|c| c.as_str()) {
                                return Ok(json!({
                                    "type": "Response",
                                    "content": content
                                }));
                            }
                        }
                    }
                } else if msg.get("type") == Some(&json!("Response")) {
                    // Some commands might still return Response
                    return Ok(msg);
                } else if msg.get("type") == Some(&json!("ToolResult")) {
                    // Tool results are what we check for security blocks
                    return Ok(msg);
                }
            }
        } else {
            // Wait for Response
            self.wait_for_message_type("Response")
        }
    }

    /// Create a test file in the workspace
    pub fn create_test_file(&self, name: &str, content: &str) -> anyhow::Result<PathBuf> {
        let file_path = self.temp_dir.join(name);
        std::fs::write(&file_path, content)?;
        Ok(file_path)
    }

    /// Check if a file exists in the workspace
    pub fn file_exists(&self, name: &str) -> bool {
        self.temp_dir.join(name).exists()
    }

    /// Read a file from the workspace
    pub fn read_file(&self, name: &str) -> anyhow::Result<String> {
        Ok(std::fs::read_to_string(self.temp_dir.join(name))?)
    }
}

impl Drop for SubprocessFixture {
    fn drop(&mut self) {
        // Kill the process and wait for it to exit
        let _ = self.process.kill();
        let _ = self.process.wait();

        // Small delay to ensure process cleanup
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Clean up temp directory
        let _ = std::fs::remove_dir_all(&self.temp_dir);
    }
}

/// Helper to check if a message is an error response
pub fn is_error_response(message: &Value) -> bool {
    // Check if the message type is "Error"
    if message.get("type") == Some(&json!("Error")) {
        return true;
    }

    // Check if it's a ToolResult with success: false
    if message.get("type") == Some(&json!("ToolResult")) {
        if message.get("success") == Some(&json!(false)) {
            return true;
        }
    }

    false
}

/// Helper to check if a message is a security block
pub fn is_security_blocked(message: &Value) -> bool {
    // Security blocks come as ToolResult messages with success: false and security-related error
    if message.get("type") == Some(&json!("ToolResult")) {
        if message.get("success") == Some(&json!(false)) {
            if let Some(error) = message.get("error").and_then(|e| e.as_str()) {
                // Check if the error message indicates a security block
                return error.contains("Security:")
                    || error.contains("denied by security")
                    || error.contains("Risk level:");
            }
        }
    }

    false
}

/// Helper to extract security info from a ToolResult security block
pub fn extract_security_info(message: &Value) -> Option<SecurityInfo> {
    if !is_security_blocked(message) {
        return None;
    }

    let tool_name = message.get("tool_name")?.as_str()?.to_string();
    let error = message.get("error")?.as_str()?;

    // Parse the error message for security details
    // Format is typically: "Security: <description>\nRisk level: <level>\nRisk: <explanation>"
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

/// Helper struct for security information
#[derive(Debug, Clone)]
pub struct SecurityInfo {
    pub tool_name: String,
    pub risk_level: String,
    pub description: String,
    pub risk_explanation: Option<String>,
}

/// Helper to parse security status from /security status command response
pub fn parse_security_status(message: &Value) -> Option<SecurityStatus> {
    if message.get("type") != Some(&json!("Response")) {
        return None;
    }

    let content = message.get("content")?.as_str()?;

    // Parse the structured response from /security status
    // Format: "Security Status\n  Mode: <mode>\n..." or "Current security mode: <mode>"
    let mut mode = None;

    for line in content.lines() {
        let trimmed = line.trim();
        // Try parsing from "/security status" format (with optional leading spaces)
        if let Some(mode_str) = trimmed.strip_prefix("Mode: ") {
            mode = Some(mode_str.to_string());
            break;
        }
        // Try parsing from "/security mode" (without args) format
        if trimmed.contains("Current security mode:") {
            // Extract mode from "Current security mode: All" format
            if let Some(mode_str) = trimmed.split(':').nth(1) {
                mode = Some(mode_str.trim().to_string());
                break;
            }
        }
    }

    mode.map(|m| SecurityStatus { mode: m })
}

/// Helper struct for security status
#[derive(Debug, Clone)]
pub struct SecurityStatus {
    pub mode: String,
}
