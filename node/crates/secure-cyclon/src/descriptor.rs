use std::net::SocketAddr;

/// Unique identifier for a node. A 32-byte value; in production it is a
/// node's Ed25519 public key (paper §IV.A), but this crate treats it as
/// opaque bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId([u8; 32]);

impl NodeId {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::with_capacity(10);
        for b in self.0.iter().take(4) {
            s.push_str(&format!("{b:02x}"));
        }
        write!(f, "NodeId({s}…)")
    }
}

/// A Cyclon peer descriptor: who the peer is, how to reach it, and when
/// the descriptor was created.
#[derive(Clone, Debug)]
pub struct Descriptor {
    pub node: NodeId,
    pub addr: SocketAddr,
    pub created_at: u64,
}

impl Descriptor {
    pub fn fresh(node: NodeId, addr: SocketAddr, now_ms: u64) -> Self {
        Self {
            node,
            addr,
            created_at: now_ms,
        }
    }
}
