use std::fs;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::time::sleep;
use tycode::ai::bedrock::BedrockProvider;
use tycode::ai::provider::AiProvider;
use tycode::ai::types::{
    Content, ContentBlock, ConversationRequest, Message, MessageRole, Model, ModelTunings,
};
use tycode::tools::file::apply_patch::ApplyPatchTool;
use tycode::tools::file::replace_in_file::ReplaceInFileTool;
use tycode::tools::r#trait::ToolExecutor;

struct RealAITestSuite {
    temp_dir: TempDir,
    search_replace_tool: ReplaceInFileTool,
    patch_tool: ApplyPatchTool,
    ai_provider: BedrockProvider,
}

impl RealAITestSuite {
    async fn new() -> Self {
        let temp_dir = TempDir::new().unwrap();
        let workspace_root = temp_dir.path().to_path_buf();

        let bedrock_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .profile_name("cline")
            .region(aws_config::Region::new("us-west-2"))
            .load()
            .await;
        let bedrock_client = aws_sdk_bedrockruntime::Client::new(&bedrock_config);

        Self {
            temp_dir,
            search_replace_tool: ReplaceInFileTool::new(workspace_root.clone()),
            patch_tool: ApplyPatchTool::new(workspace_root),
            ai_provider: BedrockProvider::new(bedrock_client),
        }
    }

    fn get_initial_rust_code(&self) -> &'static str {
        r#"use std::collections::HashMap;

pub struct Calculator {
    history: Vec<f64>,
}

impl Calculator {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
        }
    }

    pub fn add(&self, a: f64, b: f64) -> f64 {
        a + b
    }

    pub fn multiply(&self, a: f64, b: f64) -> f64 {
        a * b
    }

    pub fn get_history(&self) -> &Vec<f64> {
        &self.history
    }
}

