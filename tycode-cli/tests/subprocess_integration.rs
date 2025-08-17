use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[derive(Debug)]
struct SubprocessTest {
    process: std::process::Child,
    stdin: std::process::ChildStdin,
    rx: mpsc::Receiver<String>,
    temp_dir: PathBuf,
    settings_path: PathBuf,
}

impl SubprocessTest {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Create a unique temp directory with random component
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis();
        let random: u32 = std::process::id();
        let thread_id = format!("{:?}", std::thread::current().id()).replace("ThreadId(", "").replace(")", "");
        let temp_dir = std::env::temp_dir().join(format!("tycode_test_{}_{}{}", timestamp, random, thread_id));
        std::fs::create_dir_all(&temp_dir)?;
        let settings_path = temp_dir.join("settings.toml");

        // Create initial settings file with a default provider
        let initial_settings = r#"
active_provider = "default"

[providers.default]
type = "bedrock"
profile = "default"
region = "us-east-1"
"#;
        std::fs::write(&settings_path, initial_settings)?;

        let mut process = Command::new("cargo")
            .args(&[
                "run", 
                "--bin", 
                "tycode", 
                "--", 
                "--subprocess",
                "--settings-path",
                settings_path.to_str().unwrap()
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        let stdin = process.stdin.take().expect("Failed to get stdin");
        let stdout = process.stdout.take().expect("Failed to get stdout");

        let (tx, rx) = mpsc::channel();

        // Spawn thread to read stdout
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    let _ = tx.send(line);
                }
            }
        });

        Ok(SubprocessTest {
            process,
            stdin,
            rx,
            temp_dir,
            settings_path,
        })
    }

    fn send_message(&mut self, msg: Value) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string(&msg)?;
        writeln!(self.stdin, "{}", json)?;
        self.stdin.flush()?;
        Ok(())
    }

    fn receive_message(&self, timeout: Duration) -> Result<Value, Box<dyn std::error::Error>> {
        let line = self.rx.recv_timeout(timeout)?;
        Ok(serde_json::from_str(&line)?)
    }

    fn wait_for_ready(&self) -> Result<Value, Box<dyn std::error::Error>> {
        // Wait for the Ready message
        for _ in 0..10 {
            match self.receive_message(Duration::from_secs(5)) {
                Ok(msg) => {
                    if msg.get("type") == Some(&json!("Ready")) {
                        return Ok(msg);
                    }
                }
                Err(_) => continue,
            }
        }
        Err("Timeout waiting for Ready message".into())
    }

    fn wait_for_message_type(&self, msg_type: &str) -> Result<Value, Box<dyn std::error::Error>> {
        for _ in 0..10 {
            match self.receive_message(Duration::from_secs(2)) {
                Ok(msg) => {
                    if msg.get("type") == Some(&json!(msg_type)) {
                        return Ok(msg);
                    }
                    // Log unexpected messages for debugging
                    eprintln!("Unexpected message: {:?}", msg);
                }
                Err(e) => {
                    eprintln!("Error receiving message: {:?}", e);
                    return Err(e.into());
                }
            }
        }
        Err(format!("Timeout waiting for {} message", msg_type).into())
    }
}

impl Drop for SubprocessTest {
    fn drop(&mut self) {
        let _ = self.process.kill();
        // Clean up temp directory
        let _ = std::fs::remove_dir_all(&self.temp_dir);
    }
}

#[test]
fn test_subprocess_ready_message() -> Result<(), Box<dyn std::error::Error>> {
    let test = SubprocessTest::new()?;

    let ready_msg = test.wait_for_ready()?;
    assert_eq!(ready_msg.get("type"), Some(&json!("Ready")));

    // Should have initial settings
    let settings = ready_msg
        .get("settings")
        .expect("Ready message should have settings");
    let providers = settings
        .get("providers")
        .expect("Settings should have providers");
    assert!(
        providers.get("default").is_some(),
        "Should have default provider"
    );

    Ok(())
}

#[test]
fn test_settings_persistence_fixed() -> Result<(), Box<dyn std::error::Error>> {
    let mut test = SubprocessTest::new()?;
    
    // Wait for ready
    test.wait_for_ready()?;
    
    // Save settings with a new provider
    let new_settings = json!({
        "active_provider": "default",
        "providers": {
            "default": {
                "type": "bedrock",
                "profile": "default",
                "region": "us-east-1"
            },
            "disk_test": {
                "type": "bedrock",
                "profile": "disk-test-profile",
                "region": "ap-southeast-1"
            }
        }
    });
    
    test.send_message(json!({
        "type": "SaveSettings",
        "settings": new_settings
    }))?;
    
    let save_response = test.wait_for_message_type("SettingsSaved")?;
    assert_eq!(save_response.get("success"), Some(&json!(true)), "Save should succeed");
    
    // Send reload to ensure settings are fresh from disk
    test.send_message(json!({
        "type": "ReloadSettings"
    }))?;
    
    // Wait a bit for reload to complete
    thread::sleep(Duration::from_millis(100));
    
    // Now load settings and verify they were persisted
    test.send_message(json!({
        "type": "LoadSettings"
    }))?;
    
    let load_response = test.wait_for_message_type("SettingsLoaded")?;
    let settings = load_response.get("settings").expect("Should have settings");
    let providers = settings.get("providers").expect("Should have providers");
    
    assert!(
        providers.get("disk_test").is_some(), 
        "Subprocess should return disk_test provider after save - providers: {:?}",
        providers.as_object().map(|m| m.keys().collect::<Vec<_>>())
    );
    
    Ok(())
}

