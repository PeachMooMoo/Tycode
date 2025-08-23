mod fixture;

use fixture::SubprocessFixture;
use ntest::timeout;
use serde_json::json;
use std::thread;
use std::time::Duration;

#[test]
#[timeout(5000)]
fn test_subprocess_ready_message() -> anyhow::Result<()> {
    let fixture = SubprocessFixture::new()?;

    let ready_msg = fixture.wait_for_ready()?;
    assert_eq!(ready_msg.get("type"), Some(&json!("Ready")));

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
#[timeout(5000)]
fn test_settings_persistence_fixed() -> anyhow::Result<()> {
    let mut fixture = SubprocessFixture::new()?;
    fixture.wait_for_ready()?;
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

    fixture.send_message(json!({
        "type": "SaveSettings",
        "settings": new_settings
    }))?;

    let save_response = fixture.wait_for_message_type("SettingsSaved")?;
    assert_eq!(
        save_response.get("success"),
        Some(&json!(true)),
        "Save should succeed"
    );

    fixture.send_message(json!({
        "type": "ReloadSettings"
    }))?;

    fixture.send_message(json!({
        "type": "LoadSettings"
    }))?;

    let load_response = fixture.wait_for_message_type("SettingsLoaded")?;
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
#[timeout(5000)]
fn test_save_and_load_settings() -> anyhow::Result<()> {
    let mut fixture = SubprocessFixture::new()?;
    fixture.wait_for_ready()?;
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

    fixture.send_message(json!({
        "type": "SaveSettings",
        "settings": new_settings
    }))?;

    let save_response = fixture.wait_for_message_type("SettingsSaved")?;
    assert_eq!(save_response.get("success"), Some(&json!(true)));

    fixture.send_message(json!({
        "type": "LoadSettings"
    }))?;

    let load_response = fixture.wait_for_message_type("SettingsLoaded")?;
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
#[timeout(5000)]
fn test_reload_settings_after_save() -> anyhow::Result<()> {
    let mut fixture = SubprocessFixture::new()?;
    fixture.wait_for_ready()?;
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

    fixture.send_message(json!({
        "type": "SaveSettings",
        "settings": new_settings
    }))?;

    let save_response = fixture.wait_for_message_type("SettingsSaved")?;
    assert_eq!(save_response.get("success"), Some(&json!(true)));

    fixture.send_message(json!({
        "type": "ReloadSettings"
    }))?;

    thread::sleep(Duration::from_millis(100));

    fixture.send_message(json!({
        "type": "LoadSettings"
    }))?;

    let load_response = fixture.wait_for_message_type("SettingsLoaded")?;
    let settings = load_response.get("settings").expect("Should have settings");
    let providers = settings.get("providers").expect("Should have providers");

    assert!(
        providers.get("test_provider").is_some(),
        "Should have test_provider after reload"
    );

    Ok(())
}

#[test]
#[timeout(5000)]
fn test_multiple_load_after_save() -> anyhow::Result<()> {
    let mut fixture = SubprocessFixture::new()?;
    fixture.wait_for_ready()?;
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

    fixture.send_message(json!({
        "type": "SaveSettings",
        "settings": new_settings
    }))?;

    let save_response = fixture.wait_for_message_type("SettingsSaved")?;
    assert_eq!(save_response.get("success"), Some(&json!(true)));

    for i in 0..3 {
        eprintln!("Load attempt {}", i + 1);

        fixture.send_message(json!({
            "type": "LoadSettings"
        }))?;

        let load_response = fixture.wait_for_message_type("SettingsLoaded")?;
        let settings = load_response.get("settings").expect("Should have settings");
        let providers = settings.get("providers").expect("Should have providers");

        assert!(
            providers.get("asdf").is_some(),
            "Should have asdf provider on attempt {} - providers: {:?}",
            i + 1,
            providers.as_object().map(|m| m.keys().collect::<Vec<_>>())
        );
    }

    Ok(())
}

