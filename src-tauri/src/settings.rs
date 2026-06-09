use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ForceMode {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "soft")]
    Soft,
    #[serde(rename = "hard")]
    Hard,
}

impl Default for ForceMode {
    fn default() -> Self {
        ForceMode::Soft
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub work_interval_minutes: u32,
    pub rest_duration_minutes: u32,
    pub max_postpone_count: u32,
    pub force_mode: ForceMode,
    pub pause_on_fullscreen: bool,
    pub idle_detect_enabled: bool,
    pub idle_threshold_minutes: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            work_interval_minutes: 25,
            rest_duration_minutes: 5,
            max_postpone_count: 3,
            force_mode: ForceMode::Soft,
            pause_on_fullscreen: false,
            idle_detect_enabled: true,
            idle_threshold_minutes: 5,
        }
    }
}

pub fn config_dir() -> PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("EyeGuard");
    dir
}

pub fn config_path() -> PathBuf {
    config_dir().join("settings.toml")
}

pub fn load_settings() -> Settings {
    let path = config_path();
    if path.exists() {
        match fs::read_to_string(&path) {
            Ok(content) => {
                toml::from_str(&content).unwrap_or_default()
            }
            Err(_) => Settings::default(),
        }
    } else {
        let default = Settings::default();
        let _ = save_settings(&default);
        default
    }
}

pub fn save_settings(settings: &Settings) -> Result<(), String> {
    let dir = config_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let content = toml::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(config_path(), content).map_err(|e| e.to_string())?;
    Ok(())
}