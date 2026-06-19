use crate::message::Message;
use crate::peer::PeerId;

/// Where a recorded delivery came from.
///
/// A received message carries the [`Peer`](Origin::Peer) that forwarded it; a
/// locally-published message carries [`Local`](Origin::Local) — it has no wire
/// sender. The *publisher* identity is distinct from the origin and lives inside
/// the message (`plain.publisher_id`); a forwarding peer is not necessarily the
/// publisher.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Origin {
    /// The message was published on this node (`Node::publish`); it has no wire
    /// sender.
    Local,
    /// The message was forwarded by this peer (the delivering peer, not
    /// necessarily the publisher).
    Peer(PeerId),
}

/// One observed delivery: where it came from and the message payload.
///
/// Returned from [`Node::received_messages`](crate::Node::received_messages)
/// as part of a snapshot — the returned value is stable for the caller and
/// unaffected by subsequent receptions on the same node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceivedDelivery {
    /// Where this delivery came from: [`Origin::Local`] for a message this node
    /// published, or [`Origin::Peer`] for the peer that forwarded it.
    pub origin: Origin,
    /// The message payload.
    pub message: Message,
}
