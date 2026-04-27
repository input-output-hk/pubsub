use async_trait::async_trait;
use tracing::trace;

use pubsub_types::message::Message;
use pubsub_types::node::NodeId;
use pubsub_types::traits::{RelayDecision, RelayPolicy};

/// Phase 1 relay policy: unconditionally forward all valid messages.
///
/// Future phases will add:
/// - BFT consistency checks
/// - Per-publisher rate limiting
/// - Reputation-weighted relay decisions
#[derive(Default)]
pub struct DefaultRelayPolicy;

#[async_trait]
impl RelayPolicy for DefaultRelayPolicy {
    async fn should_relay(&self, msg: &Message, from: &NodeId) -> RelayDecision {
        trace!(
            topic = %msg.topic_id,
            seq = msg.sequence_nr,
            from = %from,
            "DefaultRelayPolicy: forwarding message"
        );
        RelayDecision::Forward
    }
}
