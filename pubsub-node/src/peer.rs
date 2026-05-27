use std::fmt;
use std::str::FromStr;

/// Failure modes returned when parsing a [`PeerId`] from a string.
#[derive(Debug, thiserror::Error)]
pub enum PeerIdError {
    #[error("peer id must not be empty")]
    Empty,
    #[error("peer id must not contain internal NUL bytes")]
    ContainsNul,
}

/// Logical identifier of a network participant.
///
/// Non-empty UTF-8, no internal NUL bytes. Construct via [`FromStr`]:
///
/// ```
/// use std::str::FromStr;
/// use pubsub_node::PeerId;
/// let id = PeerId::from_str("node-a").unwrap();
/// assert_eq!(id.as_str(), "node-a");
/// ```
///
/// Uniqueness is enforced per [`Network`](crate::Network) instance — two nodes
/// cannot register the same id on the same network — not globally across all
/// networks.
#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize)]
pub struct PeerId(String);

impl PeerId {
    /// Return the underlying string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for PeerId {
    type Err = PeerIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(PeerIdError::Empty);
        }
        if s.contains('\0') {
            return Err(PeerIdError::ContainsNul);
        }
        Ok(Self(s.to_owned()))
    }
}

impl<'de> serde::Deserialize<'de> for PeerId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        PeerId::from_str(&raw).map_err(serde::de::Error::custom)
    }
}

/// Abstract handle for addressing a peer.
///
/// Exposes an [`id`](PeerDescriptor::id) accessor; future iterations may add
/// network-level information (addresses, public keys) on richer implementors
/// without breaking callers that only need to address a peer by its id.
pub trait PeerDescriptor: Clone + Send + Sync + 'static {
    /// Return the peer's logical identifier.
    fn id(&self) -> &PeerId;
}

/// The v1 concrete [`PeerDescriptor`] implementation — a thin wrapper around
/// a [`PeerId`] with no other fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BasicPeerDescriptor {
    /// The peer's identifier.
    pub id: PeerId,
}

impl PeerDescriptor for BasicPeerDescriptor {
    fn id(&self) -> &PeerId {
        &self.id
    }
}

#[cfg(test)]
mod tests {
    use super::{PeerId, PeerIdError};
    use std::str::FromStr;

    #[test]
    fn empty_string_is_rejected() {
        assert!(matches!(PeerId::from_str(""), Err(PeerIdError::Empty)));
    }

    #[test]
    fn internal_nul_is_rejected() {
        assert!(matches!(
            PeerId::from_str("node\0a"),
            Err(PeerIdError::ContainsNul)
        ));
    }

    #[test]
    fn ordinary_utf8_is_accepted() {
        let id = PeerId::from_str("node-a").expect("valid id");
        assert_eq!(id.as_str(), "node-a");
        assert_eq!(format!("{id}"), "node-a");
    }
}
