use std::path::Path;

use anyhow::Context;
use serde::Deserialize;

/// TOML configuration file for pubsub-node.
///
/// All fields are optional — CLI flags take precedence and built-in defaults
/// apply when both are absent.  Unknown keys (e.g. admin-only fields written by
/// pubsub-admin) are silently ignored so the same file can be used for both.
///
/// Precedence: CLI flag > config file > built-in default.
#[derive(Debug, Default, Deserialize)]
pub struct NodeConfig {
    pub network: Option<String>,
    pub topics: Option<Vec<String>>,
    pub cyclon_interval: Option<u64>,
    pub vicinity_interval: Option<u64>,
    pub topic_refresh_interval: Option<u64>,
    pub log_level: Option<String>,
    pub blockfrost_url: Option<String>,
    pub blockfrost_key: Option<String>,
    pub topic_validator_addr: Option<String>,
    pub publisher_vault_addr: Option<String>,
    pub node_registry_addr: Option<String>,
    pub registry_policy_id: Option<String>,
}

impl NodeConfig {
    pub fn from_file(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("reading config file {}", path.display()))?;
        toml::from_str(&content)
            .with_context(|| format!("parsing config file {}", path.display()))
    }
}
