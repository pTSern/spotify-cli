use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{create_dir_all, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct KeyBinding {
    pub action: String, // "increase", "decrease", "settings", "exit"
    pub key: String,    // "Right", "Left", "d", "a", "Esc", "q", "s", etc.
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VolumeSettings {
    pub step: u32,
    pub bindings: Vec<KeyBinding>,
}

impl Default for VolumeSettings {
    fn default() -> Self {
        Self {
            step: 5,
            bindings: vec![
                KeyBinding {
                    action: "increase".to_string(),
                    key: "Right".to_string(),
                },
                KeyBinding {
                    action: "decrease".to_string(),
                    key: "Left".to_string(),
                },
                KeyBinding {
                    action: "settings".to_string(),
                    key: "s".to_string(),
                },
                KeyBinding {
                    action: "exit".to_string(),
                    key: "Esc".to_string(),
                },
                KeyBinding {
                    action: "exit".to_string(),
                    key: "q".to_string(),
                },
            ],
        }
    }
}

impl VolumeSettings {
    pub fn count_action_bindings(&self, action: &str) -> usize {
        self.bindings.iter().filter(|b| b.action == action).count()
    }

    pub fn can_delete_binding(&self, index: usize) -> bool {
        if index >= self.bindings.len() {
            return false;
        }
        let action = &self.bindings[index].action;
        self.count_action_bindings(action) > 1
    }

    pub fn find_action(&self, key_name: &str) -> Option<&str> {
        let key_lower = key_name.to_lowercase();
        for b in &self.bindings {
            if b.key.to_lowercase() == key_lower {
                return Some(&b.action);
            }
        }
        None
    }

    pub fn keys_for_action(&self, action: &str) -> String {
        let keys: Vec<&str> = self
            .bindings
            .iter()
            .filter(|b| b.action == action)
            .map(|b| b.key.as_str())
            .collect();
        keys.join("/")
    }
}

fn default_status_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding { action: "seek_forward".to_string(), key: "Right".to_string() },
        KeyBinding { action: "seek_backward".to_string(), key: "Left".to_string() },
        KeyBinding { action: "vol_up".to_string(), key: "Up".to_string() },
        KeyBinding { action: "vol_down".to_string(), key: "Down".to_string() },
        KeyBinding { action: "toggle".to_string(), key: "Space".to_string() },
        KeyBinding { action: "toggle".to_string(), key: "t".to_string() },
        KeyBinding { action: "next".to_string(), key: "n".to_string() },
        KeyBinding { action: "prev".to_string(), key: "p".to_string() },
        KeyBinding { action: "shuffle".to_string(), key: "f".to_string() },
        KeyBinding { action: "repeat".to_string(), key: "r".to_string() },
        KeyBinding { action: "settings".to_string(), key: "s".to_string() },
        KeyBinding { action: "exit".to_string(), key: "Esc".to_string() },
        KeyBinding { action: "exit".to_string(), key: "q".to_string() },
    ]
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StatusSettings {
    pub seek_step: u32, // in seconds, default 10
    #[serde(default = "default_status_bindings")]
    pub bindings: Vec<KeyBinding>,
}

impl Default for StatusSettings {
    fn default() -> Self {
        Self {
            seek_step: 10,
            bindings: default_status_bindings(),
        }
    }
}

impl StatusSettings {
    pub fn count_action_bindings(&self, action: &str) -> usize {
        self.bindings.iter().filter(|b| b.action == action).count()
    }

    pub fn can_delete_binding(&self, index: usize) -> bool {
        if index >= self.bindings.len() {
            return false;
        }
        let action = &self.bindings[index].action;
        self.count_action_bindings(action) > 1
    }

    pub fn find_action(&self, key_name: &str) -> Option<&str> {
        let key_lower = key_name.to_lowercase();
        for b in &self.bindings {
            if b.key.to_lowercase() == key_lower {
                return Some(&b.action);
            }
        }
        None
    }

    pub fn keys_for_action(&self, action: &str) -> String {
        let keys: Vec<&str> = self
            .bindings
            .iter()
            .filter(|b| b.action == action)
            .map(|b| b.key.as_str())
            .collect();
        keys.join("/")
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Config {
    pub client_id: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_at: Option<u64>, // Unix timestamp in seconds
    pub default_device_id: Option<String>,
    #[serde(default)]
    pub volume_settings: VolumeSettings,
    #[serde(default)]
    pub status_settings: StatusSettings,
}

impl Config {
    pub fn config_path() -> Result<PathBuf> {
        let base_dir = dirs::config_dir()
            .or_else(dirs::home_dir)
            .context("Failed to determine config directory")?;
        let app_dir = base_dir.join("spotify-cli");
        create_dir_all(&app_dir)
            .with_context(|| format!("Failed to create config directory at {:?}", app_dir))?;
        Ok(app_dir.join("config.json"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }

        let mut file = File::open(&path)
            .with_context(|| format!("Failed to open config file at {:?}", path))?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let mut config: Config = serde_json::from_str(&contents).unwrap_or_default();

        // Allow environment variable to override or supply client_id
        if let Ok(env_id) = std::env::var("SPOTIFY_CLIENT_ID") {
            if !env_id.trim().is_empty() {
                config.client_id = Some(env_id.trim().to_string());
            }
        }

        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        let json = serde_json::to_string_pretty(self)?;
        let mut file = File::create(&path)
            .with_context(|| format!("Failed to write config to {:?}", path))?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }

    pub fn is_token_expired(&self) -> bool {
        match self.expires_at {
            Some(expires) => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                // Consider expired if within 60 seconds of expiry
                now + 60 >= expires
            }
            None => true,
        }
    }

    pub fn clear_tokens(&mut self) -> Result<()> {
        self.access_token = None;
        self.refresh_token = None;
        self.expires_at = None;
        self.save()
    }
}
