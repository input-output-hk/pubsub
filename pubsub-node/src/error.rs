use std::path::PathBuf;

use crate::peer::PeerId;

/// Failure modes returned by [`load_node_config`](crate::load_node_config).
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The configuration file could not be read from disk.
    #[error("failed to read config file {path:?}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    /// The file's contents could not be parsed as TOML matching the
    /// node-config schema. The error's `Display` chain includes line and
    /// column.
    #[error("failed to parse TOML config {path:?}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },

    /// A peer entry parsed successfully but its `id` failed the
    /// [`PeerId`] validation rules.
    #[error("invalid peer entry: {0}")]
    InvalidPeer(String),

    /// A `subscribed_topics` entry parsed successfully but its value failed
    /// the [`TopicId`](crate::TopicId) validation rules.
    #[error("config invalid topic entry: {0}")]
    InvalidTopic(String),

    /// Two entries in a subscription-list file declared the same `node_id`.
    #[error("duplicate subscription-list entry for node id {0}")]
    DuplicateSubscriptionEntry(String),

    /// Two entries in a topic-registry file declared the same topic `id`.
    #[error("duplicate topic-registry entry for topic id {0}")]
    DuplicateTopicEntry(String),

    /// A `publishers` entry in a topic-registry file was not valid lowercase
    /// hex decoding to public-key bytes.
    #[error("invalid publisher key in topic-registry file: {0}")]
    InvalidPublisherKey(String),
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

    /// The node's identifier does not match its signing key — the configured
    /// identity and the signer disagree, so every control message the node
    /// emitted would be rejected by its peers. Surfaced at construction before
    /// any registration or background activity.
    #[error("node identity {0} does not match its signing key")]
    IdentityMismatch(PeerId),
}
