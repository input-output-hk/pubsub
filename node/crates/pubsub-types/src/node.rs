use std::fmt;
use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

/// Derive a canonical `NodeId` from an Ed25519 public key using BLAKE2b-256.
///
/// Relay node identity comes from cryptographic keys (D2 paper, Ch.3).
/// This is the authoritative source of identity; the key is propagated through
/// Cyclon gossip as part of each node's `PeerDescriptor`.
pub fn node_id_from_key(public_key: &[u8]) -> NodeId {
    use pallas_crypto::hash::Hasher;
    let hash = Hasher::<256>::hash(public_key);
    let mut id = [0u8; 32];
    id.copy_from_slice(hash.as_ref());
    NodeId(id)
}

/// Derive a temporary `NodeId` from a socket address using BLAKE2b-256.
///
/// Used only by the transport layer to label inbound connections before
/// the peer's public key is known (i.e., before the first Cyclon gossip exchange).
/// The placeholder is overwritten once the peer sends its `PeerDescriptor`.
pub fn node_id_from_addr(addr: SocketAddr) -> NodeId {
    use pallas_crypto::hash::Hasher;
    let hash = Hasher::<256>::hash(addr.to_string().as_bytes());
    let mut id = [0u8; 32];
    id.copy_from_slice(hash.as_ref());
    NodeId(id)
}

/// Relay node identity, derived from the node's Ed25519 public key.
/// Propagated through Cyclon gossip as part of each node's `PeerDescriptor`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub [u8; 32]);

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for b in &self.0[..4] {
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}

/// Information about a relay node, exchanged through Cyclon gossip.
///
/// Relay nodes join the overlay permissionlessly via gossip — they do not register
/// on-chain. The on-chain Node Registry contract (D2 Ch.4) is for replication
/// servers only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Unique node identifier (derived from the node's Ed25519 public key)
    pub node_id: NodeId,

    /// Network address for PubSub protocol connections
    pub addr: SocketAddr,

    /// Ed25519 public key for message authentication between nodes
    pub public_key: Vec<u8>,

    /// Topics this node is interested in (used by Vicinity for navigation)
    pub subscribed_topics: Vec<crate::message::TopicId>,
}

/// View entry in Cyclon — a peer descriptor exchanged during gossip
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerDescriptor {
    pub node_info: NodeInfo,

    /// Age counter — incremented each Cyclon cycle, used to evict stale entries
    pub age: u32,

    /// Ed25519 signature of `signing_bytes()` by the originating node's private key.
    /// Empty for descriptors created locally before signing is set up (e.g. bootstrap seeds).
    #[serde(default)]
    pub signature: Vec<u8>,
}

impl PeerDescriptor {
    /// Construct an unsigned descriptor (signature empty).
    pub fn unsigned(node_info: NodeInfo, age: u32) -> Self {
        Self { node_info, age, signature: vec![] }
    }

    /// Stable byte representation over which the owning node signs.
    /// Encodes `node_id || public_key` — enough to verify self-certifying identity.
    pub fn signing_bytes(info: &NodeInfo) -> Vec<u8> {
        let mut out = Vec::with_capacity(32 + info.public_key.len());
        out.extend_from_slice(&info.node_id.0);
        out.extend_from_slice(&info.public_key);
        out
    }

    /// Verify the descriptor's signature against the public key carried in `node_info`.
    ///
    /// Returns `None` if the descriptor is unsigned (empty signature), `Some(true)` if
    /// the signature is valid, `Some(false)` if it is present but invalid.
    pub fn verify_signature(&self) -> Option<bool> {
        use pallas_crypto::key::ed25519::{PublicKey, Signature};
        if self.signature.is_empty() {
            return None;
        }
        let Ok(pk) = PublicKey::try_from(self.node_info.public_key.as_ref()) else {
            return Some(false);
        };
        let Ok(sig) = Signature::try_from(self.signature.as_slice()) else {
            return Some(false);
        };
        let msg = Self::signing_bytes(&self.node_info);
        Some(pk.verify(&msg, &sig))
    }
}
