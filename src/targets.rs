use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;

/// Per-target connection settings loaded from a targets file.
///
/// This is intended to separate "app-wide" configuration (logging, UI, etc.)
/// from "server/target" configuration (instance, ports, SSH user/key).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TargetsConfig {
    #[serde(default)]
    pub targets: HashMap<String, TargetConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TargetConfig {
    pub instance_id: String,

    #[serde(default)]
    pub local_port: Option<u16>,
    #[serde(default)]
    pub remote_port: Option<u16>,

    /// Remote host for port forwarding through the instance (uses AWS-StartPortForwardingSessionToRemoteHost)
    #[serde(default)]
    pub remote_host: Option<String>,

    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub region: Option<String>,

    // SSH settings applied to the generated ~/.ssh/config entry.
    #[serde(default)]
    pub ssh_user: Option<String>,
    #[serde(default)]
    pub ssh_identity_file: Option<String>,
    #[serde(default)]
    pub ssh_identities_only: Option<bool>,
}

impl TargetsConfig {
    fn base_dir() -> Result<PathBuf> {
        let dir = if cfg!(windows) {
            dirs::config_dir().context("Could not determine config directory")?
        } else {
            dirs::home_dir()
                .map(|h| h.join(".config"))
                .or_else(dirs::config_dir)
                .context("Could not determine config directory")?
        };
        Ok(dir.join("nimbus"))
    }

    /// Return candidate paths in priority order: JSON first, then TOML.
    fn candidate_paths() -> Result<Vec<PathBuf>> {
        let base = Self::base_dir()?;
        Ok(vec![
            base.join("targets.json"),
            base.join("targets.toml"),
        ])
    }

    pub async fn load(path: Option<&str>) -> Result<(Self, PathBuf)> {
        let candidates = match path {
            Some(p) => vec![PathBuf::from(p)],
            None => Self::candidate_paths()?,
        };

        let found = candidates.iter().find(|p| p.exists());
        let path = found.cloned().ok_or_else(|| {
            anyhow::anyhow!(
                "Targets file not found. Create it (e.g. from targets.json.example) or pass --targets-file.\nSearched: {:?}",
                candidates
            )
        })?;

        let content = fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read targets file: {:?}", path))?;

        let parsed: TargetsConfig = if path.extension().and_then(|s| s.to_str()) == Some("toml") {
            toml::from_str(&content)
                .with_context(|| format!("Failed to parse TOML targets file: {:?}", path))?
        } else {
            serde_json::from_str(&content)
                .with_context(|| format!("Failed to parse JSON targets file: {:?}", path))?
        };

        Ok((parsed, path))
    }

    pub fn get(&self, name: &str) -> Option<&TargetConfig> {
        self.targets.get(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_load_json_targets() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("targets.json");

        let json = r#"{
    "targets": {
        "dev": {
            "instance_id": "i-123",
            "local_port": 5555,
            "remote_port": 22,
            "profile": "default",
            "region": "ap-northeast-1",
            "ssh_user": "ubuntu",
            "ssh_identity_file": "~/.ssh/test.pem",
            "ssh_identities_only": true
        }
    }
}"#;

        fs::write(&path, json).await.unwrap();
        let (cfg, _path) = TargetsConfig::load(Some(path.to_string_lossy().as_ref()))
            .await
            .unwrap();

        let dev = cfg.get("dev").unwrap();
        assert_eq!(dev.instance_id, "i-123");
        assert_eq!(dev.local_port, Some(5555));
        assert_eq!(dev.remote_port, Some(22));
        assert_eq!(dev.ssh_user.as_deref(), Some("ubuntu"));
        assert_eq!(dev.ssh_identities_only, Some(true));
    }
}
