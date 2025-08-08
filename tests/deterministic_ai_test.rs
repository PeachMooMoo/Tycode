use serde_json::Value;
use tycode::ai::bedrock::BedrockProvider;
use tycode::ai::provider::AiProvider;
use tycode::ai::types::{
    Content, ContentBlock, ConversationRequest, Message, MessageRole, Model, ModelSettings,
};

struct DeterministicAITest {
    ai_provider: BedrockProvider,
}

impl DeterministicAITest {
    async fn new() -> Self {
        let bedrock_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .profile_name("cline")
            .region(aws_config::Region::new("us-west-2"))
            .load()
            .await;
        let bedrock_client = aws_sdk_bedrockruntime::Client::new(&bedrock_config);

        Self {
            ai_provider: BedrockProvider::new(bedrock_client),
        }
    }

    fn get_large_file_content(&self) -> String {
        let mut content = String::new();
        content.push_str("# Large Configuration File\n");
        content.push_str("# This file contains 1000+ lines of configuration\n\n");

        for i in 1..=100 {
            content.push_str(&format!("section_{} = {{\n", i));
            content.push_str(&format!("    enabled = true\n"));
            content.push_str(&format!("    timeout = {}\n", i * 10));
            content.push_str(&format!("    retries = 3\n"));
            content.push_str(&format!("    host = \"server-{}.example.com\"\n", i));
            content.push_str(&format!("    port = {}\n", 8000 + i));
            content.push_str(&format!("    ssl_enabled = false\n"));
            content.push_str(&format!("    max_connections = 100\n"));
            content.push_str(&format!("    buffer_size = 1024\n"));
            content.push_str(&format!("    log_level = \"info\"\n"));
            content.push_str("}\n\n");
        }

        content.push_str("# Special function that needs to be replaced\n");
        content.push_str("def process_data(input_data):\n");
        content.push_str("    if not input_data:\n");
        content.push_str("        return None\n");
        content.push_str("    \n");
        content.push_str("    result = []\n");
        content.push_str("    for item in input_data:\n");
        content.push_str("        if item > 0:\n");
        content.push_str("            result.append(item * 2)\n");
        content.push_str("    return result\n\n");

        for i in 101..=200 {
            content.push_str(&format!("config_{} = \"value_{}\"\n", i, i));
        }

        content
    }

    fn get_expected_replacement(&self) -> &'static str {
        r#"def process_data(input_data):
    """Enhanced data processing with validation and logging."""
    import logging
    
    if not input_data:
        logging.warning("Empty input data provided")
        return []
    
    if not isinstance(input_data, list):
        raise TypeError("Input must be a list")
    
    result = []
    for item in input_data:
        if isinstance(item, (int, float)) and item > 0:
            processed_value = item * 2
            result.append(processed_value)
            logging.debug(f"Processed {item} -> {processed_value}")
    
