//! The node's crate-internal pure core: the explicit state value and the
//! synchronous transition function the event loop drives.
//!
//! [`NodeState`] is a plain struct — no channels, no tasks, no interior
//! locking — so it is constructible and drivable in a synchronous unit test.
//! All mutation — including the node's own subscription set — goes through
//! [`apply`], which performs no protocol I/O and returns the outbound commands
//! ([`Effect`]) the shell must execute. The subscription set is **derived** from
//! the registry membership stream (the node's own entry); the node has no local
//! subscribe/unsubscribe mutator (the subscription list is the source of truth,
//! ADR 0013/0014/0015). Operator log events are emitted inline at the decision
//! sites; they are ambient observability, not part of the transition's contract.
//!
//! The shell side (queue, event loop, producers) lives in `crate::node`.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use crate::crypto::{PublicKey, Verifier};
use crate::event::Event;
use crate::message::{ConnectionMessage, Message, SignedMessage};
use crate::peer::PeerId;
use crate::received::ReceivedDelivery;
use crate::subscription_registry::MembershipEvent;
use crate::topic::TopicId;
use crate::topic_registry::TopicRegistryEvent;

/// The node's full mutable state as one explicit value.
///
/// Mutated only under the shell's lock, exclusively via [`apply`] (sole
/// caller: the event loop) — including the subscription set, which is folded
/// from the node's own entry on the registry membership stream rather than a
/// local mutator. The verifier rides along as the immutable service handle the
/// message-received transition consults.
///
/// Holds what the transition reads or writes — nothing more: static
/// shell concerns (the network handle, the config-derived peer list) stay on
/// the node. Peer or registry-derived data joins this struct when a
/// transition first consumes it.
// FR-008: single explicit state value; crate-internal (Clarifications
// 2026-06-09). Field set per the seam contract §1.1 / data-model.md; the
// peers-placement boundary is IMPLEMENTATION_NOTES N-007 (revisit at 008/005).
pub(crate) struct NodeState {
    self_id: PeerId,
    subscriptions: HashSet<TopicId>,
    received: Vec<ReceivedDelivery>,
    verifier: Arc<dyn Verifier>,
    /// Per-topic candidate peers, folded from the subscription-registry stream
    /// (`Event::MembershipUpdate`). The node's own id is never present. This is
    /// the topic-derived peer set, distinct from the shell's static config
    /// `peers` bootstrap list (`IMPLEMENTATION_NOTES` N-007).
    candidates: HashMap<TopicId, HashSet<PeerId>>,
    /// Registered topics → their authorized publisher keys (empty ⇒ open),
    /// folded from the topic-registry stream (`Event::TopicRegistryUpdate`).
    /// Written only by `handle_topic_registry_update`. The node's **effective**
    /// subscription set — its message accept-filter — is `subscriptions`
    /// intersected with the keys here; a subscribed topic absent here is not yet
    /// (or no longer) a legitimate topic, so its traffic is dropped.
    registered_topics: HashMap<TopicId, BTreeSet<PublicKey>>,
}

impl NodeState {
    /// Construct the state value from already-parsed inputs.
    pub(crate) fn new(
        self_id: PeerId,
        subscriptions: HashSet<TopicId>,
        verifier: Arc<dyn Verifier>,
    ) -> Self {
        Self {
            self_id,
            subscriptions,
            received: Vec::new(),
            verifier,
            candidates: HashMap::new(),
            registered_topics: HashMap::new(),
        }
    }

    /// Snapshot of every recorded delivery, in processing order.
    #[must_use]
    pub(crate) fn received_snapshot(&self) -> Vec<ReceivedDelivery> {
        self.received.clone()
    }

