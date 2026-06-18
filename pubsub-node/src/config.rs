use std::path::Path;
use std::str::FromStr;

use crate::error::ConfigError;
use crate::peer::PeerId;

/// A single peer descriptor as it appears in a TOML node-config file.
///
/// Unknown fields are rejected by [`serde`] so operators see a clear error
/// when they configure something the running binary does not understand.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PeerEntry {
    /// The peer's identifier.
    pub id: PeerId,
}

/// The parsed contents of a TOML node-config file.
///
/// An empty or absent `peers` array is valid: a node constructed from an
/// empty list cannot originate sends, but may still receive messages from
/// other nodes that list it. The node's subscribed topics are **not** in this
/// config — they are sourced from the node's own subscription-registry entry
/// at startup (the source of truth; see ADR 0013).
#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct NodeConfig {
    /// The peer descriptors loaded from the TOML file, in declaration order.
    #[serde(default)]
    pub peers: Vec<PeerEntry>,
}

// Shadow types used only by `load_node_config`. They let the loader
// differentiate a syntactic TOML parse failure (ConfigError::Parse) from a
// PeerId / TopicId validation failure (ConfigError::InvalidPeer /
// ConfigError::InvalidTopic): the shadow's `String` field accepts any id at
// TOML-parse time, and the loader then runs FromStr explicitly on each entry
// to surface the rule violation. The public PeerEntry / NodeConfig derives
// stay strict (they go through PeerId / TopicId Deserialize) for any other
// caller that wants single-pass strict parsing.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPeerEntry {
    id: String,
}

#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawNodeConfig {
    #[serde(default)]
    peers: Vec<RawPeerEntry>,
}

/// Load and validate a TOML node-config file.
///
/// Pipeline:
///
/// 1. Read the file at `path`. A read failure surfaces as
///    [`ConfigError::Io`].
/// 2. Parse the contents as TOML. A syntactic or structural failure (or an
///    unknown top-level field) surfaces as [`ConfigError::Parse`], whose
///    `Display` chain includes line and column information from the
///    underlying parser.
/// 3. Validate each [`PeerId`] via [`FromStr`]. A rule violation (empty id,
///    internal NUL byte) surfaces as [`ConfigError::InvalidPeer`].
///
/// Topics are **not** part of the node config — a node's subscribed topics are
/// sourced from its own subscription-registry entry at startup (ADR 0013).
pub fn load_node_config(path: &Path) -> Result<NodeConfig, ConfigError> {
    let content = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let raw: RawNodeConfig = toml::from_str(&content).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })?;

    let peers = raw
        .peers
        .into_iter()
        .map(|entry| {
            PeerId::from_str(&entry.id)
                .map(|id| PeerEntry { id })
                .map_err(|err| ConfigError::InvalidPeer(format!("{}: {err}", path.display())))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(NodeConfig { peers })
}
