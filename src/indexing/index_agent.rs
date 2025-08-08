use crate::ai::provider::AiProvider;
use crate::ai::types::{Content, ConversationRequest, Message, MessageRole, Model, ModelSettings};
use anyhow::{Context, Result};
use std::path::Path;

pub struct IndexAgent {
    model: Model,
}

impl IndexAgent {
    pub fn new() -> Self {
        let model = Model::ClaudeSonnet37;
        Self { model }
    }

    pub async fn generate_file_outline(
        &self,
        provider: &dyn AiProvider,
        file_path: impl AsRef<Path>,
        content: &str,
    ) -> Result<Option<String>> {
        let file_path = file_path.as_ref();
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let prompt = self.create_file_outline_prompt(file_name, content);

        let request = ConversationRequest {
            messages: vec![Message {
                role: MessageRole::User,
                content: Content::text_only(prompt),
            }],
            model: ModelSettings {
                model: self.model,
                ..ModelSettings::default()
            },
            system_prompt: "You are an expert code indexing assistant.".to_string(),
            stop_sequences: vec![],
            tools: vec![],
        };

        let response = provider
            .converse(request)
            .await
            .context("Failed to generate file outline")?;

        let response_text = response.content.text();

        if response_text.trim() == "SKIP" {
            Ok(None)
        } else {
            Ok(Some(response_text.trim().to_string()))
        }
    }

    pub async fn generate_directory_summary(
        &self,
        provider: &dyn AiProvider,
        dir_path: impl AsRef<Path>,
        file_list: &[String],
    ) -> Result<String> {
        let dir_path = dir_path.as_ref();
        let dir_name = dir_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let prompt = self.create_directory_summary_prompt(dir_name, file_list);

        let request = ConversationRequest {
            messages: vec![Message {
                role: MessageRole::User,
                content: Content::text_only(prompt),
            }],
            model: ModelSettings {
                model: self.model,
                ..ModelSettings::default()
            },
            system_prompt: "You are an expert code indexing assistant.".to_string(),
            stop_sequences: vec![],
            tools: vec![],
        };

        let response = provider
            .converse(request)
            .await
            .context("Failed to generate directory summary")?;

        Ok(response.content.text())
    }

    fn create_file_outline_prompt(&self, file_name: &str, content: &str) -> String {
        format!(
            r#"You are an expert code indexing assistant. Analyze this file and determine if it contains source code.

File: {file_name}

If this file contains source code (any programming language), generate a concise outline showing:
- Public structs, enums, and types with their fields
- Public functions and methods with their signatures
- Public constants and modules
- Key traits and implementations
- NO implementation details, just interfaces

If this file is NOT source code (documentation, config, data files, etc.), respond with exactly: SKIP

File content:
```
{content}
```

Response:"#,
            file_name = file_name,
            content = content
        )
    }

    fn create_directory_summary_prompt(&self, dir_name: &str, file_list: &[String]) -> String {
        let files_section = file_list.join("\n");

        format!(
            r#"You are an expert code indexing assistant. Create a concise summary of this directory's purpose.

Directory: {dir_name}

Files and subdirectories:
{files}

Generate a brief summary explaining what this directory is for and how it fits into the codebase:"#,
            dir_name = dir_name,
            files = files_section
        )
    }
}