pub fn main() {
    let calc = Calculator::new();
    let result = calc.add(5.0, 3.0);
    println!("Result: {}", result);
}"#
    }

    fn get_editing_tasks(&self) -> Vec<(&'static str, fn(&str) -> bool)> {
        vec![
            ("Add error handling to the add method - it should return Result<f64, String> and check for infinite values", Self::validate_task1),
            ("Add a subtract method that also returns Result<f64, String> with the same error handling", Self::validate_task2),
            ("Modify the Calculator struct to store calculation history - each operation should record the result", Self::validate_task3),
            ("Add a clear_history method and modify get_history to return a slice instead of reference to Vec", Self::validate_task4),
            ("Update the main function to use the new error handling and print the history after calculations", Self::validate_task5)
        ]
    }

    fn validate_task1(code: &str) -> bool {
        code.contains("pub fn add(&")
            && code.contains("Result<f64, String>")
            && code.contains("is_infinite")
    }

    fn validate_task2(code: &str) -> bool {
        code.contains("pub fn subtract(")
            && code.contains("Result<f64, String>")
            && Self::validate_task1(code)
    }

    fn validate_task3(code: &str) -> bool {
        code.contains("history")
            && (code.contains("self.history.push") || code.contains("history.push"))
            && Self::validate_task2(code)
    }

    fn validate_task4(code: &str) -> bool {
        code.contains("clear_history") && code.contains("&[f64]") && Self::validate_task3(code)
    }

    fn validate_task5(code: &str) -> bool {
        code.contains("match ")
            || code.contains(".unwrap_or")
            || code.contains("?") && code.contains("history") && Self::validate_task4(code)
    }

    async fn ask_ai_for_changes(
        &self,
        current_code: &str,
        task: &str,
        use_search_replace: bool,
    ) -> Result<String, String> {
        let file_path = if use_search_replace {
            "sr_calculator.rs"
        } else {
            "ud_calculator.rs"
        };

        let tool_instruction = if use_search_replace {
            format!(
                r#"Use the replace_in_file tool with this format:
{{
  "file_path": "{}",
  "diff": "------- SEARCH\n[exact code to find]\n=======\n[replacement code]\n+++++++ REPLACE"
}}

You can use multiple SEARCH/REPLACE blocks in one diff parameter. Make sure the search text matches exactly including whitespace."#,
                file_path
            )
        } else {
            format!(
                r#"Use the apply_patch tool with unified diff format:
{{
  "file_path": "{}", 
  "patch": "--- {}\n+++ {}\n@@ -line,count +line,count @@\n context\n-removed\n+added\n context"
}}

Make sure to get the line numbers exactly right."#,
                file_path, file_path, file_path
            )
        };

        let system_prompt = format!(
            "You are a Rust programming assistant. You will be given code and asked to make specific changes.\n\n{}\n\nOnly respond with the tool call, no other text.",
            tool_instruction
        );

        let user_message = format!(
            "Current code:\n```rust\n{}\n```\n\nTask: {}\n\nPlease make the requested changes.",
            current_code, task
        );

        let request = ConversationRequest {
            messages: vec![Message {
                role: MessageRole::User,
                content: Content::text_only(user_message),
            }],
            model: Model::ClaudeSonnet4,
            system_prompt,
            tunings: ModelTunings::default(),
            stop_sequences: vec![],
            tools: if use_search_replace {
                vec![self.search_replace_tool.get_tool_definition()]
            } else {
                vec![self.patch_tool.get_tool_definition()]
            },
        };

        match self.ai_provider.converse(request).await {
            Ok(response) => {
                // Extract tool use from response
                for block in response.content.blocks() {
                    if let ContentBlock::ToolUse(tool_use) = block {
                        return Ok(serde_json::to_string(&tool_use.arguments).unwrap());
                    }
                }
                Err("No tool use found in AI response".to_string())
            }
            Err(e) => Err(format!("AI request failed: {:?}", e)),
        }
    }

    async fn apply_changes(
        &self,
        file_path: &str,
        ai_response: &str,
        use_search_replace: bool,
    ) -> Result<String, String> {
        let arguments: serde_json::Value = serde_json::from_str(ai_response)
            .map_err(|e| format!("Failed to parse AI response: {:?}", e))?;

        let tool = if use_search_replace {
            &self.search_replace_tool as &dyn ToolExecutor
        } else {
            &self.patch_tool as &dyn ToolExecutor
        };

        match tool.execute(&arguments).await {
            Ok(result) => {
                if result
                    .get("success")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    // Read the updated file
                    let updated_code = fs::read_to_string(self.temp_dir.path().join(file_path))
                        .map_err(|e| format!("Failed to read updated file: {:?}", e))?;
                    Ok(updated_code)
                } else {
                    Err(format!("Tool execution failed: {}", result))
                }
            }
            Err(e) => Err(format!("Tool execution error: {:?}", e)),
        }
    }

    async fn run_editing_session(&self, use_search_replace: bool) -> (bool, Duration, Vec<String>) {
        let approach_name = if use_search_replace {
            "Search/Replace"
        } else {
            "Unified Diff"
        };
        println!("\n=== Testing {} Approach ===", approach_name);

        let start_time = Instant::now();
        let mut current_code = self.get_initial_rust_code().to_string();
        let tasks = self.get_editing_tasks();
        let mut error_log = Vec::new();

        let file_path = if use_search_replace {
            "sr_calculator.rs"
        } else {
            "ud_calculator.rs"
        };

        // Write initial file
        if let Err(e) = fs::write(self.temp_dir.path().join(file_path), &current_code) {
            error_log.push(format!("Failed to write initial file: {:?}", e));
            return (false, start_time.elapsed(), error_log);
        }

        for (i, (task_description, validator)) in tasks.iter().enumerate() {
            println!("  Task {}: {}", i + 1, task_description);

            // Add delay to avoid rate limiting
            if i > 0 {
                sleep(Duration::from_secs(2)).await;
            }

            match self
                .ask_ai_for_changes(&current_code, task_description, use_search_replace)
                .await
            {
                Ok(ai_response) => {
                    match self
                        .apply_changes(file_path, &ai_response, use_search_replace)
                        .await
                    {
                        Ok(updated_code) => {
                            if validator(&updated_code) {
                                current_code = updated_code;
                                println!("    ✅ Success - Changes validated");
                            } else {
                                error_log.push(format!("Task {}: Validation failed - AI made changes but they don't meet requirements", i + 1));
                                println!(
                                    "    ❌ Validation failed - Changes don't meet requirements"
                                );
                                return (false, start_time.elapsed(), error_log);
                            }
                        }
                        Err(e) => {
                            error_log.push(format!("Task {}: Apply failed - {:?}", i + 1, e));
                            println!("    ❌ Apply failed: {}", e);
                            return (false, start_time.elapsed(), error_log);
                        }
                    }
                }
                Err(e) => {
                    error_log.push(format!("Task {}: AI request failed - {:?}", i + 1, e));
                    println!("    ❌ AI request failed: {}", e);
                    return (false, start_time.elapsed(), error_log);
                }
            }
        }

        let total_time = start_time.elapsed();
        println!("  ✅ All tasks completed in {:?}", total_time);

        (true, total_time, error_log)
    }

    async fn run_comparison(&self) {
        println!("\n🤖 REAL AI PERFORMANCE COMPARISON 🤖");
        println!("Using Claude 4 Sonnet via Bedrock to make iterative code changes\n");

        println!("Initial Rust code:");
        println!("```rust\n{}\n```", self.get_initial_rust_code());

        println!("\nTasks to complete:");
        for (i, (task_description, _)) in self.get_editing_tasks().iter().enumerate() {
            println!("  {}. {}", i + 1, task_description);
        }

        // Test search/replace approach
        let (sr_success, sr_time, sr_errors) = self.run_editing_session(true).await;

        // Test unified diff approach
        let (ud_success, ud_time, ud_errors) = self.run_editing_session(false).await;

        // Print results
        println!("\n{}", "=".repeat(60));
        println!("📊 FINAL RESULTS");
        println!("{}", "=".repeat(60));

        println!("\n🔍 Search/Replace Approach:");
        println!("  Success: {}", if sr_success { "✅ YES" } else { "❌ NO" });
        println!("  Time: {:?}", sr_time);
        if !sr_errors.is_empty() {
            println!("  Errors:");
            for error in &sr_errors {
                println!("    - {}", error);
            }
        }

        println!("\n📝 Unified Diff Approach:");
        println!("  Success: {}", if ud_success { "✅ YES" } else { "❌ NO" });
        println!("  Time: {:?}", ud_time);
        if !ud_errors.is_empty() {
            println!("  Errors:");
            for error in &ud_errors {
                println!("    - {}", error);
            }
        }

        println!("\n🏆 WINNER:");
        if sr_success && !ud_success {
            println!("  Search/Replace approach is more reliable!");
        } else if ud_success && !sr_success {
            println!("  Unified Diff approach is more reliable!");
        } else if sr_success && ud_success {
            if sr_time < ud_time {
                println!(
                    "  Search/Replace approach is faster! ({:?} vs {:?})",
                    sr_time, ud_time
                );
            } else if ud_time < sr_time {
                println!(
                    "  Unified Diff approach is faster! ({:?} vs {:?})",
                    ud_time, sr_time
                );
            } else {
                println!("  Both approaches performed equally well!");
            }
        } else {
            println!("  Both approaches failed - need to investigate!");
        }

        // Assert that at least search/replace works
        assert!(sr_success, "Search/Replace approach should succeed");
    }
}

