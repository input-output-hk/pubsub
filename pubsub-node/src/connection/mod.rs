//! The connection domain: the upstream-state enum and the connection-selection
//! strategy seam.
//!
//! A node holds logical, per-`(peer, topic)` connections in two roles —
//! upstream (requested; message sources, with an explicit
//! [`UpstreamState`]) and downstream (accepted; fan-out destinations). The
//! connection structures themselves live on the crate-internal node state
//! (`crate::state`); this module owns the vocabulary that names them and the
//! [`ConnectionStrategy`] trait the node consults to decide which upstreams it
//! expects to hold.
//!
//! The trait lives here; each concrete selection policy is its own submodule:
//! [`ConnectToAllCandidates`] (the v1 full-mesh policy) in
//! [`connect_to_all`], and [`SeededBoundedSelection`] (the seeded, bounded
//! policy, feature 005) in [`seeded_bounded`].

use std::collections::{HashMap, HashSet};

use crate::peer::PeerId;
use crate::topic::TopicId;

mod connect_to_all;
mod seeded_bounded;

pub use connect_to_all::ConnectToAllCandidates;
pub use seeded_bounded::SeededBoundedSelection;

/// The state of an upstream (dialer-side) connection for one `(peer, topic)`.
///
/// An upstream entry is created by the node's own strategy on a setup event in
/// [`AwaitingAccept`](UpstreamState::AwaitingAccept); it advances to
/// [`Active`](UpstreamState::Active) when the peer's `Accepted` arrives.
/// Terminal outcomes are removals, not stored states — there is no
/// closing/rejected variant.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum UpstreamState {
    /// A `Request` has been sent; the peer's `Accepted` has not yet arrived.
    /// Admits no payload.
    AwaitingAccept,
    /// The peer accepted; payload it forwards on this topic is admitted.
    Active,
}

/// The connection-selection policy a node consults on a setup event.
///
/// `expected_upstream` is **pure and synchronous**: given the node's current
/// view (the topics it is a member of and the per-topic candidate peers it has
/// discovered), it returns the set of upstream `(peer, topic)` connections the
/// node should hold. The node applies the result as a diff — it dials every
/// expected pair it does not already hold `Active`, and never removes an entry
/// on the strength of the strategy alone (selection only adds).
///
/// The trait is the seam future iterations vary (peer sampling, degree caps,
/// topology policies); the v1 implementor is [`ConnectToAllCandidates`], and the
/// seeded, bounded policy is [`SeededBoundedSelection`].
pub trait ConnectionStrategy: Send + Sync {
    /// The expected upstream set given the node's view.
    ///
    /// `subscriptions` is the node's **membership-derived** topic set (the
    /// topics it has joined), not the registration-gated effective filter —
    /// the dial side mirrors the acceptance rule, where topic registration
    /// gates delivery rather than establishment. `candidates` maps each topic
    /// to the peers discovered on it (the node's own id is never present).
    fn expected_upstream(
        &self,
        subscriptions: &HashSet<TopicId>,
        candidates: &HashMap<TopicId, HashSet<PeerId>>,
    ) -> HashSet<(PeerId, TopicId)>;
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

    /// A `Request{topic}` control event from `emitter`.
    pub(crate) fn request_from(emitter: &str, topic_id: &str) -> Event {
        control_event(
            emitter,
            ConnectionAction::Request {
                topic: topic(topic_id),
            },
        )
    }

    /// An `Accepted{topic}` control event from `emitter`.
    pub(crate) fn accepted_from(emitter: &str, topic_id: &str) -> Event {
        control_event(
            emitter,
            ConnectionAction::Accepted {
                topic: topic(topic_id),
            },
        )
    }

    /// A `Terminated{topic}` control event from `emitter`.
    pub(crate) fn terminated_from(emitter: &str, topic_id: &str) -> Event {
        control_event(
            emitter,
            ConnectionAction::Terminated {
                topic: topic(topic_id),
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

        /// Append a `ConnectionSetup` step.
        pub(crate) fn setup(mut self) -> Self {
            self.0.push(Event::ConnectionSetup);
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