    /// Snapshot of the node's subscription set — the actual message
    /// accept-filter (unspecified order): the topics the node both declared
    /// (its subscription-list entry) **and** that are registered (legitimate)
    /// in the topic registry, i.e. `subscriptions ∩ registered_topics`. A
    /// declared topic that is not a registered topic is excluded (it has no
    /// effect — traffic on it is dropped). The declared set and the
    /// registered-topics projection remain separate internal fields; only this
    /// intersection is observable.
    #[must_use]
    pub(crate) fn subscriptions_snapshot(&self) -> Vec<TopicId> {
        self.subscriptions
            .iter()
            .filter(|topic| self.registered_topics.contains_key(*topic))
            .cloned()
            .collect()
    }

    /// Snapshot of the candidate peers for `topic` (unspecified order; the
    /// node's own id is never included). Empty if the topic has no members.
    #[must_use]
    pub(crate) fn candidates_snapshot(&self, topic: &TopicId) -> Vec<PeerId> {
        self.candidates
            .get(topic)
            .map(|peers| peers.iter().cloned().collect())
            .unwrap_or_default()
    }
}

/// Outbound commands the shell executes on the transition's behalf.
///
/// The transition itself performs no protocol I/O; it returns these and the
/// shell's effect executor (in `crate::node`) carries them out outside the
/// state lock. Crate-internal, like [`NodeState`].
// The executor matches both variants, but no `apply` arm constructs an effect
// yet — the connection transitions that produce them land with User Story 1
// (tasks T009–T011). The allow keeps this inert checkpoint warning-clean and
// is removed once the first constructor exists.
#[allow(dead_code)]
pub(crate) enum Effect {
    /// Send `message` to the peer registered under `to`. Every wire action a
    /// connection transition takes — a `Request`, an `Accepted`, a
    /// `Terminated` notice — reduces to this single effect, so the executor
    /// has one send arm (R4).
    Send {
        /// The peer to deliver to.
        to: PeerId,
        /// The message to send.
        message: Message,
    },
    /// The semantic misbehavior signal: an `Active` upstream forwarded a
    /// payload that failed signature verification after passing every earlier
    /// check (FR-017). The executor logs it (`connection_severed`, warn) and
    /// nothing else in this feature; a future blacklist consumes this variant
    /// without reshaping the transition's output.
    Misbehaved {
        /// The offending peer.
        peer: PeerId,
        /// The topic the severed connection was for.
        topic: TopicId,
        /// A static cause tag for the operator log.
        cause: &'static str,
    },
}

/// The single state-transition function. Synchronous; no protocol I/O.
///
/// Dispatches each event to its named handler and returns the effects the
/// shell must execute. Pre-connection every path returns an empty list.
// FR-008 purity (ambient tracing permitted per spec Assumptions / ADR 0011);
// one dispatch arm per Event variant — new variants add a handler, not edits
// to existing arms (FR-012).
pub(crate) fn apply(state: &mut NodeState, event: Event) -> Vec<Effect> {
    match event {
        Event::MessageReceived { from, message } => handle_message_received(state, from, message),
        Event::MembershipUpdate(update) => handle_membership_update(state, update),
        Event::TopicRegistryUpdate(update) => handle_topic_registry_update(state, update),
        Event::ConnectionSetup => handle_connection_setup(state),
        Event::Shutdown => handle_shutdown(state),
    }
}

/// Transition for the connection-establishment trigger.
///
/// Will consult the node's connection-selection strategy and dial the expected
/// upstreams it does not already hold (the FR-007 diff). Inert until User
/// Story 1 wires the strategy and connection structures (tasks T009–T011);
/// returns no effects for now.
fn handle_connection_setup(_state: &mut NodeState) -> Vec<Effect> {
    Vec::new()
}

/// Transition for the graceful-shutdown trigger.
///
/// Will clear both connection structures and emit one `Terminated` notice per
/// held entry (FR-020). Inert until User Story 4 (tasks T024–T025); returns no
/// effects for now.
fn handle_shutdown(_state: &mut NodeState) -> Vec<Effect> {
    Vec::new()
}

