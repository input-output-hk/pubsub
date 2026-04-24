use std::fmt;
use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

/// Derive a stable `NodeId` from a socket address using BLAKE2b-256.
///
/// This is the single canonical function for address-based NodeId derivation
/// used everywhere in the testnet stack (transport, registry, main).
pub fn node_id_from_addr(addr: SocketAddr) -> NodeId {
    use blake2::digest::consts::U32;
    use blake2::{Blake2b, Digest};
    type Blake2b256 = Blake2b<U32>;
    let mut h = Blake2b256::new();
    h.update(addr.to_string().as_bytes());
    let result = h.finalize();
    let mut id = [0u8; 32];
    id.copy_from_slice(&result);
    NodeId(id)
}

/// Identity of a registered PubSub relay node.
/// In production, this comes from on-chain registration.
/// In testnet, this is provided by mock config.
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

/// Information about a registered relay node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Unique node identifier (derived from public key)
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
}
