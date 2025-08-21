mod fixture;

use fixture::{parse_security_status, SubprocessFixture};
use ntest::timeout;

#[test]
#[timeout(5000)]
fn test_security_default_mode() -> anyhow::Result<()> {
    let mut fixture = SubprocessFixture::new()?;
    fixture.wait_for_ready()?;
    println!("ready");

    // Check default security mode
    let response = fixture.send_chat_message("/security status")?;
    let status = parse_security_status(&response);
    assert_eq!(
        status.map(|s| s.mode),
        Some("Auto".to_string()),
        "Default mode should be Auto"
    );

    Ok(())
}



#[test]
#[timeout(5000)]
fn test_security_mode_invalid_input() -> anyhow::Result<()> {
    let mut fixture = SubprocessFixture::new()?;
    fixture.wait_for_ready()?;

    // Try invalid mode
    let response = fixture.send_chat_message("/security mode invalid_mode")?;
    let response_text = response
        .get("content")
        .and_then(|r| r.as_str())
        .unwrap_or("");
    assert!(
        response_text.contains("Unknown security mode") || response_text.contains("invalid"),
        "Should reject invalid security mode"
    );

    // Mode should remain unchanged (Auto)
    let response = fixture.send_chat_message("/security status")?;
    let status = parse_security_status(&response);
    assert_eq!(
        status.map(|s| s.mode),
        Some("Auto".to_string()),
        "Mode should remain Auto after invalid input"
    );

    Ok(())
}

