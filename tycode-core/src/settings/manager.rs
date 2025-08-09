// Settings manager - simplified for WASM compatibility
// File system operations are not available in WASM builds

use crate::settings::config::Settings;
use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct SettingsManager {
    settings_path: PathBuf,
    settings: Settings,
}

impl SettingsManager {
    pub fn new() -> Result<Self> {
        // In WASM, we just use default settings
        Ok(Self {
            settings_path: PathBuf::from("settings.toml"),
            settings: Settings::default(),
        })
    }

    pub fn from_path(path: PathBuf) -> Result<Self> {
        // In WASM, we ignore the path and use default settings
        Ok(Self {
            settings_path: path,
            settings: Settings::default(),
        })
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn settings_mut(&mut self) -> &mut Settings {
        &mut self.settings
    }

    pub fn save(&self) -> Result<()> {
        // File operations not available in WASM
        Ok(())
    }

    pub fn reload(&mut self) -> Result<()> {
        // File operations not available in WASM
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
