use crate::agents::ToolType;
use crate::ai::{ToolDefinition, ToolResultData, ToolUseData};
use crate::chat::state::{FileModificationApi, SharedChatState};
use crate::tools::file::apply_patch::ApplyPatchTool;
use crate::tools::file::delete_file::DeleteFileTool;
use crate::tools::file::list_files::ListFilesTool;
use crate::tools::file::read_file::ReadFileTool;
use crate::tools::file::replace_in_file::ReplaceInFileTool;
use crate::tools::file::search_files::SearchFilesTool;
use crate::tools::file::set_tracked_files::SetTrackedFilesTool;
use crate::tools::file::write_file::WriteFileTool;
use crate::tools::r#trait::ToolExecutor;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, error};

use super::execute_command::ExecuteCommandTool;

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn ToolExecutor>>,
    file_modification_api: FileModificationApi,
}

impl ToolRegistry {
    pub fn new(
        workspace_roots: Vec<PathBuf>,
        file_modification_api: FileModificationApi,
        chat_state: Option<Arc<SharedChatState>>,
    ) -> Self {
        let mut registry = Self {
            tools: HashMap::new(),
            file_modification_api: file_modification_api.clone(),
        };

        registry.register_file_tools(workspace_roots.clone(), file_modification_api, chat_state);
        registry.register_command_tools(workspace_roots);
        registry
    }

    fn register_file_tools(
        &mut self,
        workspace_roots: Vec<PathBuf>,
        file_modification_api: FileModificationApi,
        chat_state: Option<Arc<SharedChatState>>,
    ) {
        self.register_tool(Arc::new(ReadFileTool::new(workspace_roots.clone())));
        self.register_tool(Arc::new(WriteFileTool::new(workspace_roots.clone())));
        self.register_tool(Arc::new(ListFilesTool::new(workspace_roots.clone())));
        self.register_tool(Arc::new(SearchFilesTool::new(workspace_roots.clone())));
        self.register_tool(Arc::new(DeleteFileTool::new(workspace_roots.clone())));

        // Register set_tracked_files tool if chat state is available
        if let Some(chat_state) = chat_state {
            self.register_tool(Arc::new(SetTrackedFilesTool::new(
                workspace_roots.clone(),
                chat_state,
            )));
        }

        match file_modification_api {
            FileModificationApi::Patch => {
                debug!("Registering ApplyPatchTool for Patch API");
                self.register_tool(Arc::new(ApplyPatchTool::new(workspace_roots)));
            }
            FileModificationApi::FindReplace => {
                debug!("Registering ReplaceInFileTool for FindReplace API");
                self.register_tool(Arc::new(ReplaceInFileTool::new(workspace_roots)));
            }
        }
    }

    fn register_command_tools(&mut self, workspace_roots: Vec<PathBuf>) {
        self.register_tool(Arc::new(ExecuteCommandTool::new(workspace_roots)));
    }

    pub fn register_tool(&mut self, tool: Arc<dyn ToolExecutor>) {
        let name = tool.name().to_string();
        debug!(tool_name = %name, "Registering tool");
        self.tools.insert(name, tool);
    }

    /// Maps abstract tool types to concrete tool names based on configuration
    fn get_concrete_tool_name(&self, tool_type: ToolType) -> Option<&'static str> {
        match tool_type {
            ToolType::ReadFile => Some("read_file"),
            ToolType::WriteFile => Some("write_file"),
            ToolType::ListFiles => Some("list_files"),
            ToolType::SearchFiles => Some("search_files"),
            ToolType::ModifyFile => match self.file_modification_api {
                FileModificationApi::Patch => Some("apply_patch"),
                FileModificationApi::FindReplace => Some("replace_in_file"),
            },
            ToolType::ExecuteCommand => Some("execute_command"),
            ToolType::DeleteFile => Some("delete_file"),
            ToolType::SetTrackedFiles => Some("set_tracked_files"),
        }
    }

    /// Gets tool definitions for a specific set of tool types
    pub fn get_tool_definitions_for_types(&self, tool_types: &[ToolType]) -> Vec<ToolDefinition> {
        tool_types
            .iter()
            .filter_map(|&tool_type| self.get_concrete_tool_name(tool_type))
            .filter_map(|tool_name| self.tools.get(tool_name))
            .map(|tool| ToolDefinition {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                input_schema: tool.input_schema(),
            })
            .collect()
    }

    pub fn get_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .map(|tool| ToolDefinition {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                input_schema: tool.input_schema(),
            })
            .collect()
    }

    pub async fn execute_tool(&self, tool_use: &ToolUseData) -> ToolResultData {
        let tool = match self.tools.get(&tool_use.name) {
            Some(tool) => tool,
            None => {
                error!(tool_name = %tool_use.name, "Unknown tool");
                return ToolResultData {
                    tool_use_id: tool_use.id.clone(),
                    content: format!("Unknown tool: {}", tool_use.name),
                    is_error: true,
                };
            }
        };

        match tool.execute(&tool_use.arguments).await {
            Ok(result) => ToolResultData {
                tool_use_id: tool_use.id.clone(),
                content: result.to_string(),
                is_error: false,
            },
            Err(e) => {
                error!(?e, tool_name = %tool_use.name, "Tool execution failed");
                ToolResultData {
                    tool_use_id: tool_use.id.clone(),
                    content: format!("Error: {:?}", e),
                    is_error: true,
                }
            }
        }
    }

    pub fn list_tools(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }
}

