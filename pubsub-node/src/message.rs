use std::fmt;

use crate::crypto::{MessageHash, PublicKey, Signature, Timestamp};
use crate::peer::PeerId;
use crate::topic::TopicId;

/// Append `bytes` to `out` as a `u32` big-endian length prefix followed by the
/// bytes themselves — the one length-prefix primitive shared by every
/// `signed_bytes()` encoder (`PlainMessage`, `PlainConnection`).
///
/// Keeping the canonical signing-byte encoding in a single primitive is what
/// lets a future change touch one place: in particular, **signature domain
/// separation** (a per-message-kind tag so the signature commits to "this is a
/// dissemination message" vs "this is a control message") is deferred to the
/// real-serialization milestone — see `IMPLEMENTATION_NOTES.md` N-016 — and
/// would be introduced here / at each encoder's first write.
fn push_len_prefixed(out: &mut Vec<u8>, bytes: &[u8]) {
    let len = u32::try_from(bytes.len()).expect("field length fits in u32");
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(bytes);
}

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
/// Two variants exist: [`Message::Dissemination`], a signed dissemination
/// message, and [`Message::Connection`], a signed connection-control message.
/// Both are signed; the variant axis is dissemination-vs-control. The enum
/// is `#[non_exhaustive]` so future protocol-message kinds (peer sampling,
/// registry lookups, …) can be added as sibling variants without breaking
/// external consumers — pattern-matches outside this crate must include a
/// catch-all arm.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Message {
    /// A signed dissemination message: signed-over content plus a signature.
    Dissemination(SignedMessage),
    /// A signed connection-control message: a handshake action
    /// (`Request`/`Accepted`/`Terminated`) plus a signature over its content.
    Connection(ConnectionMessage),
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

/// A complete signed connection-control message: the signed-over
/// [`PlainConnection`] content together with the [`Signature`] over its
/// canonical bytes.
///
/// The sibling of [`SignedMessage`] for the connection-control protocol
/// (ADR 0010 hierarchy, ADR 0017). The emitting node's signer produces the
/// signature; the receiver verifies it against the emitter's key carried
/// inside `plain` — the transport frame's sender is not consulted on this
/// path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectionMessage {
    /// The signed-over content.
    pub plain: PlainConnection,
    /// The signature over `plain.signed_bytes()`.
    pub signature: Signature,
}

/// The signed-over content of a connection-control message: the emitting
/// node's identity and the handshake action.
///
/// The canonical signing-byte encoding lives on this type
/// ([`PlainConnection::signed_bytes`]); the signature binds the emitter, the
/// action kind, and the topic together.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlainConnection {
    /// The node that emitted (and signs) this message — the control-path
    /// identity, carried inside the signed content.
    pub emitter: PeerId,
    /// The handshake action and its topic.
    pub action: ConnectionAction,
}

/// A connection-handshake action, each carrying the topic it concerns.
///
/// Marked `#[non_exhaustive]`: a `Rejected` variant arrives with the deny-path
/// package (a ROADMAP-justified forward shape), so external pattern-matches
/// must include a catch-all arm. Tag bytes for the signing encoding are
/// assigned explicitly in [`PlainConnection::signed_bytes`].
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConnectionAction {
    /// Dialer → acceptor: request an upstream connection on `topic`.
    Request {
        /// The topic the connection is for.
        topic: TopicId,
    },
    /// Acceptor → dialer: accept the requested connection on `topic`.
    Accepted {
        /// The topic the connection is for.
        topic: TopicId,
    },
    /// Either role → counterpart: tear down the connection on `topic`.
    Terminated {
        /// The topic the connection was for.
        topic: TopicId,
    },
}