/// Transition for a topic-registry delta.
///
/// Folds only the `registered_topics` projection (topic → authorized
/// publishers): which topics are legitimate and who may publish to each. It does
/// not touch `subscriptions` or `candidates` — each registry's handler owns its
/// own field; the node's effective accept-filter is the intersection, read at
/// message-acceptance time. Pure; returns no effects.
// FR-011/FR-013; ADR 0016.
fn handle_topic_registry_update(state: &mut NodeState, event: TopicRegistryEvent) -> Vec<Effect> {
    match event {
        TopicRegistryEvent::Registered { topic, publishers } => {
            state.registered_topics.insert(topic, publishers);
        }
        TopicRegistryEvent::PublishersChanged {
            topic,
            added,
            removed,
        } => {
            let entry = state.registered_topics.entry(topic).or_default();
            for key in added {
                entry.insert(key);
            }
            for key in &removed {
                entry.remove(key);
            }
        }
        TopicRegistryEvent::Removed { topic } => {
            state.registered_topics.remove(&topic);
        }
    }
    Vec::new()
}

/// Transition for a subscription-registry membership delta.
///
/// The node derives **all** its registry state from this single stream: an
/// event about the node's **own** id updates its subscription set (what it
/// accepts on receive); an event about **any other** node updates the per-topic
/// candidate set. The node starts with empty subscriptions and folds the
/// `watch` stream (cold-start own entry + members, then live deltas) from
/// empty. Pure; returns no effects.
// FR-013/FR-015/FR-016/FR-018; ADR 0014.
fn handle_membership_update(state: &mut NodeState, event: MembershipEvent) -> Vec<Effect> {
    match event {
        MembershipEvent::Joined { node, topics } => {
            if node == state.self_id {
                // The node's own entry: this *is* its subscription set.
                state.subscriptions = topics.into_iter().collect();
            } else {
                for topic in topics {
                    state
                        .candidates
                        .entry(topic)
                        .or_default()
                        .insert(node.clone());
                }
            }
        }
        MembershipEvent::TopicsChanged {
            node,
            added,
            removed,
        } => {
            if node == state.self_id {
                for topic in added {
                    state.subscriptions.insert(topic);
                }
                for topic in &removed {
                    state.subscriptions.remove(topic);
                    // No longer interested in this topic — drop its candidates.
                    state.candidates.remove(topic);
                }
            } else {
                for topic in added {
                    state
                        .candidates
                        .entry(topic)
                        .or_default()
                        .insert(node.clone());
                }
                for topic in &removed {
                    if let Some(peers) = state.candidates.get_mut(topic) {
                        peers.remove(&node);
                    }
                }
            }
        }
        MembershipEvent::Left { node } => {
            if node == state.self_id {
                // The node's own registration was withdrawn.
                state.subscriptions.clear();
                state.candidates.clear();
            } else {
                for peers in state.candidates.values_mut() {
                    peers.remove(&node);
                }
            }
        }
    }
    Vec::new()
}

/// Transition for an inbound network message: dispatches per message kind.
fn handle_message_received(state: &mut NodeState, from: PeerId, message: Message) -> Vec<Effect> {
    tracing::debug!(
        target: "pubsub_node::node",
        from = %from,
        "recv",
    );

    match message {
        Message::Signed(signed) => handle_signed_message(state, from, signed),
        Message::Connection(connection) => handle_connection_message(state, from, connection),
    }
}

/// Transition for an inbound connection-control message.
///
/// Will verify the carried emitter's signature and dispatch on the action
/// kind to `handle_connection_request` / `_accepted` / `_terminated`. Inert
/// until User Story 1 (tasks T010–T011); returns no effects for now.
fn handle_connection_message(
    _state: &mut NodeState,
    _from: PeerId,
    _connection: ConnectionMessage,
) -> Vec<Effect> {
    Vec::new()
}

