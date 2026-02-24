use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const MODEL_ID: &str = "BAAI/bge-small-en-v1.5";
pub const IGNORE_FOLDERS: &[&str] = &[".obsidian", ".git", ".stfolder", "templates"];

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub vault_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub config_file: PathBuf,
    pub log_file: PathBuf,
}

impl AppPaths {
    pub fn from_env() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .context("Could not find config directory")?
            .join("obra");
        let data_dir = dirs::data_dir()
            .context("Could not find data directory")?
            .join("obra");

        Self::new(config_dir, data_dir)
    }

    pub fn new(config_dir: PathBuf, data_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&data_dir)?;

        Ok(Self {
            config_file: config_dir.join("config.json"),
            log_file: data_dir.join("daemon.log"),
            data_dir,
        })
    }
}

pub fn load_config(paths: &AppPaths) -> Result<Config> {
    if !paths.config_file.exists() {
        return Err(anyhow::anyhow!(
            "Configuration not found. Please run `obra --init <VAULT_PATH>` first."
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_config_save_load() -> Result<()> {
        let config_dir = tempdir()?;
        let data_dir = tempdir()?;
        let paths = AppPaths::new(
            config_dir.path().to_path_buf(),
            data_dir.path().to_path_buf(),
        )?;

        let config = Config {
            vault_path: PathBuf::from("/tmp/vault"),
        };

        save_config(&paths, &config)?;
        let loaded = load_config(&paths)?;

        assert_eq!(loaded.vault_path, config.vault_path);
        Ok(())
    }

    #[test]
    fn test_load_nonexistent_config() -> Result<()> {
        let config_dir = tempdir()?;
        let data_dir = tempdir()?;
        let paths = AppPaths::new(
            config_dir.path().to_path_buf(),
            data_dir.path().to_path_buf(),
        )?;

        let result = load_config(&paths);
        assert!(result.is_err());
        Ok(())
    }
}
