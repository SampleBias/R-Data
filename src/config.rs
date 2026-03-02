//! Configuration loading from environment and optional config file.

use anyhow::Result;
use std::path::PathBuf;

/// Application configuration.
#[derive(Debug, Clone)]
pub struct Config {
    pub api_key: Option<String>,
    pub api_base_url: Option<String>,
    pub model: Option<String>,
    pub serpapi_key: Option<String>,
    pub viz_width: u32,
    pub viz_height: u32,
    pub default_bins: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_key: None,
            api_base_url: None,
            model: None,
            serpapi_key: None,
            viz_width: 800,
            viz_height: 600,
            default_bins: 20,
        }
    }
}

/// Loads configuration from environment variables and optional config file.
pub struct ConfigManager;

impl ConfigManager {
    /// Load config from env vars (ZAI_API_KEY, ZHIPU_API_KEY, SERPAPI_KEY, etc.)
    /// and optionally from ~/.config/r-data-agent/config.toml.
    /// Environment variables take precedence over config file.
    pub fn load_config() -> Result<Config> {
        let mut config = Self::load_from_file().unwrap_or_default();

        // Env vars override file
        if let Ok(v) = std::env::var("ZAI_API_KEY") {
            config.api_key = Some(v);
        } else if let Ok(v) = std::env::var("ZHIPU_API_KEY") {
            config.api_key = Some(v);
        }
        if let Ok(v) = std::env::var("SERPAPI_KEY") {
            config.serpapi_key = Some(v);
        }
        if let Ok(v) = std::env::var("R_DATA_API_BASE_URL") {
            config.api_base_url = Some(v);
        }
        if let Ok(v) = std::env::var("R_DATA_MODEL") {
            config.model = Some(v);
        }

        Ok(config)
    }

    fn load_from_file() -> Result<Config> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Config::default());
        }
        let content = std::fs::read_to_string(&path)?;
        let file_config: FileConfig = toml::from_str(&content)?;

        Ok(Config {
            api_key: file_config.api_key,
            api_base_url: file_config.api_base_url,
            model: file_config.model,
            serpapi_key: file_config.serpapi_key,
            viz_width: file_config.viz_width.unwrap_or(800),
            viz_height: file_config.viz_height.unwrap_or(600),
            default_bins: file_config.default_bins.unwrap_or(20),
        })
    }

    fn config_path() -> Result<PathBuf> {
        dirs::config_dir()
            .map(|p| p.join("r-data-agent").join("config.toml"))
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))
    }
}

#[derive(Debug, serde::Deserialize)]
struct FileConfig {
    api_key: Option<String>,
    api_base_url: Option<String>,
    model: Option<String>,
    serpapi_key: Option<String>,
    viz_width: Option<u32>,
    viz_height: Option<u32>,
    default_bins: Option<u32>,
}