/// Transition for a signed dissemination message.
///
/// Records the delivery when its topic is subscribed **and** a registered
/// (legitimate) topic, and its signature verifies; otherwise the message is
/// dropped (with an info-level `message_dropped` event carrying the cause).
// FR-001/002/003 + FR-014; the cheap filters run first — subscribed?, then
// registered? — so off-topic / illegitimate-topic traffic never pays the
// signature-verification cost (ADR 0016).
fn handle_signed_message(
    state: &mut NodeState,
    from: PeerId,
    signed: SignedMessage,
) -> Vec<Effect> {
    if !state.subscriptions.contains(&signed.plain.topic) {
        tracing::info!(
            target: "pubsub_node::node",
            event = "message_dropped",
            cause = "topic_not_subscribed",
            self_id = %state.self_id,
            from = %from,
            topic = %signed.plain.topic,
        );
        return Vec::new();
    }

    // Topic-validity: the topic must be a registered (legitimate) topic. A
    // subscribed-but-unregistered topic is dropped — the effective accept-filter
    // is `subscriptions ∩ registered_topics` (ADR 0016, FR-014).
    if !state.registered_topics.contains_key(&signed.plain.topic) {
        tracing::info!(
            target: "pubsub_node::node",
            event = "message_dropped",
            cause = "topic_not_registered",
            self_id = %state.self_id,
            from = %from,
            topic = %signed.plain.topic,
        );
        return Vec::new();
    }

    // Authorized-publisher: on a non-open topic the message's publisher key must
    // be in the topic's authorized set; an open topic (empty set) accepts any
    // publisher. Checked before signature verification — a cheap set lookup, so
    // unauthorized-publisher traffic never pays the verification cost (FR-015,
    // ADR 0016). The `registered?` check above guarantees the entry exists.
    if let Some(authorized) = state.registered_topics.get(&signed.plain.topic) {
        if !authorized.is_empty() && !authorized.contains(signed.plain.publisher_id.as_public_key())
        {
            tracing::info!(
                target: "pubsub_node::node",
                event = "message_dropped",
                cause = "publisher_not_authorized",
                self_id = %state.self_id,
                from = %from,
                topic = %signed.plain.topic,
                publisher_id = %signed.plain.publisher_id,
            );
            return Vec::new();
        }
    }

    let verify_outcome = state.verifier.verify(
        signed.plain.publisher_id.as_public_key(),
        &signed.plain.signed_bytes(),
        &signed.signature,
    );
    if verify_outcome.is_err() {
        tracing::info!(
            target: "pubsub_node::node",
            event = "message_dropped",
            cause = "invalid_signature",
            self_id = %state.self_id,
            from = %from,
            topic = %signed.plain.topic,
            publisher_id = %signed.plain.publisher_id,
        );
        return Vec::new();
    }

    state.received.push(ReceivedDelivery {
        from,
        message: Message::Signed(signed),
    });
    Vec::new()
}

