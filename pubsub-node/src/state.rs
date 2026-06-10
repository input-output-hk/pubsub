//! The node's crate-internal pure core: the explicit state value and the
//! synchronous transition function the event loop drives.
//!
//! [`NodeState`] is a plain struct — no channels, no tasks, no interior
//! locking — so it is constructible and drivable in a synchronous unit test.
//! All event-driven mutation goes through [`apply`], which performs no
//! protocol I/O and returns the outbound commands ([`Effect`]) the shell must
//! execute. Control-plane subscription changes go through
//! [`NodeState::subscribe`] / [`NodeState::unsubscribe`]. Operator log events
//! are emitted inline at the decision sites; they are ambient observability,
//! not part of the transition's contract.
//!
//! The shell side (queue, event loop, producers) lives in `crate::node`.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::crypto::Verifier;
use crate::event::Event;
use crate::message::{Message, SignedMessage};
use crate::node::{SubscribeOutcome, UnsubscribeOutcome};
use crate::peer::PeerId;
use crate::received::ReceivedDelivery;
use crate::subscription_registry::MembershipEvent;
use crate::topic::TopicId;

/// The node's full mutable state as one explicit value.
///
/// Mutated only under the shell's lock: event-driven transitions via
/// [`apply`] (sole caller: the event loop), control-plane subscription
/// changes via [`subscribe`](Self::subscribe) /
/// [`unsubscribe`](Self::unsubscribe). The verifier rides along as the
/// immutable service handle the message-received transition consults.
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
        }
    }

    /// Snapshot of every recorded delivery, in processing order.
    #[must_use]
    pub(crate) fn received_snapshot(&self) -> Vec<ReceivedDelivery> {
        self.received.clone()
    }

    /// Snapshot of the current subscription set (unspecified order).
    #[must_use]
    pub(crate) fn subscriptions_snapshot(&self) -> Vec<TopicId> {
        self.subscriptions.iter().cloned().collect()
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

    /// Add `topic` to the subscription set.
    ///
    /// Returns [`SubscribeOutcome::Added`] when newly inserted, or
    /// [`SubscribeOutcome::AlreadyPresent`] for an idempotent no-op.
    // FR-004: outcome semantics unchanged from ADR 0008; logic lives here so
    // it is synchronously testable (ADR 0012).
    // Owned `TopicId` matches `HashSet::insert`'s consuming shape and the
    // public-API contract (ADR 0008); the lint-flagged "needless pass by
    // value" is the contract choice, not an accident (as on the 002 original).
    #[allow(clippy::needless_pass_by_value)]
    pub(crate) fn subscribe(&mut self, topic: TopicId) -> SubscribeOutcome {
        let was_inserted = self.subscriptions.insert(topic.clone());

        if was_inserted {
            tracing::info!(
                target: "pubsub_node::node",
                event = "topic_subscribed",
                self_id = %self.self_id,
                topic = %topic,
            );
            SubscribeOutcome::Added
        } else {
            tracing::debug!(
                target: "pubsub_node::node",
                event = "topic_subscribe_noop",
                self_id = %self.self_id,
                topic = %topic,
                reason = "already_present",
            );
            SubscribeOutcome::AlreadyPresent
        }
    }

    /// Remove `topic` from the subscription set.
    ///
    /// Returns [`UnsubscribeOutcome::Removed`] when present and removed, or
    /// [`UnsubscribeOutcome::NotSubscribed`] for an idempotent no-op.
    // Owned `TopicId` for API symmetry with `subscribe`; see the analogous
    // allow there.
    #[allow(clippy::needless_pass_by_value)]
    pub(crate) fn unsubscribe(&mut self, topic: TopicId) -> UnsubscribeOutcome {
        let was_removed = self.subscriptions.remove(&topic);

        if was_removed {
            tracing::info!(
                target: "pubsub_node::node",
                event = "topic_unsubscribed",
                self_id = %self.self_id,
                topic = %topic,
            );
            UnsubscribeOutcome::Removed
        } else {
            tracing::debug!(
                target: "pubsub_node::node",
                event = "topic_unsubscribe_noop",
                self_id = %self.self_id,
                topic = %topic,
                reason = "not_subscribed",
            );
            UnsubscribeOutcome::NotSubscribed
        }
    }
}

/// Outbound commands the shell executes on the transition's behalf.
///
/// Uninhabited at this stage: the node only ingests. The first variants
/// (message forwarding, dialing, closing) arrive with the connection model;
/// the type exists now so the transition's signature is stable for the
/// features that extend it.
// FR-013: ships present-but-empty; locked signature justified by the ROADMAP
// consumers (004-connections effects; 008's RegistryUpdate arm) — ADR 0011.
#[non_exhaustive]
pub(crate) enum Effect {}

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
    }
}

