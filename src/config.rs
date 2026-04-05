use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Path to the beancount file (~ is expanded)
    pub beancount_file: String,
    /// Default currency
    pub currency: String,
    /// Run bean-check after every write
    pub auto_bean_check: bool,
    /// Spawn fava in background after committing transactions
    pub launch_fava_after_entry: bool,
    /// Port for fava
    pub fava_port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            beancount_file: "~/finances/main.beancount".to_string(),
            currency: "USD".to_string(),
            auto_bean_check: true,
            launch_fava_after_entry: false,
            fava_port: 5000,
        }
    }
}

impl Config {
    pub fn config_path() -> Result<PathBuf> {
        let base = dirs::config_dir().context("Could not find config directory")?;
        Ok(base.join("ptaui").join("config.json"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            let cfg = Config::default();
            cfg.save()?;
            return Ok(cfg);
        }
        let data = std::fs::read_to_string(&path)
            .with_context(|| format!("Reading config from {}", path.display()))?;
        let cfg: Config = serde_json::from_str(&data)
            .with_context(|| format!("Parsing config from {}", path.display()))?;
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Creating config dir {}", parent.display()))?;
        }
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, data)
            .with_context(|| format!("Writing config to {}", path.display()))?;
        Ok(())
    }

    /// Resolve beancount_file with ~ expansion
    pub fn resolved_beancount_file(&self) -> PathBuf {
        if self.beancount_file.starts_with("~/") {
            if let Some(home) = dirs::home_dir() {
                return home.join(&self.beancount_file[2..]);
            }
        }
        PathBuf::from(&self.beancount_file)
    }
}