// Synchronous state-machine tests: construct a NodeState, apply scripted
// events, assert on state and returned effects after each step. No async
// runtime, no channels, no tasks; never asserts on log output (constitution:
// logs are operator UX). Covers FR-001/002/003/004/013, US2-AS1..3, and the
// empty-subscription edge case.
#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::crypto::mock::{MockCryptoScheme, TestSigner, TestVerifier};
    use crate::crypto::{Signer, Timestamp};
    use crate::message::{MessagePayload, PlainMessage, SignedMessage};
    use crate::subscription_registry::MembershipScript;
    use crate::topic_registry::TopicRegistryScript;

    fn topic(s: &str) -> TopicId {
        TopicId::from_str(s).expect("valid topic id")
    }

    fn peer(s: &str) -> PeerId {
        PeerId::from_str(s).expect("valid peer id")
    }

    fn pk(bytes: &[u8]) -> PublicKey {
        PublicKey::new(bytes.to_vec())
    }

    fn sorted(mut v: Vec<TopicId>) -> Vec<TopicId> {
        v.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        v
    }

    /// A `TopicRegistryUpdate` event registering `t` as an **open** topic.
    fn reg_open(t: &str) -> Event {
        Event::TopicRegistryUpdate(TopicRegistryEvent::Registered {
            topic: topic(t),
            publishers: BTreeSet::new(),
        })
    }

    /// A state subscribed to the given topics, with each topic also registered
    /// **open** in the topic registry (so it is a legitimate topic and the
    /// effective accept-filter — `subscriptions ∩ registered_topics` — equals
    /// the subscription set). These example tests exercise the subscription and
    /// signature filters; topic-validity and publisher-authorization have their
    /// own dedicated tests below.
    fn state_subscribed(topics: impl IntoIterator<Item = TopicId>) -> NodeState {
        let topics: Vec<TopicId> = topics.into_iter().collect();
        let mut state = NodeState::new(
            peer("self"),
            topics.iter().cloned().collect(),
            Arc::new(TestVerifier),
        );
        for t in topics {
            apply(
                &mut state,
                Event::TopicRegistryUpdate(TopicRegistryEvent::Registered {
                    topic: t,
                    publishers: BTreeSet::new(),
                }),
            );
        }
        state
    }

    /// A deterministic signer from an explicit scheme seed (distinct seeds yield
    /// distinct keys — used to model authorized vs unauthorized publishers).
    fn signer_seeded(seed: [u8; 32]) -> TestSigner {
        let mut scheme = MockCryptoScheme::with_seed(seed);
        let kp = scheme.generate_keypair();
        TestSigner::new(kp.private)
    }

    /// The standard deterministic signer (fixed scheme seed).
    fn signer() -> TestSigner {
        signer_seeded([7u8; 32])
    }

    /// Build a validly-signed message on `topic` carrying `Ping(n)`.
    fn signed_ping(signer: &TestSigner, topic: TopicId, n: u64) -> Message {
        let plain = PlainMessage {
            topic,
            publisher_id: signer.public_key().into(),
            parent_hash: None,
            sequence: 0,
            timestamp: Timestamp::from_millis(0),
            payload: MessagePayload::Ping(n),
        };
        let signature = signer.sign(&plain.signed_bytes());
        Message::Signed(SignedMessage { plain, signature })
    }

    /// Same as [`signed_ping`] but with the payload altered after signing,
    /// so the signature no longer verifies (the suite's mismatch pattern).
    fn tampered_ping(signer: &TestSigner, topic: TopicId, n: u64) -> Message {
        let Message::Signed(mut sm) = signed_ping(signer, topic, n) else {
            unreachable!("signed_ping always builds a Message::Signed");
        };
        sm.plain.payload = MessagePayload::Ping(n.wrapping_add(1));
        Message::Signed(sm)
    }

    // FR-001 / US2-AS1: subscribed topic + valid signature => recorded, in
    // order, with no effects and no I/O.
    #[test]
    fn valid_messages_recorded_in_processing_order() {
        let t1 = topic("t1");
        let mut state = state_subscribed(vec![t1.clone()]);
        let s = signer();
        let m1 = signed_ping(&s, t1.clone(), 1);
        let m2 = signed_ping(&s, t1.clone(), 2);

        let effects = apply(
            &mut state,
            Event::MessageReceived {
                from: peer("a"),
                message: m1.clone(),
            },
        );
        assert!(effects.is_empty(), "no effects pre-connection");
        let snap = state.received_snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].from, peer("a"));
        assert_eq!(snap[0].message, m1);

        let effects = apply(
            &mut state,
            Event::MessageReceived {
                from: peer("b"),
                message: m2.clone(),
            },
        );
        assert!(effects.is_empty());
        let snap = state.received_snapshot();
        assert_eq!(snap.len(), 2, "second delivery appended");
        assert_eq!(snap[1].from, peer("b"));
        assert_eq!(snap[1].message, m2);
    }

    // FR-002 / US2-AS2: off-topic message leaves state unchanged.
    #[test]
    fn off_topic_message_leaves_state_unchanged() {
        let t1 = topic("t1");
        let mut state = state_subscribed(vec![t1.clone()]);
        let s = signer();

        // One accepted delivery first, so "unchanged" is asserted against a
        // non-empty record.
        apply(
            &mut state,
            Event::MessageReceived {
                from: peer("a"),
                message: signed_ping(&s, t1, 1),
            },
        );
        let before = state.received_snapshot();

        let effects = apply(
            &mut state,
            Event::MessageReceived {
                from: peer("a"),
                message: signed_ping(&s, topic("t2"), 2),
            },
        );
        assert!(effects.is_empty());
        assert_eq!(state.received_snapshot(), before, "off-topic drop");
    }

    // FR-003: subscribed topic but invalid signature => dropped.
    #[test]
    fn invalid_signature_message_dropped() {
        let t1 = topic("t1");
        let mut state = state_subscribed(vec![t1.clone()]);
        let s = signer();

        let effects = apply(
            &mut state,
            Event::MessageReceived {
                from: peer("a"),
                message: tampered_ping(&s, t1, 1),
            },
        );
        assert!(effects.is_empty());
        assert!(
            state.received_snapshot().is_empty(),
            "tampered message never recorded"
        );
    }

    // Edge case: an empty subscription set drops every inbound message.
    #[test]
    fn empty_subscription_set_drops_everything() {
        let mut state = state_subscribed(vec![]);
        let s = signer();

        for n in 0..3 {
            let effects = apply(
                &mut state,
                Event::MessageReceived {
                    from: peer("a"),
                    message: signed_ping(&s, topic("t1"), n),
                },
            );
            assert!(effects.is_empty());
        }
        assert!(state.received_snapshot().is_empty());
    }

    // US2-AS3: same initial state + same event sequence => same final state.
    #[test]
    fn transition_is_deterministic() {
        let t1 = topic("t1");
        let s = signer();
        let script = || {
            vec![
                Event::MessageReceived {
                    from: peer("a"),
                    message: signed_ping(&s, t1.clone(), 1),
                },
                Event::MessageReceived {
                    from: peer("b"),
                    message: signed_ping(&s, topic("t2"), 2),
                },
                Event::MessageReceived {
                    from: peer("b"),
                    message: tampered_ping(&s, t1.clone(), 3),
                },
                Event::MessageReceived {
                    from: peer("c"),
                    message: signed_ping(&s, t1.clone(), 4),
                },
            ]
        };

        let mut first = state_subscribed(vec![t1.clone()]);
        for event in script() {
            assert!(apply(&mut first, event).is_empty());
        }
        let mut second = state_subscribed(vec![t1.clone()]);
        for event in script() {
            assert!(apply(&mut second, event).is_empty());
        }

        assert_eq!(first.received_snapshot(), second.received_snapshot());
        let sorted = |mut v: Vec<TopicId>| {
            v.sort_by(|a, b| a.as_str().cmp(b.as_str()));
            v
        };
        assert_eq!(
            sorted(first.subscriptions_snapshot()),
            sorted(second.subscriptions_snapshot())
        );
    }

    // A self membership update changes which subsequent messages are accepted —
    // the transition reads the current subscription state, not a snapshot. The
    // subscription set is derived from the node's own entry on the membership
    // stream; there is no local subscribe mutator (ADR 0013/0014/0015).
    #[test]
    fn subscription_change_affects_subsequent_transitions() {
        let t1 = topic("t1");
        let mut state = state_subscribed(vec![]); // self_id = "self", empty subscriptions
        let s = signer();

        apply(
            &mut state,
            Event::MessageReceived {
                from: peer("a"),
                message: signed_ping(&s, t1.clone(), 1),
            },
        );
        assert!(state.received_snapshot().is_empty(), "not subscribed yet");

        // The node's own entry arrives on the membership stream → subscribes t1,
        // and the topic registry registers t1 (legitimate) → t1 is now effective.
        apply(
            &mut state,
            Event::MembershipUpdate(MembershipEvent::joined("self", ["t1"])),
        );
        apply(
            &mut state,
            Event::TopicRegistryUpdate(TopicRegistryEvent::Registered {
                topic: t1.clone(),
                publishers: BTreeSet::new(),
            }),
        );
        apply(
            &mut state,
            Event::MessageReceived {
                from: peer("a"),
                message: signed_ping(&s, t1, 2),
            },
        );
        assert_eq!(state.received_snapshot().len(), 1, "subscribed now");
    }

    // US3 / FR-013/015/016: MembershipUpdate folds into per-topic candidate
    // sets; the node's own id is excluded; the transition returns no effects.
    #[test]
    fn membership_updates_fold_into_candidates_excluding_self() {
        let mut state = state_subscribed(vec![topic("t1"), topic("t2")]); // self_id = "self"
        let script = MembershipScript::new()
            .joined("a", ["t1"])
            .joined("b", ["t1", "t2"])
            .joined("self", ["t1"]) // own id — must be ignored
            .topics_changed("a", ["t2"], ["t1"])
            .left("b");
        for ev in script {
            assert!(apply(&mut state, Event::MembershipUpdate(ev)).is_empty());
        }
        // a moved t1->t2; b left; self never added.
        assert!(state.candidates_snapshot(&topic("t1")).is_empty());
        assert_eq!(state.candidates_snapshot(&topic("t2")), vec![peer("a")]);
    }

    // US2 / FR-014, SC-003: effective subscriptions = subscriptions ∩ registered.
    // A subscribed topic that is not a registered topic is excluded.
    #[test]
    fn subscriptions_are_subscribed_intersect_registered() {
        let mut state = NodeState::new(peer("self"), HashSet::new(), Arc::new(TestVerifier));
        // Topic registry registers only `weather`; membership declares both.
        apply(&mut state, reg_open("weather"));
        apply(
            &mut state,
            Event::MembershipUpdate(MembershipEvent::joined("self", ["weather", "ghosttopic"])),
        );
        assert_eq!(
            sorted(state.subscriptions_snapshot()),
            vec![topic("weather")],
            "ghosttopic is subscribed but not registered → excluded",
        );
    }

    // US2 / FR-014, SC-003/SC-004: a message on a subscribed-but-unregistered
    // topic is dropped (topic_not_registered); registering the topic later makes
    // it effective and the next message is accepted — no restart.
    #[test]
    fn unregistered_subscribed_topic_drops_then_accepts_after_registration() {
        let mut state = NodeState::new(peer("self"), HashSet::new(), Arc::new(TestVerifier));
        let s = signer();
        apply(
            &mut state,
            Event::MembershipUpdate(MembershipEvent::joined("self", ["ghosttopic"])),
        );
        // Subscribed but not registered → dropped.
        assert!(apply(
            &mut state,
            Event::MessageReceived {
                from: peer("a"),
                message: signed_ping(&s, topic("ghosttopic"), 1),
            },
        )
        .is_empty());
        assert!(
            state.received_snapshot().is_empty(),
            "unregistered topic → message dropped",
        );
        // Register it → now effective → accepted.
        apply(&mut state, reg_open("ghosttopic"));
        apply(
            &mut state,
            Event::MessageReceived {
                from: peer("a"),
                message: signed_ping(&s, topic("ghosttopic"), 2),
            },
        );
        assert_eq!(
            state.received_snapshot().len(),
            1,
            "registered now → accepted"
        );
    }

    // US2 / SC-004: removing a topic from the registry makes it ineffective.
    #[test]
    fn removing_a_topic_makes_it_ineffective() {
        let mut state = NodeState::new(peer("self"), HashSet::new(), Arc::new(TestVerifier));
        apply(
            &mut state,
            Event::MembershipUpdate(MembershipEvent::joined("self", ["weather"])),
        );
        assert!(
            state.subscriptions_snapshot().is_empty(),
            "not registered yet",
        );
        apply(&mut state, reg_open("weather"));
        assert_eq!(state.subscriptions_snapshot(), vec![topic("weather")],);
        apply(
            &mut state,
            Event::TopicRegistryUpdate(TopicRegistryEvent::Removed {
                topic: topic("weather"),
            }),
        );
        assert!(
            state.subscriptions_snapshot().is_empty(),
            "removed → no longer effective",
        );
    }

    // US2 / FR-013: handle_topic_registry_update folds the registered-topics
    // projection across a scripted register → publishers-changed → remove
    // sequence (declarative TopicRegistryScript); every apply returns no effects.
    #[test]
    fn topic_registry_script_folds_projection() {
        let mut state = state_subscribed(vec![topic("weather")]);
        // state_subscribed already registered weather open; drive a script that
        // re-registers it with a publisher, rotates publishers, and removes an
        // unrelated topic.
        let script = TopicRegistryScript::new()
            .registered("weather", [pk(b"k1")])
            .publishers_changed("weather", [pk(b"k4")], [pk(b"k1")])
            .removed("other");
        for ev in script {
            assert!(apply(&mut state, Event::TopicRegistryUpdate(ev)).is_empty());
        }
        // weather stays registered (so still effective); the no-op remove of an
        // unregistered "other" is harmless.
        assert_eq!(state.subscriptions_snapshot(), vec![topic("weather")],);
    }

    // US3 / FR-015, SC-005: a non-open topic accepts only authorized publishers;
    // an open topic accepts any. Authorization precedes signature verification —
    // an unauthorized publisher with a *valid* signature is still dropped.
    #[test]
    fn publisher_authorization_restricted_then_open() {
        let authorized = signer();
        let outsider = signer_seeded([9u8; 32]);
        let weather = topic("weather");
        let mut state = NodeState::new(
            peer("self"),
            HashSet::from([weather.clone()]),
            Arc::new(TestVerifier),
        );
        // weather restricted to the authorized signer's key.
        apply(
            &mut state,
            Event::TopicRegistryUpdate(TopicRegistryEvent::Registered {
                topic: weather.clone(),
                publishers: BTreeSet::from([authorized.public_key()]),
            }),
        );

        // Authorized publisher, valid signature → recorded.
        apply(
            &mut state,
            Event::MessageReceived {
                from: peer("relay"),
                message: signed_ping(&authorized, weather.clone(), 1),
            },
        );
        assert_eq!(
            state.received_snapshot().len(),
            1,
            "authorized publisher accepted",
        );

        // Unauthorized publisher with a VALID signature → dropped (authorization
        // precedes verification).
        apply(
            &mut state,
            Event::MessageReceived {
                from: peer("relay"),
                message: signed_ping(&outsider, weather.clone(), 2),
            },
        );
        assert_eq!(
            state.received_snapshot().len(),
            1,
            "unauthorized publisher dropped despite a valid signature",
        );

        // Re-register weather OPEN → the outsider is now accepted.
        apply(&mut state, reg_open("weather"));
        apply(
            &mut state,
            Event::MessageReceived {
                from: peer("relay"),
                message: signed_ping(&outsider, weather, 3),
            },
        );
        assert_eq!(
            state.received_snapshot().len(),
            2,
            "open topic accepts any publisher",
        );
    }

    // US3 / FR-015: authorization is ordered BEFORE verification — an authorized
    // publisher's *tampered* (invalid-signature) message passes the authorization
    // check but is dropped at verification.
    #[test]
    fn authorized_but_tampered_message_dropped_at_verification() {
        let authorized = signer();
        let weather = topic("weather");
        let mut state = NodeState::new(
            peer("self"),
            HashSet::from([weather.clone()]),
            Arc::new(TestVerifier),
        );
        apply(
            &mut state,
            Event::TopicRegistryUpdate(TopicRegistryEvent::Registered {
                topic: weather.clone(),
                publishers: BTreeSet::from([authorized.public_key()]),
            }),
        );
        apply(
            &mut state,
            Event::MessageReceived {
                from: peer("relay"),
                message: tampered_ping(&authorized, weather, 1),
            },
        );
        assert!(
            state.received_snapshot().is_empty(),
            "authorized publisher but invalid signature → dropped at verify",
        );
    }
}
