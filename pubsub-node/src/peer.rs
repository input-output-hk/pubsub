use std::fmt;
use std::str::FromStr;

use crate::crypto::mock::{derive_public, PUBLIC_SUFFIX};
use crate::crypto::{PrivateKey, PublicKey};

/// Failure modes returned when parsing a [`PeerId`] from a string.
#[derive(Debug, thiserror::Error)]
pub enum PeerIdError {
    #[error("peer id must not be empty")]
    Empty,
    #[error("peer id must not contain internal NUL bytes")]
    ContainsNul,
}

/// Logical identifier of a network participant — a public key.
///
/// The string form is the mock-stage **alias rule**: parsing a string (which
/// must be non-empty UTF-8 with no internal NUL bytes) treats it as an alias
/// and derives the key from it, and [`Display`](fmt::Display) renders the
/// alias back, so config files, fixtures, and logs stay human-legible:
///
/// ```
/// use std::str::FromStr;
/// use pubsub_node::PeerId;
/// let id = PeerId::from_str("node-a").unwrap();
/// assert_eq!(id.to_string(), "node-a");
/// ```
///
/// A key that does not stem from an alias displays as lowercase hex instead.
/// Two ids are equal exactly when their key bytes are equal.
///
/// Uniqueness is enforced per [`Network`](crate::Network) instance — two nodes
/// cannot register the same id on the same network — not globally across all
/// networks.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PeerId(PublicKey);

impl PeerId {
    /// Construct a peer id directly from a public key.
    #[must_use]
    pub fn new(public: PublicKey) -> Self {
        Self(public)
    }

    /// Borrow the inner public key, e.g. to check it against a signer's
    /// identity or to dispatch signature verification.
    #[must_use]
    pub fn as_public_key(&self) -> &PublicKey {
        &self.0
    }
}

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The alias rule's inverse: a key derived from an alias renders the
        // alias back; any other key renders as lowercase hex.
        if let Some(prefix) = self.0.as_bytes().strip_suffix(PUBLIC_SUFFIX) {
            if let Ok(alias) = std::str::from_utf8(prefix) {
                return f.write_str(alias);
            }
        }
        self.0.fmt(f)
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
        Ok(Self(derive_public(&PrivateKey::new(s.as_bytes().to_vec()))))
    }
}

impl serde::Serialize for PeerId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(self)
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
    use crate::crypto::mock::MockCryptoScheme;
    use crate::crypto::PublicKey;
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

    // R2 / ADR 0017: Display is the inverse of the alias-rule FromStr.
    #[test]
    fn alias_round_trips_through_display() {
        let id = PeerId::from_str("node-a").expect("valid id");
        assert_eq!(id.to_string(), "node-a");
        assert_eq!(
            PeerId::from_str(&id.to_string()).expect("valid id"),
            id,
            "FromStr ∘ Display round-trips for aliases",
        );
    }

    // R2: a key that does not stem from an alias displays as lowercase hex.
    #[test]
    fn non_alias_key_displays_as_hex() {
        let id = PeerId::new(PublicKey::new(vec![0xde, 0xad, 0xbe, 0xef]));
        assert_eq!(id.to_string(), "deadbeef");
    }

    // R2 / R9: the same alias yields the same identity through every
    // construction path — string parsing and the mock keypair factory agree.
    #[test]
    fn construction_paths_agree_on_the_same_alias() {
        let parsed = PeerId::from_str("node-a").expect("valid id");
        let scheme = MockCryptoScheme::with_seed([0u8; 32]);
        let derived = PeerId::new(scheme.keypair_from_alias("node-a").public);
        assert_eq!(parsed, derived);
        assert_ne!(
            parsed,
            PeerId::new(scheme.keypair_from_alias("node-b").public),
            "distinct aliases yield distinct identities",
        );
    }
}
