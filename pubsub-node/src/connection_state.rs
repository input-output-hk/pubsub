//! The connection-domain vocabulary the core owns: the unified link key
//! components ([`LinkRole`], [`LinkDirection`]), the establishment lifecycle
//! ([`LinkState`]), plus test-only declarative builders for the events that
//! drive the connection state machine.
//!
//! This is deliberately *not* a strategy — the selection/acceptance policies
//! live under [`crate::strategies`]. What lives here is the vocabulary the pure
//! core (`crate::state`) keys its **link store** on (ADR 0032): one logical
//! link per `(peer, topic, role, direction)`, whose send/receive orientation is
//! *derived* from role × direction rather than stored — for `Relay` links the
//! dialer receives (Out = message source, In = fan-out destination); for
//! `Publisher` links the dialer sends (Out = injection target for the node's
//! own published messages, In = a source of that peer's published messages).

use std::collections::BTreeMap;

use crate::peer::PeerId;
use crate::topic::TopicId;

/// Which dissemination duty a link serves (ADR 0032).
///
/// A `Relay` and a `Publisher` link between the same `(peer, topic)` coexist as
/// independent entries with independent lifecycles and caps.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum LinkRole {
    /// Full flood participation: the link carries published **and** relayed
    /// messages (the relaying link of the M3 vocabulary).
    Relay,
    /// A publishing link (the M3 S-link): carries only the dialing publisher's
    /// own locally-originated messages — never relayed traffic, in either
    /// direction (ADR 0033).
    Publisher,
}

/// Who dialed (ADR 0032). Orientation per role is derived — see [`LinkRole`].
///
/// `#[non_exhaustive]`: feature 016 (bidirectional links) may add a variant if
/// its design requires one; its baseline representation is the Out + In *pair*
/// a symmetric edge predicate produces, so the stored direction stays binary
/// here.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum LinkDirection {
    /// This node dialed.
    Out,
    /// The peer dialed.
    In,
}

/// The node's unified link store: one entry per logical link, keyed by
/// `(peer, topic, role, direction)` (ADR 0032). Ordered so snapshot and
/// shutdown-notice emission are deterministic.
pub type Links = BTreeMap<(PeerId, TopicId, LinkRole, LinkDirection), LinkState>;

/// The establishment lifecycle of a link (the former `UpstreamState`).
///
/// An `Out` entry is created by the node's own selection strategy on a dial
/// tick in [`AwaitingAccept`](LinkState::AwaitingAccept); it advances to
/// [`Active`](LinkState::Active) when the peer's `Accepted` arrives. An `In`
/// entry is recorded directly `Active` at acceptance (the acceptor has nothing
/// to await). Terminal outcomes are removals, not stored states — there is no
/// closing/rejected variant.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum LinkState {
    /// A `Request` has been sent; the peer's `Accepted` has not yet arrived.
    /// Admits no payload.
    AwaitingAccept,
    /// The link is established; payload consistent with its role × direction
    /// orientation is admitted.
    Active,
}

/// Test-only declarative constructors for the events that drive the connection
/// state machine.
///
/// Multi-step lifecycle tests read better as a script of one-line steps than as
/// inline struct literals — the constitution's declarative-test-construction
/// standard. The free constructors build a single [`Event`] each (signing
/// control and payload messages through the deterministic mock scheme), and
/// [`ConnectionScript`] chains them into an ordered `Vec<Event>` covering the
/// membership, setup, control-message, payload, and shutdown steps:
///
/// ```ignore
/// let script = ConnectionScript::new()
///     .member_joined("b", ["t"])
///     .setup()
///     .accepted_from("b", "t")
///     .shutdown();
/// for event in script { /* apply + assert per step */ }
/// ```
///
/// The whole module is gated to `cfg(test)`; `dead_code` is allowed because
/// different user-story phases exercise different subsets of the steps.
#[cfg(test)]
#[allow(dead_code)]
pub(crate) mod test_support {
    use std::str::FromStr;

    use super::LinkRole;
    use crate::crypto::mock::MockCryptoScheme;
    use crate::crypto::{Signer, Timestamp};
    use crate::event::Event;
    use crate::message::{
        ConnectionAction, ConnectionMessage, Message, MessagePayload, PlainConnection,
        PlainMessage, PublisherId, SignedMessage,
    };
    use crate::peer::PeerId;
    use crate::subscription_registry::MembershipEvent;
    use crate::topic::TopicId;

