use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::ai::types::ModelSettings;
use crate::chat::FileModificationApi;

// Generic helper function for skip_serializing_if
fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    value == &T::default()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub version: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub global: GlobalSettings,
    #[serde(default, skip_serializing_if = "is_default")]
    pub providers: ProviderSettings,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub agents: HashMap<String, ModelSettings>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: "1.0".to_string(),
            global: GlobalSettings::default(),
            providers: ProviderSettings::default(),
            agents: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GlobalSettings {
    #[serde(default, skip_serializing_if = "is_default")]
    pub file_modification_api: FileModificationApi,
    #[serde(default, skip_serializing_if = "is_default")]
    pub trace: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderSettings {
    #[serde(default, skip_serializing_if = "is_default")]
    pub bedrock: BedrockProvider,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BedrockProvider {
    pub profile: Option<String>,
}

impl Default for BedrockProvider {
    fn default() -> Self {
        Self { profile: None }
    }
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            bedrock: BedrockProvider::default(),
        }
    }
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            file_modification_api: FileModificationApi::FindReplace,
            trace: false,
        }
    }
}

impl Settings {
    pub fn get_agent_model(&self, agent_name: &str) -> Option<&str> {
        self.agents.get(agent_name).map(|s| s.model.name())
    }

    pub fn get_agent_settings(&self, agent_name: &str) -> Option<&ModelSettings> {
        self.agents.get(agent_name)
    }

    /// Create settings from user config, only including non-default values
    pub fn from_settings_diff(&self, _defaults: &Settings) -> Self {
        // Just return self - the skip_serializing_if attributes will handle
        // filtering out default values during serialization
        self.clone()
    }
}
