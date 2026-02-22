use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use anyhow::{Context, Result};
use std::fs;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub vault_path: PathBuf,
}

pub struct AppPaths {
    pub data_dir: PathBuf,
    pub config_file: PathBuf,
}

impl AppPaths {
    pub fn from_env() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .context("Could not find config directory")?
            .join("ob");
        let data_dir = dirs::data_dir()
            .context("Could not find data directory")?
            .join("ob");

        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&data_dir)?;

        Ok(Self {
            config_file: config_dir.join("config.json"),
            data_dir,
        })
    }
}

pub fn load_config(paths: &AppPaths) -> Result<Config> {
    if !paths.config_file.exists() {
        return Err(anyhow::anyhow!(
            "Configuration not found. Please run `ob --init <VAULT_PATH>` first."
        ));
    }
    let content = fs::read_to_string(&paths.config_file)?;
    let config: Config = serde_json::from_str(&content)?;
    Ok(config)
}

pub fn save_config(paths: &AppPaths, config: &Config) -> Result<()> {
    let content = serde_json::to_string_pretty(config)?;
    fs::write(&paths.config_file, content)?;
    Ok(())
}