    fn peer(s: &str) -> PeerId {
        PeerId::from_str(s).expect("valid peer id")
    }

    fn topic(s: &str) -> TopicId {
        TopicId::from_str(s).expect("valid topic id")
    }

    /// A signer for the alias's keypair (deterministic; agrees with
    /// `PeerId::from_str(alias)` by construction).
    fn alias_signer(alias: &str) -> impl Signer {
        let scheme = MockCryptoScheme::with_seed([0u8; 32]);
        scheme.signer(scheme.keypair_from_alias(alias).private)
    }

    /// A signed control message from `emitter` carrying `action`.
    fn control(emitter: &str, action: ConnectionAction) -> Message {
        let plain = PlainConnection {
            emitter: peer(emitter),
            action,
        };
        let signature = alias_signer(emitter).sign(&plain.signed_bytes());
        Message::Connection(ConnectionMessage { plain, signature })
    }

    /// A control-message `Event` (the frame `from` is set to the emitter; the
    /// control path keys on the carried emitter, not the frame).
    fn control_event(emitter: &str, action: ConnectionAction) -> Event {
        Event::MessageReceived {
            from: peer(emitter),
            message: control(emitter, action),
        }
    }

    /// A self-membership / candidate `MembershipUpdate` event.
    pub(crate) fn membership_joined<const N: usize>(node: &str, topics: [&str; N]) -> Event {
        Event::MembershipUpdate(MembershipEvent::joined(node, topics))
    }

    /// A relay `Request{topic}` control event from `emitter`.
    pub(crate) fn request_from(emitter: &str, topic_id: &str) -> Event {
        control_event(
            emitter,
            ConnectionAction::Request {
                topic: topic(topic_id),
                role: LinkRole::Relay,
            },
        )
    }

    /// A publish-intent `Request{topic}` control event from `emitter`
    /// (feature 015).
    pub(crate) fn publish_request_from(emitter: &str, topic_id: &str) -> Event {
        control_event(
            emitter,
            ConnectionAction::Request {
                topic: topic(topic_id),
                role: LinkRole::Publisher,
            },
        )
    }

    /// A relay `Accepted{topic}` control event from `emitter`.
    pub(crate) fn accepted_from(emitter: &str, topic_id: &str) -> Event {
        control_event(
            emitter,
            ConnectionAction::Accepted {
                topic: topic(topic_id),
                role: LinkRole::Relay,
            },
        )
    }

    /// A publish `Accepted{topic}` control event from `emitter` (feature 015).
    pub(crate) fn publish_accepted_from(emitter: &str, topic_id: &str) -> Event {
        control_event(
            emitter,
            ConnectionAction::Accepted {
                topic: topic(topic_id),
                role: LinkRole::Publisher,
            },
        )
    }

    /// A relay `Terminated{topic}` control event from `emitter`.
    pub(crate) fn terminated_from(emitter: &str, topic_id: &str) -> Event {
        control_event(
            emitter,
            ConnectionAction::Terminated {
                topic: topic(topic_id),
                role: LinkRole::Relay,
            },
        )
    }

    /// A publish `Terminated{topic}` control event from `emitter` (feature 015).
    pub(crate) fn publish_terminated_from(emitter: &str, topic_id: &str) -> Event {
        control_event(
            emitter,
            ConnectionAction::Terminated {
                topic: topic(topic_id),
                role: LinkRole::Publisher,
            },
        )
    }

    /// A relay `Rejected{topic}` control event from `emitter` (acceptor →
    /// dialer, over-capacity refusal; feature 005).
    pub(crate) fn rejected_from(emitter: &str, topic_id: &str) -> Event {
        control_event(
            emitter,
            ConnectionAction::Rejected {
                topic: topic(topic_id),
                role: LinkRole::Relay,
            },
        )
    }

    /// A control message signed by `signing_alias` but claiming a different
    /// `emitter_alias` — its signature does not verify under the carried
    /// emitter's key (the control invalid-signature case).
    pub(crate) fn misattributed_request(
        emitter_alias: &str,
        signing_alias: &str,
        topic_id: &str,
    ) -> Event {
        let plain = PlainConnection {
            emitter: peer(emitter_alias),
            action: ConnectionAction::Request {
                topic: topic(topic_id),
                role: LinkRole::Relay,
            },
        };
        let signature = alias_signer(signing_alias).sign(&plain.signed_bytes());
        Event::MessageReceived {
            from: peer(emitter_alias),
            message: Message::Connection(ConnectionMessage { plain, signature }),
        }
    }

