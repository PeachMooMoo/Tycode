use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// The name of the currently active provider
    #[serde(default = "default_active_provider")]
    pub active_provider: String,

    /// Map of provider name to configuration
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
}

fn default_active_provider() -> String {
    "default".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ProviderConfig {
    #[serde(rename = "bedrock")]
    Bedrock {
        profile: String,
        #[serde(default = "default_region")]
        region: String,
    },
    // Future providers can be added here:
    // #[serde(rename = "openai")]
    // OpenAI { api_key: String },
}

fn default_region() -> String {
    "us-west-2".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        let mut providers = HashMap::new();
        providers.insert(
            "default".to_string(),
            ProviderConfig::Bedrock {
                profile: "default".to_string(),
                region: default_region(),
            },
        );

        Self {
            active_provider: "default".to_string(),
            providers,
        }
    }
}

impl Settings {
    /// Get the active provider configuration
    pub fn active_provider(&self) -> Option<&ProviderConfig> {
        self.providers.get(&self.active_provider)
    }

    /// Set the active provider (returns error if provider doesn't exist)
    pub fn set_active_provider(&mut self, name: &str) -> Result<(), String> {
        if self.providers.contains_key(name) {
            self.active_provider = name.to_string();
            Ok(())
        } else {
            Err(format!("Provider '{}' not found", name))
        }
    }

    /// Add or update a provider configuration
    pub fn add_provider(&mut self, name: String, config: ProviderConfig) {
        self.providers.insert(name, config);
    }

    /// Remove a provider configuration
    pub fn remove_provider(&mut self, name: &str) -> Result<(), String> {
        if name == self.active_provider {
            return Err("Cannot remove the active provider".to_string());
        }

        if self.providers.remove(name).is_some() {
            Ok(())
        } else {
            Err(format!("Provider '{}' not found", name))
        }
    }

    /// List all provider names
    pub fn list_providers(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }
}

impl ProviderConfig {
    /// Get the AWS profile for Bedrock provider
    pub fn bedrock_profile(&self) -> Option<&str> {
        match self {
            ProviderConfig::Bedrock { profile, .. } => Some(profile.as_str()),
        }
    }
}
