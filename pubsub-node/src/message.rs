use std::fmt;

use crate::crypto::PublicKey;
use crate::topic::TopicId;

/// Identifies the entity whose private key signed a message.
///
/// A thin newtype over [`PublicKey`], distinct at the type level from
/// [`PeerId`](crate::PeerId): a `PublisherId` names the originator of a
/// message, whereas a `PeerId` names the network neighbour that forwarded it.
/// The compiler keeps the two roles from being used interchangeably even when
/// they wrap the same bytes.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PublisherId(PublicKey);

impl PublisherId {
    /// Construct a publisher id from a public key.
    #[must_use]
    pub fn new(public: PublicKey) -> Self {
        Self(public)
    }

    /// Borrow the inner public key, e.g. to dispatch signature verification.
    #[must_use]
    pub fn as_public_key(&self) -> &PublicKey {
        &self.0
    }
}

impl From<PublicKey> for PublisherId {
    fn from(public: PublicKey) -> Self {
        Self(public)
    }
}

impl fmt::Display for PublisherId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

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
