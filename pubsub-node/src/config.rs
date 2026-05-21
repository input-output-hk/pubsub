use std::path::Path;
use std::str::FromStr;

use crate::error::ConfigError;
use crate::peer::PeerId;

/// A single peer descriptor as it appears in a TOML peer-list file.
///
/// Unknown fields are rejected by [`serde`] so operators see a clear error
/// when they configure something the running binary does not understand.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PeerEntry {
    /// The peer's identifier.
    pub id: PeerId,
}

/// The parsed contents of a TOML peer-list file.
///
/// An empty or absent `peers` array is valid: a node constructed from an
/// empty list cannot originate sends, but may still receive messages from
/// other nodes that list it.
#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct PeerListConfig {
    /// The peer descriptors loaded from the TOML file, in declaration order.
    #[serde(default)]
    pub peers: Vec<PeerEntry>,
}

// Shadow types used only by `load_peer_list`. They let the loader differentiate
// a syntactic TOML parse failure (ConfigError::Parse) from a PeerId validation
// failure (ConfigError::InvalidPeer): the shadow's `String` field accepts any
// id at TOML-parse time, and the loader then runs FromStr explicitly on each
// id to surface the rule violation. The public PeerEntry / PeerListConfig
// derives stay strict (they go through PeerId::Deserialize) for any other
// caller that wants single-pass strict parsing.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPeerEntry {
    id: String,
}

#[derive(serde::Deserialize, Default)]
struct RawPeerListConfig {
    #[serde(default)]
    peers: Vec<RawPeerEntry>,
}

/// Load and validate a TOML peer-list file.
///
/// Runs a three-stage pipeline:
///
/// 1. Read the file at `path`. A read failure surfaces as
///    [`ConfigError::Io`].
/// 2. Parse the contents as TOML. A syntactic or structural failure surfaces
///    as [`ConfigError::Parse`], whose `Display` chain includes line and
///    column information from the underlying parser.
/// 3. Validate each [`PeerId`] via [`FromStr`]. A rule violation (empty id,
///    internal NUL byte) surfaces as [`ConfigError::InvalidPeer`].
///
/// The returned [`PeerListConfig`] is otherwise indistinguishable from one
/// constructed by hand.
pub fn load_peer_list(path: &Path) -> Result<PeerListConfig, ConfigError> {
    let content = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let raw: RawPeerListConfig = toml::from_str(&content).map_err(|source| ConfigError::Parse {
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
    Ok(PeerListConfig { peers })
}
