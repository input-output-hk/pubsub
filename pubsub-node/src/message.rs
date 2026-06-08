use std::fmt;

use crate::crypto::{MessageHash, PublicKey, Signature, Timestamp};
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

/// A protocol message exchanged between nodes.
///
/// Currently the only variant is [`Message::Signed`], a signed dissemination
/// message. The enum is `#[non_exhaustive]` so future protocol-message kinds
/// (connection control, peer sampling, registry lookups, …) can be added as
/// sibling variants without breaking external consumers — pattern-matches
/// outside this crate must include a catch-all arm.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Message {
    /// A signed dissemination message: signed-over content plus a signature.
    Signed(SignedMessage),
}

/// A complete signed dissemination message: the signed-over [`PlainMessage`]
/// content together with the [`Signature`] over its canonical bytes.
///
/// This is the "envelope" of the staged design — the whole signed message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedMessage {
    /// The signed-over content.
    pub plain: PlainMessage,
    /// The signature over `plain.signed_bytes()`.
    pub signature: Signature,
}

/// The signed-over content of a dissemination message: every envelope field
/// except the signature.
///
/// The canonical signing-byte encoding lives on this type
/// ([`PlainMessage::signed_bytes`]); the signature is produced over those
/// bytes and held alongside in a [`SignedMessage`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlainMessage {
    /// The topic this message is tagged with.
    pub topic: TopicId,
    /// The originator of the message (whose key signs it).
    pub publisher_id: PublisherId,
    /// Hash of this publisher's previous message on this topic, if any.
    pub parent_hash: Option<MessageHash>,
    /// Per-publisher monotonic sequence number.
    pub sequence: u64,
    /// Advisory publication timestamp (Unix-epoch milliseconds).
    pub timestamp: Timestamp,
    /// The application payload.
    pub payload: MessagePayload,
}

impl PlainMessage {
    /// Encode the canonical signing bytes for this message.
    ///
    /// This is the single seam over which signatures are produced and verified,
    /// and the input to [`MessageHash::of`]. Any change to the layout is a
    /// protocol change and must update this documentation in the same commit.
    ///
    /// The layout is a hand-rolled, length-prefixed concatenation. There is no
    /// leading version tag. Multi-byte integers are big-endian. Fields, in
    /// order:
    ///
    /// 1. topic — `u32` byte length, then the topic's UTF-8 bytes.
    /// 2. publisher key — `u32` byte length, then the public-key bytes.
    /// 3. parent hash — exactly 32 bytes; the all-zero [`MessageHash::ZERO`]
    ///    sentinel encodes an absent parent.
    /// 4. sequence — 8 bytes (`u64`).
    /// 5. timestamp — 8 bytes (`u64` milliseconds).
    /// 6. payload — `u32` byte length, then the payload encoding.
    ///
    /// The payload encoding is a 1-byte variant tag followed by the variant's
    /// body. Tags are assigned explicitly (not by declaration order), so future
    /// [`MessagePayload`] variants append new tag values without disturbing the
    /// existing ones:
    ///
    /// - `0x00` — `Ping(n)`: the tag byte then `n` as 8 big-endian bytes.
    #[must_use]
    pub fn signed_bytes(&self) -> Vec<u8> {
        fn push_len_prefixed(out: &mut Vec<u8>, bytes: &[u8]) {
            let len = u32::try_from(bytes.len()).expect("field length fits in u32");
            out.extend_from_slice(&len.to_be_bytes());
            out.extend_from_slice(bytes);
        }

        let mut out = Vec::new();
        push_len_prefixed(&mut out, self.topic.as_str().as_bytes());
        push_len_prefixed(&mut out, self.publisher_id.as_public_key().as_bytes());

        let parent = self.parent_hash.as_ref().unwrap_or(&MessageHash::ZERO);
        out.extend_from_slice(parent.as_bytes());

        out.extend_from_slice(&self.sequence.to_be_bytes());
        out.extend_from_slice(&self.timestamp.as_millis().to_be_bytes());

        let mut payload_encoded = Vec::new();
        match &self.payload {
            MessagePayload::Ping(n) => {
                payload_encoded.push(0x00);
                payload_encoded.extend_from_slice(&n.to_be_bytes());
            }
        }
        push_len_prefixed(&mut out, &payload_encoded);

        out
    }
}
