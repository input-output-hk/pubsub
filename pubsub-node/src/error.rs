use std::path::PathBuf;

use crate::peer::PeerId;

/// Failure modes returned by [`load_peer_list`](crate::load_peer_list).
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The configuration file could not be read from disk.
    #[error("failed to read config file {path:?}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    /// The file's contents could not be parsed as TOML matching the peer-list
    /// schema. The error's `Display` chain includes line and column.
    #[error("failed to parse TOML config {path:?}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },

    /// A peer entry parsed successfully but its `id` failed the
    /// [`PeerId`](crate::PeerId) validation rules.
    #[error("invalid peer entry: {0}")]
    InvalidPeer(String),

    /// A `subscribed_topics` entry parsed successfully but its value failed
    /// the [`TopicId`](crate::TopicId) validation rules.
    #[error("config invalid topic entry: {0}")]
    InvalidTopic(String),
}

/// Failure modes returned by [`Network`](crate::Network) implementations.
#[derive(Debug, thiserror::Error)]
pub enum NetworkError {
    /// Two peers attempted to register the same id on the same network.
    #[error("peer id {0} is already registered on this network")]
    DuplicateRegistration(PeerId),
}

/// Failure modes from [`Node`](crate::Node) construction and sends.
#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    #[error(transparent)]
    Network(#[from] NetworkError),
}
