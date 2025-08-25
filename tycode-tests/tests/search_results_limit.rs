mod fixture;
use fixture::*;
use std::fs;

#[test]
fn test_search_results_limit_prevents_context_explosion() -> anyhow::Result<()> {
    let settings = r#"
active_provider = "mock"

[providers.mock]
type = "mock"
behavior = { tool_use = { tool_name = "search_files", tool_arguments = "{\"directory_path\": \".\", \"pattern\": \"test_pattern\", \"max_results\": 10}" } }

[security]
mode = "all"
"#;

    let mut fixture = SubprocessFixture::with_settings(Some(settings))?;
    fixture.wait_for_ready_typed()?;

    // Create 50 test files with matching content
    for i in 0..50 {
        let file_path = fixture.temp_dir.join(format!("test_file_{:03}.txt", i));
        fs::write(
            &file_path,
            "test_pattern: This is a line that matches\nAnother line\ntest_pattern: Another match\nMore content here\n",
        )?;
    }

    // Send message that will trigger the mock provider to use search_files tool
    fixture.send_chat("Search for test_pattern")?;

    // Collect messages to see the tool execution
    let mut tool_executed = false;
    let mut result_count = 0;

    for _ in 0..20 {
        match fixture.try_receive() {
            Some(msg) => {
                println!("Received message: {:?}", msg);
                if let Some(tool_result) = msg.as_tool_result() {
                    println!("Tool result: {:?}", tool_result);
                    if tool_result.0 == "search_files" {
                        tool_executed = true;
                        // Parse the result to check count
                        if let Some(content) = tool_result.2 {
                            if let Some(count_start) = content.find("\"count\":") {
                                let count_substr = &content[count_start + 8..];
                                if let Some(end) = count_substr.find(|c: char| !c.is_ascii_digit())
                                {
                                    let count_str = &count_substr[..end];
                                    if let Ok(count) = count_str.parse::<usize>() {
                                        result_count = count;
                                    }
                                }
                            }
                        }
                        // Break after we've seen the tool execution
                        break;
                    }
                }
            }
            None => std::thread::sleep(std::time::Duration::from_millis(100)),
        }
    }

    assert!(tool_executed, "search_files tool should have been executed");
    assert!(
        result_count <= 10,
        "Results should be limited to max_results=10 (got {})",
        result_count
    );

    Ok(())
}

#[test]
fn test_subprocess_search_with_explicit_max_results() -> anyhow::Result<()> {
    // Test that when the mock provider calls search_files with max_results,
    // the results are properly limited
    let settings = r#"
active_provider = "mock"

[providers.mock]
type = "mock"
behavior = { tool_use = { tool_name = "search_files", tool_arguments = "{\"directory_path\": \".\", \"pattern\": \"test\", \"max_results\": 30}" } }

[security]
mode = "all"
"#;

    let mut fixture = SubprocessFixture::with_settings(Some(settings))?;
    fixture.wait_for_ready_typed()?;

    // Create 40 test files
    for i in 0..40 {
        let file_path = fixture.temp_dir.join(format!("match_{:02}.txt", i));
        fs::write(
            &file_path,
            "test content here\nmore test lines\ntest everywhere\n",
        )?;
    }

    // Trigger the search via mock provider
    fixture.send_chat("Run search")?;

    // Wait for tool execution and response
    let mut tool_result_received = false;
    let mut result_count = 0;

    for _ in 0..20 {
        match fixture.try_receive() {
            Some(msg) => {
                if let Some(tool_result) = msg.as_tool_result() {
                    if tool_result.0 == "search_files" && tool_result.1 {
                        tool_result_received = true;
                        // Parse result for count
                        if let Some(content) = tool_result.2 {
                            if let Some(count_start) = content.find("\"count\":") {
                                let count_substr = &content[count_start + 8..];
                                if let Some(end) = count_substr.find(|c: char| !c.is_ascii_digit())
                                {
                                    if let Ok(count) = count_substr[..end].parse::<usize>() {
                                        result_count = count;
                                    }
                                }
                            }
                        }
                        // Break after we've seen the tool execution
                        break;
                    }
                }
            }
            None => std::thread::sleep(std::time::Duration::from_millis(100)),
        }
    }

    assert!(
        tool_result_received,
        "Should have received search_files tool result"
    );
    assert!(
        result_count <= 30,
        "Results should be limited to max_results=30 (got {})",
        result_count
    );

    Ok(())
}

#[test]
fn test_search_without_max_results_uses_default() -> anyhow::Result<()> {
    // Test that when max_results is not specified, the default limit (100) is used
    let settings = r#"
active_provider = "mock"

[providers.mock]
type = "mock"
behavior = { tool_use = { tool_name = "search_files", tool_arguments = "{\"directory_path\": \".\", \"pattern\": \"content\"}" } }

[security]
mode = "all"
"#;

    let mut fixture = SubprocessFixture::with_settings(Some(settings))?;
    fixture.wait_for_ready_typed()?;

    // Create 150 files to exceed default limit
    for i in 0..150 {
        let file_path = fixture.temp_dir.join(format!("file_{:03}.txt", i));
        fs::write(&file_path, "content\n")?;
    }

    // Trigger search without max_results
    fixture.send_chat("Search files")?;

    // Wait for tool execution
    let mut result_count = 0;
    let mut tool_executed = false;

    for _ in 0..20 {
        match fixture.try_receive() {
            Some(msg) => {
                if let Some(tool_result) = msg.as_tool_result() {
                    if tool_result.0 == "search_files" {
                        tool_executed = true;
                        if let Some(content) = tool_result.2 {
                            // Extract count from result
                            if let Some(count_start) = content.find("\"count\":") {
                                let count_substr = &content[count_start + 8..];
                                if let Some(end) = count_substr.find(|c: char| !c.is_ascii_digit())
                                {
                                    if let Ok(count) = count_substr[..end].parse::<usize>() {
                                        result_count = count;
                                    }
                                }
                            }
                        }
                        // Break after we've seen the tool execution
                        break;
                    }
                }
            }
            None => std::thread::sleep(std::time::Duration::from_millis(100)),
        }
    }

    assert!(tool_executed, "search_files tool should have been executed");
    assert!(
        result_count <= 100,
        "Default max_results should be 100 (got {})",
        result_count
    );

    Ok(())
}
