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

    pub fn print_divider(&self) {
        if self.use_colors {
            println!("\x1b[36m═══════════════════════════════════════════════════\x1b[0m");
        } else {
            println!("═══════════════════════════════════════════════════");
        }
    }
}
