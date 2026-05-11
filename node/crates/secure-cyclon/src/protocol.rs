use crate::descriptor::{Descriptor, NodeId};

/// Gossip request from initiator to partner (paper §II.B, Fig. 1).
///
/// `peers` contains the `swap_len` descriptors offered to the partner: a
/// fresh self-descriptor plus `swap_len - 1` randomly selected peers from
/// the initiator's view. A receiver MUST reject a request whose
/// `peers.len()` exceeds `swap_len`.
#[derive(Clone, Debug)]
pub struct GossipRequest {
    pub sender: Descriptor,
    pub peers: Vec<Descriptor>,
}

/// Gossip response: `swap_len` random descriptors drawn from the partner's
/// view, excluding the initiator.
#[derive(Clone, Debug)]
pub struct GossipResponse {
    pub sender_id: NodeId,
    pub peers: Vec<Descriptor>,
}