/// Transition for a subscription-registry membership delta: folds the change
/// into the per-topic candidate set, excluding the node's own id (the registry
/// stream may include it; self-exclusion is applied here, locally). Pure;
/// returns no effects (the candidate set is read by a future sampler/dialer).
// FR-013/FR-015/FR-016; ADR 0014.
fn handle_membership_update(state: &mut NodeState, event: MembershipEvent) -> Vec<Effect> {
    match event {
        MembershipEvent::Joined { node, topics } => {
            if node != state.self_id {
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
            if node != state.self_id {
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
            for peers in state.candidates.values_mut() {
                peers.remove(&node);
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
    }
}

/// Transition for a signed dissemination message.
///
/// Records the delivery when its topic is subscribed and its signature
/// verifies; otherwise the message is dropped (with an info-level
/// `message_dropped` event carrying the cause).
// FR-001/002/003; ported verbatim from the 003 consumer loop — topic filter
// first (cheap), then signature verification, so off-topic traffic never pays
// the verification cost.
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

    fn topic(s: &str) -> TopicId {
        TopicId::from_str(s).expect("valid topic id")
    }

    fn peer(s: &str) -> PeerId {
        PeerId::from_str(s).expect("valid peer id")
    }

    /// A state subscribed to the given topics, with the standard mock verifier.
    fn state_subscribed(topics: impl IntoIterator<Item = TopicId>) -> NodeState {
        NodeState::new(
            peer("self"),
            topics.into_iter().collect(),
            Arc::new(TestVerifier),
        )
    }

    /// A deterministic signer (fixed scheme seed).
    fn signer() -> TestSigner {
        let mut scheme = MockCryptoScheme::with_seed([7u8; 32]);
        let kp = scheme.generate_keypair();
        TestSigner::new(kp.private)
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
        let Message::Signed(mut sm) = signed_ping(signer, topic, n);
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

    // FR-004: subscribe/unsubscribe outcome pairs, idempotent no-ops included.
    #[test]
    fn subscription_mutator_outcomes() {
        let t1 = topic("t1");
        let mut state = state_subscribed(vec![]);

        assert_eq!(state.subscribe(t1.clone()), SubscribeOutcome::Added);
        assert_eq!(
            state.subscribe(t1.clone()),
            SubscribeOutcome::AlreadyPresent
        );
        assert_eq!(state.subscriptions_snapshot(), vec![t1.clone()]);

        assert_eq!(state.unsubscribe(t1.clone()), UnsubscribeOutcome::Removed);
        assert_eq!(state.unsubscribe(t1), UnsubscribeOutcome::NotSubscribed);
        assert!(state.subscriptions_snapshot().is_empty());
    }

    // Subscribing mid-script changes which subsequent messages are accepted —
    // the transition reads the current subscription state, not a snapshot.
    #[test]
    fn subscription_change_affects_subsequent_transitions() {
        let t1 = topic("t1");
        let mut state = state_subscribed(vec![]);
        let s = signer();

        apply(
            &mut state,
            Event::MessageReceived {
                from: peer("a"),
                message: signed_ping(&s, t1.clone(), 1),
            },
        );
        assert!(state.received_snapshot().is_empty(), "not subscribed yet");

        state.subscribe(t1.clone());
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
        let script = [
            MembershipEvent::Joined {
                node: peer("a"),
                topics: [topic("t1")].into_iter().collect(),
            },
            MembershipEvent::Joined {
                node: peer("b"),
                topics: [topic("t1"), topic("t2")].into_iter().collect(),
            },
            MembershipEvent::Joined {
                node: peer("self"), // own id — must be ignored
                topics: [topic("t1")].into_iter().collect(),
            },
            MembershipEvent::TopicsChanged {
                node: peer("a"),
                added: [topic("t2")].into_iter().collect(),
                removed: [topic("t1")].into_iter().collect(),
            },
            MembershipEvent::Left { node: peer("b") },
        ];
        for ev in script {
            assert!(apply(&mut state, Event::MembershipUpdate(ev)).is_empty());
        }
        // a moved t1->t2; b left; self never added.
        assert!(state.candidates_snapshot(&topic("t1")).is_empty());
        assert_eq!(state.candidates_snapshot(&topic("t2")), vec![peer("a")]);
    }
}
