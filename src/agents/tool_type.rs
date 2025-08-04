#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolType {
    ReadFile,
    WriteFile,
    ListFiles,
    SearchFiles,
    ModifyFile,
}

impl ToolType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::ReadFile => "read_file",
            Self::WriteFile => "write_file",
            Self::ListFiles => "list_files",
            Self::SearchFiles => "search_files",
            Self::ModifyFile => "modify_file",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "read_file" => Some(Self::ReadFile),
            "write_file" => Some(Self::WriteFile),
            "list_files" => Some(Self::ListFiles),
            "search_files" => Some(Self::SearchFiles),
            "modify_file" | "apply_patch" | "replace_in_file" => Some(Self::ModifyFile),
            _ => None,
        }
    }
}