use crate::agents::tool_type::ToolType;
use crate::ai::{ToolDefinition, ToolUseData};
use crate::chat::state::FileModificationApi;
use crate::security::types::RiskLevel;
use crate::tools::complete_task::CompleteTask;
use crate::tools::file::apply_patch::ApplyPatchTool;
use crate::tools::file::delete_file::DeleteFileTool;
use crate::tools::file::list_files::ListFilesTool;
use crate::tools::file::read_file::ReadFileTool;
use crate::tools::file::replace_in_file::ReplaceInFileTool;
use crate::tools::file::search_files::SearchFilesTool;
use crate::tools::file::set_tracked_files::SetTrackedFilesTool;
use crate::tools::file::write_file::WriteFileTool;
use crate::tools::r#trait::{ToolExecutor, ToolRequest};
use crate::tools::spawn_agent::SpawnAgent;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, error};

use super::run_build_test::RunBuildTestTool;

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn ToolExecutor>>,
    file_modification_api: FileModificationApi,
}

impl ToolRegistry {
    pub fn new(workspace_roots: Vec<PathBuf>, file_modification_api: FileModificationApi) -> Self {
        let mut registry = Self {
            tools: HashMap::new(),
            file_modification_api: file_modification_api.clone(),
        };

        registry.register_file_tools(workspace_roots.clone(), file_modification_api);
        registry.register_command_tools(workspace_roots);
        registry.register_agent_tools();
        registry
    }

    fn register_file_tools(
        &mut self,
        workspace_roots: Vec<PathBuf>,
        file_modification_api: FileModificationApi,
    ) {
        self.register_tool(Arc::new(ReadFileTool::new(workspace_roots.clone())));
        self.register_tool(Arc::new(WriteFileTool::new(workspace_roots.clone())));
        self.register_tool(Arc::new(ListFilesTool::new(workspace_roots.clone())));
        self.register_tool(Arc::new(SearchFilesTool::new(workspace_roots.clone())));
        self.register_tool(Arc::new(DeleteFileTool::new(workspace_roots.clone())));
        self.register_tool(Arc::new(SetTrackedFilesTool::new(workspace_roots.clone())));

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
        self.register_tool(Arc::new(RunBuildTestTool::new(workspace_roots)));
    }

