use crate::settings::config::Settings;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub struct SettingsManager {
    settings_path: PathBuf,
    settings: Settings,
}

impl SettingsManager {
    pub fn new() -> Result<Self> {
        let settings_path = Self::get_settings_path()?;
        let settings = Self::load_or_create(&settings_path)?;

        Ok(Self {
            settings_path,
            settings,
        })
    }

    pub fn from_path(path: PathBuf) -> Result<Self> {
        let settings = Self::load_or_create(&path)?;
        Ok(Self {
            settings_path: path,
            settings,
        })
    }

    fn get_settings_path() -> Result<PathBuf> {
        // First try ~/.tycode/settings.toml
        if let Some(home_dir) = dirs::home_dir() {
            let global_path = home_dir.join(".tycode").join("settings.toml");
            if global_path.exists() {
                return Ok(global_path);
            }
            // If it doesn't exist, this is where we'll create it
            return Ok(global_path);
        }

        // Fallback to current directory
        Ok(PathBuf::from(".tycode/settings.toml"))
    }

    fn load_or_create(path: &Path) -> Result<Settings> {
        if path.exists() {
            Self::load_from_file(path)
        } else {
            // Create minimal settings file with just version and helpful comments
            let user_settings = Settings::default();
            Self::ensure_parent_dir(path)?;
            Self::save_settings_to_file(path, &user_settings)?;
            eprintln!("📝 Created minimal settings file at: {}", path.display());

            Ok(user_settings)
        }
    }

    fn load_from_file(path: &Path) -> Result<Settings> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("Failed to read settings from {}", path.display()))?;

        let user_settings = toml::from_str::<Settings>(&contents)
            .with_context(|| format!("Failed to parse settings from {}", path.display()))?;

        Ok(user_settings)
    }

    fn save_settings_to_file(path: &Path, settings: &Settings) -> Result<()> {
        // Create a pretty TOML with helpful comments
        let mut contents = String::new();
        contents.push_str("# TyCode Settings Configuration\n");
        contents.push_str("# Only user-modified settings are saved here.\n");
        contents
            .push_str("# Defaults are applied automatically and will update with new versions.\n");
        contents.push_str("# See SETTINGS.md for all available options.\n\n");

        let toml_content =
            toml::to_string_pretty(settings).context("Failed to serialize settings")?;
        contents.push_str(&toml_content);

        fs::write(path, contents)
            .with_context(|| format!("Failed to write settings to {}", path.display()))?;

        Ok(())
    }

    fn ensure_parent_dir(path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {}", parent.display()))?;
        }
        Ok(())
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn settings_mut(&mut self) -> &mut Settings {
        &mut self.settings
    }

    pub fn save(&self) -> Result<()> {
        // Save only the user-modified settings
        Self::save_settings_to_file(&self.settings_path, &self.settings)
    }

    pub fn reload(&mut self) -> Result<()> {
        let settings = Self::load_from_file(&self.settings_path)?;
        self.settings = settings;
        Ok(())
    }

    pub fn update_setting<F>(&mut self, updater: F) -> Result<()>
    where
        F: FnOnce(&mut Settings),
    {
        updater(&mut self.settings);
        self.save()
    }

    pub fn path(&self) -> &Path {
        &self.settings_path
    }
}