impl PlainConnection {
    /// Encode the canonical signing bytes for this control message.
    ///
    /// Hand-rolled, length-prefixed concatenation in the
    /// [`PlainMessage::signed_bytes`] style. There is no leading version tag.
    /// Multi-byte integers are big-endian. Fields, in order:
    ///
    /// 1. emitter key — `u32` byte length, then the emitter's public-key bytes.
    /// 2. action — a 1-byte tag, then the topic as `u32` byte length + UTF-8
    ///    bytes. Tags are assigned explicitly so future variants append new
    ///    values without disturbing the existing ones: `0x00` Request,
    ///    `0x01` Accepted, `0x02` Terminated.
    ///
    /// The signature is produced over exactly these bytes, binding emitter
    /// identity, action kind, and topic together. Any layout change is a
    /// protocol change and must update this documentation in the same commit.
    #[must_use]
    pub fn signed_bytes(&self) -> Vec<u8> {
        let (tag, topic) = match &self.action {
            ConnectionAction::Request { topic } => (0x00u8, topic),
            ConnectionAction::Accepted { topic } => (0x01u8, topic),
            ConnectionAction::Terminated { topic } => (0x02u8, topic),
        };

        let mut out = Vec::new();
        push_len_prefixed(&mut out, self.emitter.as_public_key().as_bytes());
        out.push(tag);
        push_len_prefixed(&mut out, topic.as_str().as_bytes());
        out
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::{ConnectionAction, PlainConnection};
    use crate::crypto::mock::MockCryptoScheme;
    use crate::crypto::mock::TestVerifier;
    use crate::crypto::{Signer, Verifier};
    use crate::peer::PeerId;
    use crate::topic::TopicId;

    fn peer(s: &str) -> PeerId {
        PeerId::from_str(s).expect("valid peer id")
    }

    fn topic(s: &str) -> TopicId {
        TopicId::from_str(s).expect("valid topic id")
    }

    // FR-011 / contracts §1.1: the signing-byte layout is stable and explicit.
    #[test]
    fn signed_bytes_layout_is_stable() {
        let plain = PlainConnection {
            emitter: peer("a"),
            action: ConnectionAction::Request { topic: topic("t1") },
        };
        // emitter key = alias bytes + mock public suffix = b"a_public" (len 8);
        // tag 0x00 (Request); topic "t1" (len 2).
        let mut expected = Vec::new();
        expected.extend_from_slice(&8u32.to_be_bytes());
        expected.extend_from_slice(b"a_public");
        expected.push(0x00);
        expected.extend_from_slice(&2u32.to_be_bytes());
        expected.extend_from_slice(b"t1");
        assert_eq!(plain.signed_bytes(), expected);
    }

    // The action tag distinguishes the three kinds on the wire.
    #[test]
    fn action_tags_are_distinct() {
        let bytes = |action| {
            PlainConnection {
                emitter: peer("a"),
                action,
            }
            .signed_bytes()
        };
        let req = bytes(ConnectionAction::Request { topic: topic("t1") });
        let acc = bytes(ConnectionAction::Accepted { topic: topic("t1") });
        let term = bytes(ConnectionAction::Terminated { topic: topic("t1") });
        assert_ne!(req, acc);
        assert_ne!(acc, term);
        assert_ne!(req, term);
    }

    // FR-011: an emitter signs its own control message and it verifies under
    // the emitter's key.
    #[test]
    fn sign_verify_round_trip() {
        let scheme = MockCryptoScheme::with_seed([0u8; 32]);
        let kp = scheme.keypair_from_alias("a");
        let signer = scheme.signer(kp.private);
        let plain = PlainConnection {
            emitter: peer("a"),
            action: ConnectionAction::Request { topic: topic("t1") },
        };
        let sig = signer.sign(&plain.signed_bytes());
        assert!(TestVerifier
            .verify(plain.emitter.as_public_key(), &plain.signed_bytes(), &sig)
            .is_ok());
    }

    // FR-011: the signature binds emitter, kind, and topic — tampering with any
    // one makes the original signature no longer verify.
    #[test]
    fn tamper_on_any_bound_field_breaks_signature() {
        let scheme = MockCryptoScheme::with_seed([0u8; 32]);
        let kp = scheme.keypair_from_alias("a");
        let signer = scheme.signer(kp.private);
        let original = PlainConnection {
            emitter: peer("a"),
            action: ConnectionAction::Request { topic: topic("t1") },
        };
        let sig = signer.sign(&original.signed_bytes());
        let key = original.emitter.as_public_key().clone();

        // Tamper the emitter (the field bound into the bytes).
        let tampered_emitter = PlainConnection {
            emitter: peer("b"),
            action: ConnectionAction::Request { topic: topic("t1") },
        };
        assert!(TestVerifier
            .verify(&key, &tampered_emitter.signed_bytes(), &sig)
            .is_err());

        // Tamper the kind.
        let tampered_kind = PlainConnection {
            emitter: peer("a"),
            action: ConnectionAction::Accepted { topic: topic("t1") },
        };
        assert!(TestVerifier
            .verify(&key, &tampered_kind.signed_bytes(), &sig)
            .is_err());

        // Tamper the topic.
        let tampered_topic = PlainConnection {
            emitter: peer("a"),
            action: ConnectionAction::Request { topic: topic("t2") },
        };
        assert!(TestVerifier
            .verify(&key, &tampered_topic.signed_bytes(), &sig)
            .is_err());
    }
}