impl Clone for ToolRegistry {
    fn clone(&self) -> Self {
        let mut new_registry = Self {
            tools: HashMap::new(),
            file_modification_api: self.file_modification_api.clone(),
        };

        for (name, tool) in &self.tools {
            new_registry.tools.insert(name.clone(), tool.clone());
        }

        new_registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_tool_registry_creation_patch_api() {
        let temp_dir = tempdir().unwrap();
        let registry = ToolRegistry::new(temp_dir.path().to_path_buf(), FileModificationApi::Patch);

        let tools = registry.list_tools();
        assert_eq!(tools.len(), 6);
        assert!(tools.contains(&"read_file"));
        assert!(tools.contains(&"write_file"));
        assert!(tools.contains(&"list_files"));
        assert!(tools.contains(&"search_files"));
        assert!(tools.contains(&"apply_patch"));
        assert!(tools.contains(&"delete_file"));
        assert!(!tools.contains(&"replace_in_file"));
    }

    #[tokio::test]
    async fn test_tool_registry_creation_find_replace_api() {
        let temp_dir = tempdir().unwrap();
        let registry = ToolRegistry::new(
            temp_dir.path().to_path_buf(),
            FileModificationApi::FindReplace,
        );

        let tools = registry.list_tools();
        assert_eq!(tools.len(), 6);
        assert!(tools.contains(&"read_file"));
        assert!(tools.contains(&"write_file"));
        assert!(tools.contains(&"list_files"));
        assert!(tools.contains(&"search_files"));
        assert!(tools.contains(&"replace_in_file"));
        assert!(tools.contains(&"delete_file"));
        assert!(!tools.contains(&"apply_patch"));
    }

    #[tokio::test]
    async fn test_tool_definitions() {
        let temp_dir = tempdir().unwrap();
        let registry = ToolRegistry::new(temp_dir.path().to_path_buf(), FileModificationApi::Patch);

        let definitions = registry.get_tool_definitions();
        assert_eq!(definitions.len(), 6);

        let read_file_def = definitions
            .iter()
            .find(|def| def.name == "read_file")
            .unwrap();
        assert_eq!(read_file_def.description, "Read the contents of a file");
    }

    #[tokio::test]
    async fn test_tool_execution() {
        let temp_dir = tempdir().unwrap();
        let registry = ToolRegistry::new(temp_dir.path().to_path_buf(), FileModificationApi::Patch);

        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "Hello, registry!").unwrap();

        let tool_use = ToolUseData {
            id: "test_id".to_string(),
            name: "read_file".to_string(),
            arguments: json!({
                "file_path": "test.txt",
                "summary": false
            }),
        };

        let result = registry.execute_tool(&tool_use).await;
        assert!(!result.is_error);

        let content: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(content["content"], "Hello, registry!");
    }

    #[tokio::test]
    async fn test_unknown_tool() {
        let temp_dir = tempdir().unwrap();
        let registry = ToolRegistry::new(temp_dir.path().to_path_buf(), FileModificationApi::Patch);

        let tool_use = ToolUseData {
            id: "test_id".to_string(),
            name: "unknown_tool".to_string(),
            arguments: json!({}),
        };

        let result = registry.execute_tool(&tool_use).await;
        assert!(result.is_error);
        assert!(result.content.contains("Unknown tool"));
    }

    #[tokio::test]
    async fn test_get_tool_definitions_for_types() {
        let temp_dir = tempdir().unwrap();
        let registry = ToolRegistry::new(temp_dir.path().to_path_buf(), FileModificationApi::Patch);

        let tool_types = vec![
            ToolType::ReadFile,
            ToolType::WriteFile,
            ToolType::ModifyFile, // Should map to apply_patch
        ];

        let definitions = registry.get_tool_definitions_for_types(&tool_types);
        assert_eq!(definitions.len(), 3);

        let tool_names: Vec<&str> = definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(tool_names.contains(&"read_file"));
        assert!(tool_names.contains(&"write_file"));
        assert!(tool_names.contains(&"apply_patch"));
    }

    #[tokio::test]
    async fn test_get_tool_definitions_for_types_find_replace() {
        let temp_dir = tempdir().unwrap();
        let registry = ToolRegistry::new(
            temp_dir.path().to_path_buf(),
            FileModificationApi::FindReplace,
        );

        let tool_types = vec![
            ToolType::ReadFile,
            ToolType::ModifyFile, // Should map to replace_in_file
        ];

        let definitions = registry.get_tool_definitions_for_types(&tool_types);
        assert_eq!(definitions.len(), 2);

        let tool_names: Vec<&str> = definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(tool_names.contains(&"read_file"));
        assert!(tool_names.contains(&"replace_in_file"));
    }
}
