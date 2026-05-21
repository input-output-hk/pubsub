use std::path::Path;
use std::str::FromStr;

use crate::error::ConfigError;
use crate::peer::PeerId;

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PeerEntry {
    pub id: PeerId,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct PeerListConfig {
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
