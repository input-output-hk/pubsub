use std::path::PathBuf;

use crate::peer::PeerId;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file {path:?}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse TOML config {path:?}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },

    #[error("invalid peer entry: {0}")]
    InvalidPeer(String),
}

#[derive(Debug, thiserror::Error)]
pub enum NetworkError {
    #[error("peer id {0} is already registered on this network")]
    DuplicateRegistration(PeerId),
}

#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    #[error(transparent)]
    Network(#[from] NetworkError),
}
