use serde_json::json;

mod fixture;
use fixture::SubprocessFixture;

#[test]
fn test_retry_events_with_mock_provider() -> anyhow::Result<()> {
    // Create settings with mock provider that will fail twice then succeed
    let settings = r#"
active_provider = "mock"

[providers.mock]
type = "mock"
behavior = { retry_then_success = { errors_before_success = 2 } }

[security]
mode = "all"
"#;

    let mut fixture = SubprocessFixture::with_settings(Some(settings))?;

    // Wait for ready
    fixture.wait_for_ready()?;
    
    // Send a chat message that will trigger the mock provider
    fixture.send_message(json!({
        "type": "Chat",
        "message": "Hello, test the retry mechanism"
    }))?;

    // We expect to receive retry events for attempts 1 and 2
    let mut retry_events = Vec::new();
    let mut response_received = false;
    
    // Collect messages until we get the final response
    for _ in 0..10 {  // Max 10 messages to prevent infinite loop
        let msg = fixture.receive_message()?;
        
        if msg.get("type") == Some(&json!("Event")) {
            if let Some(event) = msg.get("event").and_then(|e| e.as_str()) {
                if event == "retry_attempt" {
                    retry_events.push(msg);
                }
            }
        } else if msg.get("type") == Some(&json!("Response")) {
            response_received = true;
            
            // Verify the response is successful after retries
            let content = msg.get("content")
                .and_then(|c| c.as_str())
                .unwrap_or("");
            assert!(content.contains("Success after retries"), 
                    "Expected success message after retries, got: {}", content);
            break;
        }
    }
    
    // Verify we received retry events
    assert_eq!(retry_events.len(), 2, "Expected 2 retry events, got: {}", retry_events.len());
    
    // Verify retry event structure
    for (i, retry_event) in retry_events.iter().enumerate() {
        let data = retry_event.get("data")
            .expect("Retry event should have data field");
        
        // Check attempt number (1-indexed)
        let attempt = data.get("attempt")
            .and_then(|a| a.as_u64())
            .expect("Retry event should have attempt number");
        assert_eq!(attempt, (i + 1) as u64, "Retry attempt number mismatch");
        
        // Check max_retries
        let max_retries = data.get("max_retries")
            .and_then(|m| m.as_u64())
            .expect("Retry event should have max_retries");
        assert!(max_retries >= 2, "Max retries should be at least 2");
        
        // Check error message
        let error = data.get("error")
            .and_then(|e| e.as_str())
            .expect("Retry event should have error message");
        assert!(error.contains("Mock retryable error"), 
                "Expected mock error message, got: {}", error);
        
        // Check backoff_ms exists
        let backoff = data.get("backoff_ms")
            .and_then(|b| b.as_u64())
            .expect("Retry event should have backoff_ms");
        assert!(backoff > 0, "Backoff should be positive");
    }
    
    assert!(response_received, "Should have received a response after retries");
    
    Ok(())
}

#[test]
fn test_non_retryable_error_no_retry_events() -> anyhow::Result<()> {
    // Create settings with mock provider that always fails with non-retryable error
    let settings = r#"
active_provider = "mock"

[providers.mock]
type = "mock"
behavior = "always_error"

[security]
mode = "all"
"#;

    let mut fixture = SubprocessFixture::with_settings(Some(settings))?;

    // Wait for ready
    fixture.wait_for_ready()?;
    
    // Send a chat message that will trigger the mock provider
    fixture.send_message(json!({
        "type": "Chat",
        "message": "Hello, test non-retryable error"
    }))?;

    // We should NOT receive any retry events, just an error
    let mut retry_events = Vec::new();
    let mut error_received = false;
    
    // Collect messages until we get an error response
    for _ in 0..5 {  // Max 5 messages to prevent infinite loop
        let msg = fixture.receive_message()?;
        
        if msg.get("type") == Some(&json!("Event")) {
            if let Some(event) = msg.get("event").and_then(|e| e.as_str()) {
                if event == "retry_attempt" {
                    retry_events.push(msg);
                }
            }
        } else if msg.get("type") == Some(&json!("Error")) || msg.get("type") == Some(&json!("Response")) {
            error_received = true;
            
            // If it's a Response type, check that it contains error information
            if msg.get("type") == Some(&json!("Response")) {
                let content = msg.get("content")
                    .and_then(|c| c.as_str())
                    .unwrap_or("");
                // The response should contain error indication
                assert!(content.contains("error") || content.contains("Error") || content.contains("failed"),
                        "Expected error message in response, got: {}", content);
            }
            break;
        }
    }
    
    // Verify we did NOT receive retry events (non-retryable errors don't retry)
    assert_eq!(retry_events.len(), 0, 
               "Expected 0 retry events for non-retryable error, got: {}", retry_events.len());
    
    assert!(error_received, "Should have received an error response");
    
    Ok(())
}

