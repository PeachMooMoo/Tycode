use crate::settings::config::Settings;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub struct SettingsManager {
    settings_path: PathBuf,
    settings: Settings,
}

impl SettingsManager {
    /// Create a new settings manager with default settings location
    pub fn new() -> Result<Self> {
        let settings_path = Self::default_settings_path()?;
        
        // Ensure directory exists
        if let Some(parent) = settings_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {:?}", parent))?;
        }
        
        Self::from_path(settings_path)
    }

    /// Create a settings manager from a specific path
    pub fn from_path(path: PathBuf) -> Result<Self> {
        let settings = if path.exists() {
            Self::load_from_file(&path)?
        } else {
            let default_settings = Settings::default();
            // Save default settings to disk so file exists
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create directory: {:?}", parent))?;
            }
            let contents = toml::to_string_pretty(&default_settings)
                .context("Failed to serialize default settings")?;
            fs::write(&path, contents)
                .with_context(|| format!("Failed to write default settings to {:?}", path))?;
            default_settings
        };

        Ok(Self {
            settings_path: path,
            settings,
        })
    }

    /// Get the default settings path (~/.tycode/settings.toml)
    fn default_settings_path() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Failed to get home directory")?;
        Ok(home.join(".tycode").join("settings.toml"))
    }

    /// Load settings from a TOML file
    fn load_from_file(path: &Path) -> Result<Settings> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("Failed to read settings from {:?}", path))?;

        toml::from_str(&contents)
            .with_context(|| format!("Failed to parse settings from {:?}", path))
    }

    /// Get the current settings
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Get mutable reference to settings
    pub fn settings_mut(&mut self) -> &mut Settings {
        &mut self.settings
    }

    /// Save settings to file
    pub fn save(&self) -> Result<()> {
        // Ensure directory exists
        if let Some(parent) = self.settings_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {:?}", parent))?;
        }

        let contents =
            toml::to_string_pretty(&self.settings).context("Failed to serialize settings")?;

        fs::write(&self.settings_path, contents)
            .with_context(|| format!("Failed to write settings to {:?}", self.settings_path))?;

        Ok(())
    }

    /// Reload settings from file
    pub fn reload(&mut self) -> Result<()> {
        if self.settings_path.exists() {
            self.settings = Self::load_from_file(&self.settings_path)?;
        } else {
            self.settings = Settings::default();
        }
        Ok(())
    }

    /// Update settings with a closure and save
    pub fn update_setting<F>(&mut self, updater: F) -> Result<()>
    where
        F: FnOnce(&mut Settings),
    {
        updater(&mut self.settings);
        self.save()
    }

    /// Save provided settings
    pub fn save_settings(&self, settings: Settings) -> Result<()> {
        // Ensure directory exists
        if let Some(parent) = self.settings_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {:?}", parent))?;
        }

        let contents =
            toml::to_string_pretty(&settings).context("Failed to serialize settings")?;

        fs::write(&self.settings_path, contents)
            .with_context(|| format!("Failed to write settings to {:?}", self.settings_path))?;

        Ok(())
    }

    /// Get the settings file path
    pub fn path(&self) -> &Path {
        &self.settings_path
    }
}
