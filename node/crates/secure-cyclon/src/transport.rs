use std::net::SocketAddr;

use async_trait::async_trait;

use crate::descriptor::NodeId;
use crate::error::Result;
use crate::protocol::{GossipRequest, GossipResponse};

/// Outbound transport for a Cyclon gossip exchange.
///
/// The transport surface is intentionally minimal: only the initiator side
/// is exposed here. Responder dispatch happens through
/// [`Cyclon::handle_inbound`](../cyclon/struct.Cyclon.html#method.handle_inbound),
/// invoked by whichever runtime drives the node (the in-memory simulator for
/// tests; the QUIC service in a future crate).
#[async_trait]
pub trait Transport: Send + Sync {
    async fn exchange(
        &self,
        peer: &NodeId,
        addr: SocketAddr,
        request: GossipRequest,
    ) -> Result<GossipResponse>;
}
