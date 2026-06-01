use crate::topic::TopicId;

/// The body of a [`Message`].
///
/// Currently only [`MessagePayload::Ping`] is defined; the enum is marked
/// `#[non_exhaustive]` so future iterations can add variants without
/// breaking external consumers that match non-exhaustively.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MessagePayload {
    /// A connectivity-probe payload carrying an opaque numeric value.
    Ping(u64),
}

/// An envelope carrying a [`MessagePayload`] tagged with a [`TopicId`].
///
/// Every message exchanged on the network carries a topic. The topic is a
/// first-class field — receive-side filtering keys on it and forwarded
/// deliveries observe it on the envelope as a whole, never on a
/// topic-stripped payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Message {
    /// The topic this message is tagged with.
    pub topic: TopicId,
    /// The message body.
    pub payload: MessagePayload,
}

impl Message {
    /// Build a [`Message`] carrying a [`MessagePayload::Ping`].
    #[must_use]
    pub fn ping(topic: TopicId, n: u64) -> Self {
        Self {
            topic,
            payload: MessagePayload::Ping(n),
        }
    }
}
