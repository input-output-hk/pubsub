use crate::message::Message;
use crate::peer::PeerId;

/// One observed delivery: the sender's id and the message payload.
///
/// Returned from [`Node::received_messages`](crate::Node::received_messages)
/// as part of a snapshot — the returned value is stable for the caller and
/// unaffected by subsequent receptions on the same node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceivedDelivery {
    /// The id of the peer that originated this message.
    pub from: PeerId,
    /// The message payload.
    pub message: Message,
}
