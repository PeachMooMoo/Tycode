use crate::ai::{ToolDefinition, ToolResultData, ToolUseData};
use crate::chat::state::FileModificationApi;
use crate::tools::file::apply_patch::ApplyPatchTool;
use crate::tools::file::list_files::ListFilesTool;
use crate::tools::file::read_file::ReadFileTool;
use crate::tools::file::replace_in_file::ReplaceInFileTool;
use crate::tools::file::search_files::SearchFilesTool;
use crate::tools::file::write_file::WriteFileTool;
use crate::tools::r#trait::ToolExecutor;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, error};

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn ToolExecutor>>,
}

impl ToolRegistry {
    pub fn new(workspace_root: PathBuf, file_modification_api: FileModificationApi) -> Self {
        let mut registry = Self {
            tools: HashMap::new(),
        };

        registry.register_file_tools(workspace_root, file_modification_api);
        registry
    }

    fn register_file_tools(
        &mut self,
        workspace_root: PathBuf,
        file_modification_api: FileModificationApi,
    ) {
        // Register common file tools
        self.register_tool(Arc::new(ReadFileTool::new(workspace_root.clone())));
        self.register_tool(Arc::new(WriteFileTool::new(workspace_root.clone())));
        self.register_tool(Arc::new(ListFilesTool::new(workspace_root.clone())));
        self.register_tool(Arc::new(SearchFilesTool::new(workspace_root.clone())));

        // Register only the enabled file modification tool
        match file_modification_api {
            FileModificationApi::Patch => {
                debug!("Registering ApplyPatchTool for Patch API");
                self.register_tool(Arc::new(ApplyPatchTool::new(workspace_root)));
            }
            FileModificationApi::FindReplace => {
                debug!("Registering ReplaceInFileTool for FindReplace API");
                self.register_tool(Arc::new(ReplaceInFileTool::new(workspace_root)));
            }
        }
    }

    pub fn register_tool(&mut self, tool: Arc<dyn ToolExecutor>) {
        let name = tool.name().to_string();
        debug!(tool_name = %name, "Registering tool");
        self.tools.insert(name, tool);
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
        assert_eq!(tools.len(), 5);
        assert!(tools.contains(&"read_file"));
        assert!(tools.contains(&"write_file"));
        assert!(tools.contains(&"list_files"));
        assert!(tools.contains(&"search_files"));
        assert!(tools.contains(&"apply_patch"));
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
        assert_eq!(tools.len(), 5);
        assert!(tools.contains(&"read_file"));
        assert!(tools.contains(&"write_file"));
        assert!(tools.contains(&"list_files"));
        assert!(tools.contains(&"search_files"));
        assert!(tools.contains(&"replace_in_file"));
        assert!(!tools.contains(&"apply_patch"));
    }

    #[tokio::test]
    async fn test_tool_definitions() {
        let temp_dir = tempdir().unwrap();
        let registry = ToolRegistry::new(temp_dir.path().to_path_buf(), FileModificationApi::Patch);

        let definitions = registry.get_tool_definitions();
        assert_eq!(definitions.len(), 5);

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
                "file_path": "test.txt"
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
}