// Helper trait to get tool definition
trait ToolDefinitionProvider {
    fn get_tool_definition(&self) -> tycode::ai::types::ToolDefinition;
}

impl ToolDefinitionProvider for ReplaceInFileTool {
    fn get_tool_definition(&self) -> tycode::ai::types::ToolDefinition {
        tycode::ai::types::ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            input_schema: self.input_schema(),
        }
    }
}

impl ToolDefinitionProvider for ApplyPatchTool {
    fn get_tool_definition(&self) -> tycode::ai::types::ToolDefinition {
        tycode::ai::types::ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            input_schema: self.input_schema(),
        }
    }
}

#[tokio::test]
#[ignore] // Use --ignored to run this test as it makes real API calls
async fn real_ai_performance_comparison() {
    let test_suite = RealAITestSuite::new().await;
    test_suite.run_comparison().await;
}

#[tokio::test]
#[ignore] // Use --ignored to run this test as it makes real API calls
async fn test_search_replace_with_real_ai() {
    let test_suite = RealAITestSuite::new().await;
    let (success, duration, errors) = test_suite.run_editing_session(true).await;

    println!("Search/Replace with real AI:");
    println!("  Success: {}", success);
    println!("  Duration: {:?}", duration);
    if !errors.is_empty() {
        println!("  Errors: {:?}", errors);
    }

    // This test should succeed
    assert!(success, "Search/Replace with real AI should succeed");
}