    fn register_agent_tools(&mut self) {
        self.register_tool(Arc::new(SpawnAgent));
        self.register_tool(Arc::new(CompleteTask));
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
            ToolType::RunBuildTestCommand => Some("run_build_test"),
            ToolType::DeleteFile => Some("delete_file"),
            ToolType::SetTrackedFiles => Some("set_tracked_files"),
            ToolType::SpawnAgent => Some("spawn_agent"),
            ToolType::CompleteTask => Some("complete_task"),
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

    pub async fn execute_tool(
        &self,
        tool_use: &ToolUseData,
        allowed_tool_types: Option<&[ToolType]>,
    ) -> crate::tools::r#trait::ToolResult {
        // Attempt to retrieve the requested tool. If it does not exist, include a list of available tools.
        let tool = match self.tools.get(&tool_use.name) {
            Some(tool) => tool,
            None => {
                // Build a comma‑separated list of tool names for diagnostics.
                let available = self.list_tools().join(", ");
                error!(tool_name = %tool_use.name, "Unknown tool");
                return crate::tools::r#trait::ToolResult::Error(format!(
                    "Unknown tool: {}. Available tools: {}",
                    tool_use.name, available
                ));
            }
        };

        // Then check if the tool is allowed by the agent (if restrictions are provided)
        if let Some(allowed_types) = allowed_tool_types {
            let allowed_names: Vec<&str> = allowed_types
                .iter()
                .filter_map(|&tool_type| self.get_concrete_tool_name(tool_type))
                .collect();

            if !allowed_names.contains(&tool_use.name.as_str()) {
                debug!(
                    tool_name = %tool_use.name,
                    allowed_tools = ?allowed_names,
                    "Tool not in allowed list for current agent"
                );
                return crate::tools::r#trait::ToolResult::Error(format!(
                    "Tool not available for current agent: {}",
                    tool_use.name
                ));
            }
        }

        let request = ToolRequest::new(tool_use.arguments.clone(), tool_use.id.clone());
        match tool.execute(&request).await {
            Ok(result) => result,
            Err(e) => {
                error!(?e, tool_name = %tool_use.name, "Tool execution failed");
                crate::tools::r#trait::ToolResult::Error(format!("Error: {:?}", e))
            }
        }
    }

    pub fn evaluate_tool_risk(
        &self,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<RiskLevel> {
        // Retrieve the tool, or return an error that lists all known tools.
        let available = self.list_tools().join(", ");
        let tool = self
            .tools
            .get(tool_name)
            .ok_or_else(|| anyhow!("Unknown tool: {}. Available tools: {}", tool_name, available))?;

        let risk_level = tool.evaluate_risk(arguments);
        debug!(
            tool_name = %tool_name,
            ?risk_level,
            "Evaluated tool risk"
        );

        Ok(risk_level)
    }

    pub fn list_tools(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
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
        let registry = ToolRegistry::new(
            vec![temp_dir.path().to_path_buf()],
            FileModificationApi::Patch,
        );

        let tools = registry.list_tools();
        assert_eq!(tools.len(), 10);
        assert!(tools.contains(&"read_file"));
        assert!(tools.contains(&"write_file"));
        assert!(tools.contains(&"list_files"));
        assert!(tools.contains(&"search_files"));
        assert!(tools.contains(&"apply_patch"));
        assert!(tools.contains(&"delete_file"));
        assert!(tools.contains(&"set_tracked_files"));
        assert!(tools.contains(&"run_build_test"));
        assert!(tools.contains(&"spawn_agent"));
        assert!(tools.contains(&"complete_task"));
        assert!(!tools.contains(&"replace_in_file"));
    }

    #[tokio::test]
    async fn test_tool_registry_creation_find_replace_api() {
        let temp_dir = tempdir().unwrap();
        let registry = ToolRegistry::new(
            vec![temp_dir.path().to_path_buf()],
            FileModificationApi::FindReplace,
        );

        let tools = registry.list_tools();
        assert_eq!(tools.len(), 10);
        assert!(tools.contains(&"read_file"));
        assert!(tools.contains(&"write_file"));
        assert!(tools.contains(&"list_files"));
        assert!(tools.contains(&"search_files"));
        assert!(tools.contains(&"replace_in_file"));
        assert!(tools.contains(&"delete_file"));
        assert!(tools.contains(&"set_tracked_files"));
        assert!(tools.contains(&"run_build_test"));
        assert!(tools.contains(&"spawn_agent"));
        assert!(tools.contains(&"complete_task"));
        assert!(!tools.contains(&"apply_patch"));
    }

    #[tokio::test]
    async fn test_tool_definitions() {
        let temp_dir = tempdir().unwrap();
        let registry = ToolRegistry::new(
            vec![temp_dir.path().to_path_buf()],
            FileModificationApi::Patch,
        );

        let definitions = registry.get_tool_definitions();
        assert_eq!(definitions.len(), 10);

        let read_file_def = definitions
            .iter()
            .find(|def| def.name == "read_file")
            .unwrap();
        assert!(read_file_def
            .description
            .contains("Read the contents of a file"));
    }

    #[tokio::test]
    async fn test_tool_execution() {
        let temp_dir = tempdir().unwrap();
        let registry = ToolRegistry::new(
            vec![temp_dir.path().to_path_buf()],
            FileModificationApi::Patch,
        );

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

        let result = registry.execute_tool(&tool_use, None).await;
        match result {
            crate::tools::r#trait::ToolResult::Success { context_data, .. } => {
                let content: serde_json::Value =
                    serde_json::from_str(&context_data.to_string()).unwrap();
                assert_eq!(content["content"], "Hello, registry!");
            }
            _ => panic!("Expected success result"),
        }
    }

    #[tokio::test]
    async fn test_unknown_tool() {
        let temp_dir = tempdir().unwrap();
        let registry = ToolRegistry::new(
            vec![temp_dir.path().to_path_buf()],
            FileModificationApi::Patch,
        );

        let tool_use = ToolUseData {
            id: "test_id".to_string(),
            name: "unknown_tool".to_string(),
            arguments: json!({}),
        };

        let result = registry.execute_tool(&tool_use, None).await;
        match result {
            crate::tools::r#trait::ToolResult::Error(msg) => {
                assert!(msg.contains("Unknown tool"));
            }
            _ => panic!("Expected error result"),
        }
    }

    #[tokio::test]
    async fn test_get_tool_definitions_for_types() {
        let temp_dir = tempdir().unwrap();
        let registry = ToolRegistry::new(
            vec![temp_dir.path().to_path_buf()],
            FileModificationApi::Patch,
        );

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
            vec![temp_dir.path().to_path_buf()],
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

    #[tokio::test]
    async fn test_tool_execution_with_allowed_types() {
        let temp_dir = tempdir().unwrap();
        let registry = ToolRegistry::new(
            vec![temp_dir.path().to_path_buf()],
            FileModificationApi::Patch,
        );

        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "Test content").unwrap();

        // Test 1: Tool allowed - should succeed
        let allowed_types = vec![ToolType::ReadFile, ToolType::WriteFile];
        let tool_use = ToolUseData {
            id: "test_id".to_string(),
            name: "read_file".to_string(),
            arguments: json!({
                "file_path": "test.txt",
                "summary": false
            }),
        };

        let result = registry.execute_tool(&tool_use, Some(&allowed_types)).await;
        match result {
            crate::tools::r#trait::ToolResult::Success { .. } => {}
            _ => panic!("Expected success result"),
        }

        // Test 2: Tool not in allowed list - should fail
        let restricted_types = vec![ToolType::WriteFile]; // No ReadFile
        let result = registry
            .execute_tool(&tool_use, Some(&restricted_types))
            .await;
        match result {
            crate::tools::r#trait::ToolResult::Error(msg) => {
                assert!(msg.contains("Tool not available for current agent"));
            }
            _ => panic!("Expected error result"),
        }

        // Test 3: No restrictions (None) - should succeed
        let result = registry.execute_tool(&tool_use, None).await;
        match result {
            crate::tools::r#trait::ToolResult::Success { .. } => {}
            _ => panic!("Expected success result"),
        }
    }
}