#[test]
fn test_max_retries_exhausted() -> anyhow::Result<()> {
    // Create settings with mock provider that always fails with retryable error
    let settings = r#"
active_provider = "mock"

[providers.mock]
type = "mock"
behavior = "always_retry_error"

[security]
mode = "all"
"#;

    let mut fixture = SubprocessFixture::with_settings(Some(settings))?;

    // Wait for ready
    fixture.wait_for_ready()?;
    
    // Send a chat message that will trigger the mock provider
    fixture.send_message(json!({
        "type": "Chat",
        "message": "Hello, test max retries"
    }))?;

    // We should receive retry events up to max_retries (3), then an error
    let mut retry_events = Vec::new();
    let mut final_error_received = false;
    const EXPECTED_MAX_RETRIES: usize = 3;
    
    // Collect messages until we get the final error
    for _ in 0..20 {  // Allow more messages for multiple retries
        let msg = fixture.receive_message()?;
        
        if msg.get("type") == Some(&json!("Event")) {
            if let Some(event) = msg.get("event").and_then(|e| e.as_str()) {
                if event == "retry_attempt" {
                    retry_events.push(msg);
                }
            }
        } else if msg.get("type") == Some(&json!("Error")) || msg.get("type") == Some(&json!("Response")) {
            final_error_received = true;
            
            // Verify this is an error (not success)
            if msg.get("type") == Some(&json!("Response")) {
                let content = msg.get("content")
                    .and_then(|c| c.as_str())
                    .unwrap_or("");
                // Should indicate failure after exhausting retries
                assert!(content.contains("error") || content.contains("Error") || 
                        content.contains("failed") || content.contains("retry"),
                        "Expected error message after exhausting retries, got: {}", content);
            }
            break;
        }
    }
    
    // Verify we received the expected number of retry events
    assert!(retry_events.len() > 0, "Should have received at least one retry event");
    assert_eq!(retry_events.len(), EXPECTED_MAX_RETRIES, 
               "Should have received {} retry events (max_retries), got: {}", 
               EXPECTED_MAX_RETRIES, retry_events.len());
    
    // Verify each retry event has the correct structure
    for (i, retry_event) in retry_events.iter().enumerate() {
        let data = retry_event.get("data")
            .expect("Retry event should have data field");
        
        // Check attempt number (1-indexed)
        let attempt = data.get("attempt")
            .and_then(|a| a.as_u64())
            .expect("Retry event should have attempt number");
        assert_eq!(attempt, (i + 1) as u64, "Retry attempt number mismatch");
        
        // Check max_retries
        let max_retries = data.get("max_retries")
            .and_then(|m| m.as_u64())
            .expect("Retry event should have max_retries");
        assert_eq!(max_retries, EXPECTED_MAX_RETRIES as u64, "Max retries mismatch");
        
        // Check error message
        let error = data.get("error")
            .and_then(|e| e.as_str())
            .expect("Retry event should have error message");
        assert!(error.contains("Mock retryable error"), 
                "Expected mock error message, got: {}", error);
        
        // Check backoff_ms exists
        let backoff = data.get("backoff_ms")
            .and_then(|b| b.as_u64())
            .expect("Retry event should have backoff_ms");
        assert!(backoff > 0, "Backoff should be positive");
    }
    
    assert!(final_error_received, "Should have received final error after exhausting retries");
    
    Ok(())
}