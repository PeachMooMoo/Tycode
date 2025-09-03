use serde_json::Value;
use similar::{ChangeTag, TextDiff};
use tycode_core::ai::TokenUsage;
use tycode_core::chat::ModelInfo;

#[derive(Clone)]
pub struct Formatter {
    use_colors: bool,
}

impl Formatter {
    pub fn new() -> Self {
        Self { use_colors: true }
    }

    pub fn print_system(&self, msg: &str) {
        if self.use_colors {
            println!("\x1b[33m[System]\x1b[0m {}", msg);
        } else {
            println!("[System] {}", msg);
        }
    }

    pub fn print_ai(
        &self,
        msg: &str,
        agent: &str,
        model_info: &Option<ModelInfo>,
        token_usage: &Option<TokenUsage>,
    ) {
        let model_name = model_info
            .as_ref()
            .map(|m| m.model.name())
            .unwrap_or_default();

        let usage_text = token_usage
            .as_ref()
            .map(|usage| format!(" (usage: {}/{})", usage.input_tokens, usage.output_tokens))
            .unwrap_or_default();

        if self.use_colors {
            println!(
                "\x1b[32m[{}]\x1b[0m \x1b[90m({}){}\x1b[0m {}",
                agent, model_name, usage_text, msg
            );
        } else {
            println!("[{}] ({}){} {}", agent, model_name, usage_text, msg);
        }
    }

    pub fn print_error(&self, msg: &str) {
        if self.use_colors {
            eprintln!("\x1b[31m[Error]\x1b[0m {}", msg);
        } else {
            eprintln!("[Error] {}", msg);
        }
    }

    pub fn print_prompt(&self) -> String {
        if self.use_colors {
            "\x1b[35m>\x1b[0m ".to_string()
        } else {
            "> ".to_string()
        }
    }

    pub fn print_tool_call(&self, name: &str, arguments: &serde_json::Value) {
        if self.use_colors {
            println!("\x1b[36m🔧 Tool:\x1b[0m \x1b[1;36m{}\x1b[0m \x1b[36mwith args:\x1b[0m \x1b[90m{}\x1b[0m", name, arguments);
        } else {
            println!("🔧 Tool: {} with args: {}", name, arguments);
        }
    }

    pub fn print_formatted_tool_call(&self, name: &str, args: &Value) {
        match name {
            "write_file" => {
                if let Some(path) = args.get("file_path").and_then(|v| v.as_str()) {
                    let content_len = args
                        .get("content")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .len();
                    self.print_system(&format!("💾 Writing file {} ({} chars)", path, content_len));
                } else {
                    self.print_tool_call(name, args);
                }
            }
            "replace_in_file" => {
                if let Some(path) = args.get("file_path").and_then(|v| v.as_str()) {
                    let diff_count = self.count_diff_blocks(args.get("diff"));
                    self.print_system(&format!(
                        "📝 Modifying file {} ({} changes)",
                        path, diff_count
                    ));
                    self.render_proposed_diff(args.get("diff"));
                } else {
                    self.print_tool_call(name, args);
                }
            }
            _ => {
                self.print_tool_call(name, args);
            }
        }
    }

    fn count_diff_blocks(&self, diff: Option<&Value>) -> usize {
        diff.and_then(|v| v.as_array())
            .map(|arr| arr.len())
            .unwrap_or(0)
    }

    fn render_proposed_diff(&self, diff: Option<&Value>) {
        if let Some(arr) = diff.and_then(|v| v.as_array()) {
            for block in arr {
                if let (Some(search), Some(replace)) = (
                    block.get("search").and_then(|v| v.as_str()),
                    block.get("replace").and_then(|v| v.as_str()),
                ) {
                    self.print_diff_block(search, replace, self.use_colors);
                }
            }
        }
    }

    fn print_diff_block(&self, search: &str, replace: &str, use_colors: bool) {
        // Compute and print unified diff with full context using similar crate
        let diff = TextDiff::from_lines(search, replace);
        let mut diff = diff.unified_diff();
        let unified = diff.context_radius(7);

        for hunk in unified.iter_hunks() {
            println!("{}", hunk.header());
            for change in hunk.iter_changes() {
                let line = change.value().trim_end_matches('\n');
                match change.tag() {
                    ChangeTag::Equal => {
                        if use_colors {
                            println!(" {}", line);
                        } else {
                            println!(" {}", line);
                        }
                    }
                    ChangeTag::Delete => {
                        if use_colors {
                            println!("\x1b[91m-{}\x1b[0m", line);
                        } else {
                            println!("-{}", line);
                        }
                    }
                    ChangeTag::Insert => {
                        if use_colors {
                            println!("\x1b[92m+{}\x1b[0m", line);
                        } else {
                            println!("+{}", line);
                        }
                    }
                }
            }
        }
    }

    pub fn print_tool_result(
        &self,
        name: &str,
        success: bool,
        result: Option<&Value>,
        ui_data: Option<&Value>,
        error: Option<&str>,
    ) {
        if success {
            self.print_system(&format!("✅ {} completed", name));
            if let Some(res) = result {
                match name {
                    "write_file" => {
                        if let Some(bytes) = res.get("bytes_written").and_then(|v| v.as_u64()) {
                            let action = if res
                                .get("created")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false)
                            {
                                "created"
                            } else {
                                "updated"
                            };
                            self.print_system(&format!("  {} file ({} bytes)", action, bytes));
                        }
                    }
                    "replace_in_file" => {
                        if let Some(reps) = res.get("replacements_made").and_then(|v| v.as_u64()) {
                            self.print_system(&format!("  {} replacements made", reps));
                        }
                    }
                    "search_files" => {
                        if let Some(count) = res.get("count").and_then(|v| v.as_u64()) {
                            self.print_system(&format!("  {} matches", count));
                        }
                    }

                    _ => {
                        if let Ok(pretty) = serde_json::to_string_pretty(res) {
                            println!("  {}", pretty.replace("\n", "\n  "));
                        }
                    }
                }
            }
            if self.is_file_related(name) {
                if let Some(ui) = ui_data {
                    if let (Some(orig_str), Some(new_str)) = (
                        ui.get("original_content").and_then(|v| v.as_str()),
                        ui.get("new_content").and_then(|v| v.as_str()),
                    ) {
                        let (added, removed) = self.calculate_diff_summary(orig_str, new_str);
                        self.print_system(&format!("  {} additions, {} deletions", added, removed));
                    }
                }
            }
        } else {
            let error_msg = if let Some(e) = error {
                e
            } else if let Some(r) = result {
                r.get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error")
            } else {
                "unknown error"
            };
            self.print_error(&format!("❌ {} failed: {}", name, error_msg));
        }
    }

    fn is_file_related(&self, name: &str) -> bool {
        matches!(name, "write_file" | "replace_in_file")
    }

    fn calculate_diff_summary(&self, orig: &str, new: &str) -> (usize, usize) {
        let orig_lines: Vec<&str> = orig.split('\n').collect();
        let new_lines: Vec<&str> = new.split('\n').collect();
        let max_len = orig_lines.len().max(new_lines.len());
        let mut added = 0;
        let mut removed = 0;
        for i in 0..max_len {
            let orig_line = orig_lines.get(i).copied().unwrap_or("");
            let new_line = new_lines.get(i).copied().unwrap_or("");
            if orig_line != new_line {
                if !orig_line.is_empty() {
                    removed += 1;
                }
                if !new_line.is_empty() {
                    added += 1;
                }
            }
        }
        (added, removed)
    }
}