    logging.info(f"Processed {len(result)} items from {len(input_data)} inputs")
    return result"#
    }

    async fn test_search_replace_precision(&self) -> (bool, String) {
        let file_content = self.get_large_file_content();
        let expected_replacement = self.get_expected_replacement();

        let system_prompt = r#"You are a code refactoring assistant. Use the replace_in_file tool to make the exact changes requested.

Use this format:
{
  "file_path": "config.py",
  "diff": "------- SEARCH\n[exact code to find]\n=======\n[replacement code]\n+++++++ REPLACE"
}

Make sure the search text matches exactly including whitespace and indentation."#;

        let user_message = format!(
            r#"Current file content (1000+ lines):
```
{}
```

Task: Replace the `process_data` function with this enhanced version that includes proper error handling, logging, and type checking:

```python
{}
```

Use the replace_in_file tool to make this exact replacement."#,
            file_content, expected_replacement
        );

        let request = ConversationRequest {
            messages: vec![Message {
                role: MessageRole::User,
                content: Content::text_only(user_message),
            }],
            model: ModelSettings {
                model: Model::ClaudeSonnet4,
                ..ModelSettings::default()
            },
            system_prompt: system_prompt.to_string(),
            stop_sequences: vec![],
            tools: vec![self.get_mock_replace_tool_definition()],
        };

        match self.ai_provider.converse(request).await {
            Ok(response) => {
                for block in response.content.blocks() {
                    if let ContentBlock::ToolUse(tool_use) = block {
                        return self.validate_search_replace_call(
                            &tool_use.arguments,
                            expected_replacement,
                        );
                    }
                }
                (false, "No tool use found in AI response".to_string())
            }
            Err(e) => (false, format!("AI request failed: {:?}", e)),
        }
    }

    async fn test_unified_diff_precision(&self) -> (bool, String) {
        let file_content = self.get_large_file_content();
        let expected_replacement = self.get_expected_replacement();

        let system_prompt = r#"You are a code refactoring assistant. Use the apply_patch tool to make the exact changes requested.

Use this format:
{
  "file_path": "config.py",
  "patch": "--- config.py\n+++ config.py\n@@ -line,count +line,count @@\n context\n-removed\n+added\n context"
}

Make sure to get the line numbers exactly right for the 1000+ line file."#;

        let user_message = format!(
            r#"Current file content (1000+ lines):
```
{}
```

Task: Replace the `process_data` function with this enhanced version:

```python
{}
```

Use the apply_patch tool to make this exact replacement. The function starts around line 1012."#,
            file_content, expected_replacement
        );

        let request = ConversationRequest {
            messages: vec![Message {
                role: MessageRole::User,
                content: Content::text_only(user_message),
            }],
            model: ModelSettings {
                model: Model::ClaudeSonnet4,
                ..ModelSettings::default()
            },
            system_prompt: system_prompt.to_string(),
            stop_sequences: vec![],
            tools: vec![self.get_mock_patch_tool_definition()],
        };

        match self.ai_provider.converse(request).await {
            Ok(response) => {
                for block in response.content.blocks() {
                    if let ContentBlock::ToolUse(tool_use) = block {
                        return self
                            .validate_unified_diff_call(&tool_use.arguments, expected_replacement);
                    }
                }
                (false, "No tool use found in AI response".to_string())
            }
            Err(e) => (false, format!("AI request failed: {:?}", e)),
        }
    }

    fn validate_search_replace_call(
        &self,
        arguments: &Value,
        expected_replacement: &str,
    ) -> (bool, String) {
        let diff = match arguments.get("diff").and_then(|v| v.as_str()) {
            Some(d) => d,
            None => return (false, "No diff parameter found".to_string()),
        };

        let original_function = r#"def process_data(input_data):
    if not input_data:
        return None
    
    result = []
    for item in input_data:
        if item > 0:
            result.append(item * 2)
    return result"#;

        let expected_diff = format!(
            "------- SEARCH\n{}\n=======\n{}\n+++++++ REPLACE",
            original_function, expected_replacement
        );

        if diff.trim() == expected_diff.trim() {
            (
                true,
                "Perfect match - AI generated exact search/replace".to_string(),
            )
        } else {
            let similarity = self.calculate_similarity(diff, &expected_diff);
            (false, format!("Diff mismatch ({}% similar). Expected exact match for deterministic replacement", similarity))
        }
    }

    fn validate_unified_diff_call(
        &self,
        arguments: &Value,
        _expected_replacement: &str,
    ) -> (bool, String) {
        let patch = match arguments.get("patch").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return (false, "No patch parameter found".to_string()),
        };

        let has_correct_line_numbers = patch.contains("@@ -1012,")
            || patch.contains("@@ -1011,")
            || patch.contains("@@ -1013,");
        let contains_replacement =
            patch.contains("import logging") && patch.contains("Enhanced data processing");
        let has_proper_context = patch.contains("# Special function") || patch.contains("config_");

        if has_correct_line_numbers && contains_replacement && has_proper_context {
            (
                true,
                "Unified diff has correct structure and content".to_string(),
            )
        } else {
            let issues = vec![
                if !has_correct_line_numbers {
                    Some("incorrect line numbers")
                } else {
                    None
                },
                if !contains_replacement {
                    Some("missing replacement content")
                } else {
                    None
                },
                if !has_proper_context {
                    Some("missing context lines")
                } else {
                    None
                },
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(", ");

            (false, format!("Unified diff issues: {}", issues))
        }
    }

    fn calculate_similarity(&self, text1: &str, text2: &str) -> u32 {
        let words1: Vec<&str> = text1.split_whitespace().collect();
        let words2: Vec<&str> = text2.split_whitespace().collect();

        let common_words = words1.iter().filter(|word| words2.contains(word)).count();

        let total_words = words1.len().max(words2.len());
        if total_words == 0 {
            100
        } else {
            (common_words * 100 / total_words) as u32
        }
    }

    fn get_mock_replace_tool_definition(&self) -> tycode::ai::types::ToolDefinition {
        tycode::ai::types::ToolDefinition {
            name: "replace_in_file".to_string(),
            description: "Replace content in a file using search and replace blocks".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": {"type": "string"},
                    "diff": {"type": "string"}
                },
                "required": ["file_path", "diff"]
            }),
        }
    }

    fn get_mock_patch_tool_definition(&self) -> tycode::ai::types::ToolDefinition {
        tycode::ai::types::ToolDefinition {
            name: "apply_patch".to_string(),
            description: "Apply a unified diff patch to a file".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": {"type": "string"},
                    "patch": {"type": "string"}
                },
                "required": ["file_path", "patch"]
            }),
        }
    }

    async fn run_deterministic_comparison(&self) {
        println!("\n🎯 DETERMINISTIC AI PRECISION TEST 🎯");
        println!("Testing AI precision on 1000+ line file with exact replacement task\n");

        println!("File: 1000+ line configuration file");
        println!("Task: Replace process_data function with enhanced version");
        println!("Expected: Exact deterministic replacement\n");

        println!("=== Testing Search/Replace Approach ===");
        let (sr_success, sr_message) = self.test_search_replace_precision().await;
        println!(
            "Result: {}",
            if sr_success {
                "✅ SUCCESS"
            } else {
                "❌ FAILED"
            }
        );
        println!("Details: {}\n", sr_message);

        println!("=== Testing Unified Diff Approach ===");
        let (ud_success, ud_message) = self.test_unified_diff_precision().await;
        println!(
            "Result: {}",
            if ud_success {
                "✅ SUCCESS"
            } else {
                "❌ FAILED"
            }
        );
        println!("Details: {}\n", ud_message);

        println!("=== FINAL RESULTS ===");
        println!(
            "Search/Replace: {}",
            if sr_success {
                "✅ PRECISE"
            } else {
                "❌ IMPRECISE"
            }
        );
        println!(
            "Unified Diff:   {}",
            if ud_success {
                "✅ PRECISE"
            } else {
                "❌ IMPRECISE"
            }
        );

        if sr_success && !ud_success {
            println!("\n🏆 Search/Replace approach is more precise for deterministic tasks!");
        } else if ud_success && !sr_success {
            println!("\n🏆 Unified Diff approach is more precise for deterministic tasks!");
        } else if sr_success && ud_success {
            println!("\n🤝 Both approaches achieved the required precision!");
        } else {
            println!("\n⚠️  Both approaches failed to meet precision requirements!");
        }

        assert!(
            sr_success,
            "Search/Replace should achieve deterministic precision"
        );
    }
}

#[tokio::test]
#[ignore]
async fn deterministic_ai_precision_test() {
    let test_suite = DeterministicAITest::new().await;
    test_suite.run_deterministic_comparison().await;
}

#[tokio::test]
#[ignore]
async fn test_search_replace_precision_only() {
    let test_suite = DeterministicAITest::new().await;
    let (success, message) = test_suite.test_search_replace_precision().await;

    println!("Search/Replace Precision Test:");
    println!("Success: {}", success);
    println!("Message: {}", message);

    assert!(
        success,
        "Search/Replace should achieve deterministic precision"
    );
}
