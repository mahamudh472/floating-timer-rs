use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Persisted state, stored as JSON in the user's config directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub task: String,
    pub minutes: u32,
    /// Top-left position of the window, if it is known.
    pub pos: Option<[f32; 2]>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            task: String::new(),
            minutes: 25,
            pos: None,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        let mut settings = std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Self>(&raw).ok())
            .unwrap_or_default();
        settings.minutes = settings.minutes.clamp(1, 999);
        settings
    }

    pub fn save(&self) {
        let Some(path) = Self::path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(raw) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, raw);
        }
    }

    fn path() -> Option<PathBuf> {
        Some(
            dirs::config_dir()?
                .join("floating-timer-rs")
                .join("settings.json"),
        )
    }
}
