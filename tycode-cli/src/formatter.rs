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

    pub fn print_ai(&self, msg: &str) {
        if self.use_colors {
            println!("\x1b[32m[AI]\x1b[0m {}", msg);
        } else {
            println!("[AI] {}", msg);
        }
    }

    pub fn print_ai_with_model(&self, msg: &str, model_info: &crate::chat::events::ModelInfo) {
        if self.use_colors {
            println!(
                "\x1b[32m[AI]\x1b[0m \x1b[90m({})\x1b[0m {}",
                model_info.model.name(),
                msg
            );
        } else {
            println!("[AI] ({}) {}", model_info.model.name(), msg);
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
