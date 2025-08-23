mod fixture;

use fixture::SubprocessFixture;

#[test]
fn test_retry_events_with_mock_provider() -> anyhow::Result<()> {
    let settings = r#"
active_provider = "mock"

[providers.mock]
type = "mock"
behavior = { retry_then_success = { errors_before_success = 2 } }

[security]
mode = "all"
"#;

    let mut fixture = SubprocessFixture::with_settings(Some(settings))?;
    fixture.wait_for_ready_typed()?;
    fixture.send_chat("Hello, test the retry mechanism")?;

    let (retry_events, response) = fixture.collect_retry_events_and_response()?;
    
    assert_eq!(retry_events.len(), 2, "Expected 2 retry events, got: {}", retry_events.len());
    
    for (i, retry_data) in retry_events.iter().enumerate() {
        assert_eq!(retry_data.attempt, (i + 1) as u64, "Retry attempt number mismatch");
        assert!(retry_data.max_retries >= 2, "Max retries should be at least 2");
        assert!(retry_data.error.contains("Mock retryable error"), 
                "Expected mock error message, got: {}", retry_data.error);
        assert!(retry_data.backoff_ms > 0, "Backoff should be positive");
    }
    
    let content = response.as_response()
        .expect("Should have received a response");
    assert!(content.contains("Success after retries"), 
            "Expected success message after retries, got: {}", content);
    
    Ok(())
}

#[test]
fn test_non_retryable_error_no_retry_events() -> anyhow::Result<()> {
    let settings = r#"
active_provider = "mock"

[providers.mock]
type = "mock"
behavior = "always_error"

[security]
mode = "all"
"#;

    let mut fixture = SubprocessFixture::with_settings(Some(settings))?;
    fixture.wait_for_ready_typed()?;
    fixture.send_chat("Hello, test non-retryable error")?;

    let (retry_events, response) = fixture.collect_retry_events_and_response()?;
    
    assert_eq!(retry_events.len(), 0, 
               "Expected 0 retry events for non-retryable error, got: {}", retry_events.len());
    
    let content = response.as_response()
        .or_else(|| response.as_error())
        .expect("Should have received a response or error");
    
    assert!(content.contains("error") || content.contains("Error") || content.contains("failed"),
            "Expected error message in response, got: {}", content);
    
    Ok(())
}

#[test]
fn test_max_retries_exhausted() -> anyhow::Result<()> {
    let settings = r#"
active_provider = "mock"

[providers.mock]
type = "mock"
behavior = "always_retry_error"

[security]
mode = "all"
"#;

    let mut fixture = SubprocessFixture::with_settings(Some(settings))?;
    fixture.wait_for_ready_typed()?;
    fixture.send_chat("Hello, test max retries")?;

    let (retry_events, response) = fixture.collect_retry_events_and_response()?;
    
    const EXPECTED_MAX_RETRIES: usize = 3;
    
    assert!(retry_events.len() > 0, "Should have received at least one retry event");
    assert_eq!(retry_events.len(), EXPECTED_MAX_RETRIES, 
               "Should have received {} retry events (max_retries), got: {}", 
               EXPECTED_MAX_RETRIES, retry_events.len());
    
    for (i, retry_data) in retry_events.iter().enumerate() {
        assert_eq!(retry_data.attempt, (i + 1) as u64, "Retry attempt number mismatch");
        assert_eq!(retry_data.max_retries, EXPECTED_MAX_RETRIES as u64, "Max retries mismatch");
        assert!(retry_data.error.contains("Mock retryable error"), 
                "Expected mock error message, got: {}", retry_data.error);
        assert!(retry_data.backoff_ms > 0, "Backoff should be positive");
    }
    
    let content = response.as_response()
        .or_else(|| response.as_error())
        .expect("Should have received a response or error");
    
    assert!(content.contains("error") || content.contains("Error") || 
            content.contains("failed") || content.contains("retry"),
            "Expected error message after exhausting retries, got: {}", content);
    
    Ok(())
}