    fn signed_payload_message(publisher: &str, topic_id: &str, n: u64, tampered: bool) -> Message {
        let scheme = MockCryptoScheme::with_seed([0u8; 32]);
        let signer = scheme.signer(scheme.keypair_from_alias(publisher).private);
        let plain = PlainMessage {
            topic: topic(topic_id),
            publisher_id: PublisherId::new(signer.public_key()),
            parent_hash: None,
            sequence: 0,
            timestamp: Timestamp::from_millis(0),
            payload: MessagePayload::Ping(n),
        };
        let signature = signer.sign(&plain.signed_bytes());
        let mut msg = SignedMessage { plain, signature };
        if tampered {
            msg.plain.payload = MessagePayload::Ping(n.wrapping_add(1));
        }
        Message::Dissemination(msg)
    }

    /// A validly-signed payload `Ping(n)` event from `publisher` on `topic`
    /// (the frame `from` is the publisher).
    pub(crate) fn payload_from(publisher: &str, topic_id: &str, n: u64) -> Event {
        Event::MessageReceived {
            from: peer(publisher),
            message: signed_payload_message(publisher, topic_id, n, false),
        }
    }

    /// A payload event whose signature no longer matches its content.
    pub(crate) fn tampered_payload_from(publisher: &str, topic_id: &str, n: u64) -> Event {
        Event::MessageReceived {
            from: peer(publisher),
            message: signed_payload_message(publisher, topic_id, n, true),
        }
    }

    /// An ordered connection-lifecycle script, built one step per line.
    pub(crate) struct ConnectionScript(Vec<Event>);

    impl ConnectionScript {
        pub(crate) fn new() -> Self {
            Self(Vec::new())
        }

        /// Append a `MembershipUpdate(Joined)` step (candidate convergence).
        pub(crate) fn member_joined<const N: usize>(
            mut self,
            node: &str,
            topics: [&str; N],
        ) -> Self {
            self.0.push(membership_joined(node, topics));
            self
        }

        /// Append a `Synced` readiness step. Scripts that deliver inbound
        /// `Request`s need it first: the transition drops requests until the
        /// registry snapshots are folded (the fail-open gate). Placed before any
        /// membership step it is effect-free (the readiness dial pass sees no
        /// candidates).
        pub(crate) fn synced(mut self) -> Self {
            self.0.push(Event::Synced);
            self
        }

        /// Append a `Heartbeat` step (the dial tick; the single v1 heartbeat).
        pub(crate) fn setup(mut self) -> Self {
            self.0.push(Event::Heartbeat);
            self
        }

        /// Append an inbound `Request` step.
        pub(crate) fn request_from(mut self, emitter: &str, topic_id: &str) -> Self {
            self.0.push(request_from(emitter, topic_id));
            self
        }

        /// Append an inbound `Accepted` step.
        pub(crate) fn accepted_from(mut self, emitter: &str, topic_id: &str) -> Self {
            self.0.push(accepted_from(emitter, topic_id));
            self
        }

        /// Append an inbound `Terminated` step.
        pub(crate) fn terminated_from(mut self, emitter: &str, topic_id: &str) -> Self {
            self.0.push(terminated_from(emitter, topic_id));
            self
        }

        /// Append an inbound `Rejected` step (over-capacity refusal of a dial).
        pub(crate) fn rejected_from(mut self, emitter: &str, topic_id: &str) -> Self {
            self.0.push(rejected_from(emitter, topic_id));
            self
        }

        /// Append a validly-signed payload step.
        pub(crate) fn payload_from(mut self, publisher: &str, topic_id: &str, n: u64) -> Self {
            self.0.push(payload_from(publisher, topic_id, n));
            self
        }

        /// Append a tampered-payload step.
        pub(crate) fn tampered_payload_from(
            mut self,
            publisher: &str,
            topic_id: &str,
            n: u64,
        ) -> Self {
            self.0.push(tampered_payload_from(publisher, topic_id, n));
            self
        }

        /// Append a `Shutdown` step.
        pub(crate) fn shutdown(mut self) -> Self {
            self.0.push(Event::Shutdown);
            self
        }
    }

    impl IntoIterator for ConnectionScript {
        type Item = Event;
        type IntoIter = std::vec::IntoIter<Event>;

        fn into_iter(self) -> Self::IntoIter {
            self.0.into_iter()
        }
    }
}