#[test]
fn test_save_and_load_settings() -> Result<(), Box<dyn std::error::Error>> {
    let mut test = SubprocessTest::new()?;

    // Wait for ready
    test.wait_for_ready()?;

    // Create new settings with an additional provider
    let new_settings = json!({
        "active_provider": "default",
        "providers": {
            "default": {
                "type": "bedrock",
                "profile": "default",
                "region": "us-east-1"
            },
            "asdf": {
                "type": "bedrock",
                "profile": "asdf-profile",
                "region": "us-west-2"
            }
        }
    });

    // Save the new settings
    test.send_message(json!({
        "type": "SaveSettings",
        "settings": new_settings
    }))?;

    // Wait for save confirmation
    let save_response = test.wait_for_message_type("SettingsSaved")?;
    assert_eq!(save_response.get("success"), Some(&json!(true)));

    // Now load settings and verify the new provider is there
    test.send_message(json!({
        "type": "LoadSettings"
    }))?;

    let load_response = test.wait_for_message_type("SettingsLoaded")?;
    let settings = load_response.get("settings").expect("Should have settings");
    let providers = settings.get("providers").expect("Should have providers");

    assert!(
        providers.get("default").is_some(),
        "Should have default provider"
    );
    assert!(
        providers.get("asdf").is_some(),
        "Should have newly added asdf provider"
    );

    Ok(())
}

#[test]
fn test_reload_settings_after_save() -> Result<(), Box<dyn std::error::Error>> {
    let mut test = SubprocessTest::new()?;

    // Wait for ready
    test.wait_for_ready()?;

    // Save settings with new provider
    let new_settings = json!({
        "active_provider": "default",
        "providers": {
            "default": {
                "type": "bedrock",
                "profile": "default",
                "region": "us-east-1"
            },
            "test_provider": {
                "type": "bedrock",
                "profile": "test-profile",
                "region": "eu-west-1"
            }
        }
    });

    test.send_message(json!({
        "type": "SaveSettings",
        "settings": new_settings
    }))?;

    let save_response = test.wait_for_message_type("SettingsSaved")?;
    assert_eq!(save_response.get("success"), Some(&json!(true)));

    // Send ReloadSettings to ensure subprocess refreshes from disk
    test.send_message(json!({
        "type": "ReloadSettings"
    }))?;

    // Small delay to ensure reload completes
    thread::sleep(Duration::from_millis(100));

    // Load settings and verify
    test.send_message(json!({
        "type": "LoadSettings"
    }))?;

    let load_response = test.wait_for_message_type("SettingsLoaded")?;
    let settings = load_response.get("settings").expect("Should have settings");
    let providers = settings.get("providers").expect("Should have providers");

    assert!(
        providers.get("test_provider").is_some(),
        "Should have test_provider after reload"
    );

    Ok(())
}

#[test]
fn test_multiple_load_after_save() -> Result<(), Box<dyn std::error::Error>> {
    let mut test = SubprocessTest::new()?;

    // Wait for ready
    test.wait_for_ready()?;

    // Save settings with new provider "asdf" (the exact scenario from the bug)
    let new_settings = json!({
        "active_provider": "default",
        "providers": {
            "default": {
                "type": "bedrock",
                "profile": "default",
                "region": "us-east-1"
            },
            "asdf": {
                "type": "bedrock",
                "profile": "asdf-profile",
                "region": "us-east-1"
            }
        }
    });

    test.send_message(json!({
        "type": "SaveSettings",
        "settings": new_settings
    }))?;

    let save_response = test.wait_for_message_type("SettingsSaved")?;
    assert_eq!(save_response.get("success"), Some(&json!(true)));

    // Simulate clicking refresh multiple times
    for i in 0..3 {
        eprintln!("Load attempt {}", i + 1);

        test.send_message(json!({
            "type": "LoadSettings"
        }))?;

        let load_response = test.wait_for_message_type("SettingsLoaded")?;
        let settings = load_response.get("settings").expect("Should have settings");
        let providers = settings.get("providers").expect("Should have providers");

        // This should pass but likely won't due to the bug
        assert!(
            providers.get("asdf").is_some(),
            "Should have asdf provider on attempt {} - providers: {:?}",
            i + 1,
            providers.as_object().map(|m| m.keys().collect::<Vec<_>>())
        );
    }

    Ok(())
}

#[test]
fn test_settings_persistence() -> Result<(), Box<dyn std::error::Error>> {
    let mut test = SubprocessTest::new()?;

    // Wait for ready
    test.wait_for_ready()?;

    // Verify the settings file on disk after save
    let new_settings = json!({
        "active_provider": "default",
        "providers": {
            "default": {
                "type": "bedrock",
                "profile": "default",
                "region": "us-east-1"
            },
            "disk_test": {
                "type": "bedrock",
                "profile": "disk-test-profile",
                "region": "ap-southeast-1"
            }
        }
    });

    test.send_message(json!({
        "type": "SaveSettings",
        "settings": new_settings
    }))?;

    test.wait_for_message_type("SettingsSaved")?;

    // Read the actual file from disk to verify it was written
    let disk_contents = std::fs::read_to_string(&test.settings_path)?;
    eprintln!("Settings file on disk:\n{}", disk_contents);
    assert!(
        disk_contents.contains("disk_test"),
        "Settings file should contain disk_test provider"
    );

    // Now load through subprocess and verify it matches disk
    test.send_message(json!({
        "type": "LoadSettings"
    }))?;

    let load_response = test.wait_for_message_type("SettingsLoaded")?;
    let settings = load_response.get("settings").expect("Should have settings");
    let providers = settings.get("providers").expect("Should have providers");

    assert!(
        providers.get("disk_test").is_some(),
        "Subprocess should return disk_test provider that exists on disk"
    );

    Ok(())
}
