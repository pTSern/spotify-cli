use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{create_dir_all, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Config {
    pub client_id: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_at: Option<u64>, // Unix timestamp in seconds
    pub default_device_id: Option<String>,
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
