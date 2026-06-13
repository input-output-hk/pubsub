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

use crate::connection::{ConnectionStrategy, UpstreamState};
use crate::crypto::{PublicKey, Signer, Verifier};
use crate::event::Event;
use crate::message::{
    ConnectionAction, ConnectionMessage, Message, PlainConnection, SignedMessage,
};
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
    /// Upstream connections — those this node requested, serving as its message
    /// sources — keyed by `(peer, topic)`, each in an explicit
    /// [`UpstreamState`]. Written by the connection transitions (FR-001).
    upstream: HashMap<(PeerId, TopicId), UpstreamState>,
    /// Downstream connections — those this node accepted, serving as its
    /// fan-out destinations — as a set of `(peer, topic)` entries with no
    /// per-entry state (FR-002).
    downstream: HashSet<(PeerId, TopicId)>,
    /// The node's signing identity: signs the control messages it emits
    /// (`Request`/`Accepted`/`Terminated`). Rides along as an immutable service
    /// handle beside the verifier; the transition signs inside the pure core so
    /// each `Effect::Send` carries a complete signed message (FR-011).
    signer: Arc<dyn Signer>,
    /// The connection-selection policy consulted on a setup event, beside the
    /// verifier (the immutable service-handle slot). The transition reads it
    /// from the `ConnectionSetup` arm (ADR 0018).
    strategy: Arc<dyn ConnectionStrategy>,
}

impl NodeState {
    /// Construct the state value from already-parsed inputs.
    pub(crate) fn new(
        self_id: PeerId,
        subscriptions: HashSet<TopicId>,
        verifier: Arc<dyn Verifier>,
        signer: Arc<dyn Signer>,
        strategy: Arc<dyn ConnectionStrategy>,
    ) -> Self {
        Self {
            self_id,
            subscriptions,
            received: Vec::new(),
            verifier,
            candidates: HashMap::new(),
            registered_topics: HashMap::new(),
            upstream: HashMap::new(),
            downstream: HashSet::new(),
            signer,
            strategy,
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

    /// Snapshot of the upstream connections — `(peer, topic, state)` triples in
    /// unspecified order. A stable clone, unaffected by later events.
    // The public `Node::upstream_connections` getter that consumes this in the
    // (non-test) lib build lands in T014; the allow keeps commit 1 (state
    // machine + crate-internal tests) warning-clean and is removed there.
    #[allow(dead_code)]
    #[must_use]
    pub(crate) fn upstream_snapshot(&self) -> Vec<(PeerId, TopicId, UpstreamState)> {
        self.upstream
            .iter()
            .map(|((peer, topic), state)| (peer.clone(), topic.clone(), *state))
            .collect()
    }

    /// Snapshot of the downstream connections — `(peer, topic)` pairs in
    /// unspecified order. A stable clone, unaffected by later events.
    // See `upstream_snapshot`: the lib-build consumer (`Node::downstream_connections`)
    // lands in T014; allow removed there.
    #[allow(dead_code)]
    #[must_use]
    pub(crate) fn downstream_snapshot(&self) -> Vec<(PeerId, TopicId)> {
        self.downstream.iter().cloned().collect()
    }
}

/// Outbound commands the shell executes on the transition's behalf.
///
/// The transition itself performs no protocol I/O; it returns these and the
/// shell's effect executor (in `crate::node`) carries them out outside the
/// state lock. Crate-internal, like [`NodeState`].
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
    // Constructed by the misbehavior severance in User Story 3 (Phase 5); the
    // executor already handles it. The allow keeps the variant warning-clean
    // until that constructor lands.
    #[allow(dead_code)]
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
/// Consults the node's connection-selection strategy for the expected upstream
/// set and applies it as the FR-007 diff: dial everything expected that is not
/// already `Active`. A pair not held gains an `AwaitingAccept` entry and a
/// `Request`; a pair still at `AwaitingAccept` keeps its entry and is
/// re-requested (its earlier request may have been lost); an `Active` pair is
/// left alone. Expected-set membership never removes anything. The strategy
/// reads the **membership-derived** `subscriptions` field (not the
/// registration-gated effective filter) — the dial side mirrors the acceptance
/// rule (FR-008/009; data-model §1.4).
fn handle_connection_setup(state: &mut NodeState) -> Vec<Effect> {
    let expected = state
        .strategy
        .expected_upstream(&state.subscriptions, &state.candidates);
    // Clone the immutable bits the request builder needs so the loop can mutate
    // `state.upstream` without aliasing the whole struct.
    let self_id = state.self_id.clone();
    let signer = Arc::clone(&state.signer);

    let mut effects = Vec::new();
    for (peer, topic) in expected {
        match state.upstream.get(&(peer.clone(), topic.clone())).copied() {
            Some(UpstreamState::Active) => continue,
            Some(UpstreamState::AwaitingAccept) => {}
            None => {
                state
                    .upstream
                    .insert((peer.clone(), topic.clone()), UpstreamState::AwaitingAccept);
            }
        }
        let message = signed_connection(
            &self_id,
            signer.as_ref(),
            ConnectionAction::Request { topic },
        );
        effects.push(Effect::Send { to: peer, message });
    }
    effects
}

/// Build a control message signed by the node's own signer, with the node's
/// own id as the carried emitter (FR-011 — the signature binds emitter, kind,
/// and topic).
fn signed_connection(self_id: &PeerId, signer: &dyn Signer, action: ConnectionAction) -> Message {
    let plain = PlainConnection {
        emitter: self_id.clone(),
        action,
    };
    let signature = signer.sign(&plain.signed_bytes());
    Message::Connection(ConnectionMessage { plain, signature })
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
/// Runs the control-message checks (data-model §4) on the **carried emitter**
/// — the transport frame's sender is not consulted (FR-011/015): the carried
/// emitter must not be the node itself, and the signature must verify over
/// `plain.signed_bytes()` under the emitter's key. A passing message dispatches
/// on its action kind. Drops are cause-tagged `message_dropped` events.
fn handle_connection_message(
    state: &mut NodeState,
    _from: PeerId,
    connection: ConnectionMessage,
) -> Vec<Effect> {
    let ConnectionMessage { plain, signature } = connection;

    // FR-015: a control message whose carried emitter is the node itself is
    // dropped (checked before signature verification — self-connections are
    // unrepresentable end to end).
    if plain.emitter == state.self_id {
        tracing::info!(
            target: "pubsub_node::node",
            event = "message_dropped",
            cause = "self_emitter",
            self_id = %state.self_id,
        );
        return Vec::new();
    }

    // FR-011/015: verify the signature against the carried emitter's key.
    if state
        .verifier
        .verify(
            plain.emitter.as_public_key(),
            &plain.signed_bytes(),
            &signature,
        )
        .is_err()
    {
        tracing::info!(
            target: "pubsub_node::node",
            event = "message_dropped",
            cause = "invalid_signature",
            self_id = %state.self_id,
            emitter = %plain.emitter,
        );
        return Vec::new();
    }

    match plain.action {
        ConnectionAction::Request { topic } => {
            handle_connection_request(state, plain.emitter, topic)
        }
        ConnectionAction::Accepted { topic } => {
            handle_connection_accepted(state, &plain.emitter, &topic)
        }
        ConnectionAction::Terminated { topic } => {
            handle_connection_terminated(state, &plain.emitter, &topic)
        }
    }
}

/// Transition for a verified `Request` from `emitter` on `topic` (FR-012).
///
/// Membership-validates against the **membership-derived** subscription set
/// (registration gates delivery, not acceptance — the S7 pin): the topic must
/// be among the node's own topics AND the emitter a known member of it. A valid
/// request records the downstream entry (idempotently) and replies `Accepted`
/// to the carried emitter; a failing one is dropped with no state change and no
/// reply.
fn handle_connection_request(
    state: &mut NodeState,
    emitter: PeerId,
    topic: TopicId,
) -> Vec<Effect> {
    let topic_is_own = state.subscriptions.contains(&topic);
    let emitter_is_member = state
        .candidates
        .get(&topic)
        .is_some_and(|peers| peers.contains(&emitter));
    if !(topic_is_own && emitter_is_member) {
        tracing::info!(
            target: "pubsub_node::node",
            event = "message_dropped",
            cause = "membership_validation_failed",
            self_id = %state.self_id,
            emitter = %emitter,
            topic = %topic,
        );
        return Vec::new();
    }

    // Idempotent: the set absorbs a duplicate; a re-dial re-sends Accepted.
    state.downstream.insert((emitter.clone(), topic.clone()));
    let message = signed_connection(
        &state.self_id,
        state.signer.as_ref(),
        ConnectionAction::Accepted { topic },
    );
    vec![Effect::Send {
        to: emitter,
        message,
    }]
}

/// Transition for a verified `Accepted` from `emitter` on `topic` (FR-013).
///
/// Activates the matching `AwaitingAccept` upstream entry. An `Accepted` with
/// no matching pending entry (absent, or already `Active`) is dropped and
/// creates/modifies nothing.
fn handle_connection_accepted(
    state: &mut NodeState,
    emitter: &PeerId,
    topic: &TopicId,
) -> Vec<Effect> {
    if let Some(entry) = state.upstream.get_mut(&(emitter.clone(), topic.clone())) {
        if *entry == UpstreamState::AwaitingAccept {
            *entry = UpstreamState::Active;
            return Vec::new();
        }
    }
    tracing::info!(
        target: "pubsub_node::node",
        event = "message_dropped",
        cause = "unsolicited_accept",
        self_id = %state.self_id,
        emitter = %emitter,
        topic = %topic,
    );
    Vec::new()
}

/// Transition for a verified `Terminated` from `emitter` on `topic` (FR-014).
///
/// Removes the matching entry in either role (both, if both are held). A
/// `Terminated` for a connection not held is dropped; a `Terminated` is never
/// replied to.
fn handle_connection_terminated(
    state: &mut NodeState,
    emitter: &PeerId,
    topic: &TopicId,
) -> Vec<Effect> {
    let key = (emitter.clone(), topic.clone());
    let removed_upstream = state.upstream.remove(&key).is_some();
    let removed_downstream = state.downstream.remove(&key);
    if !(removed_upstream || removed_downstream) {
        tracing::info!(
            target: "pubsub_node::node",
            event = "message_dropped",
            cause = "unknown_termination",
            self_id = %state.self_id,
            emitter = %emitter,
            topic = %topic,
        );
    }
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
    use crate::connection::test_support::{
        accepted_from, membership_joined, misattributed_request, request_from, terminated_from,
        ConnectionScript,
    };
    use crate::connection::ConnectToAllCandidates;
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

    /// The v1 selection policy, as the transition-visible service handle.
    fn strategy() -> Arc<dyn ConnectionStrategy> {
        Arc::new(ConnectToAllCandidates)
    }

    /// A signer for the alias's keypair — agrees with `PeerId::from_str(alias)`
    /// by construction, so it is the node's own coherent signing identity.
    fn alias_signer(alias: &str) -> Arc<dyn Signer> {
        let scheme = MockCryptoScheme::with_seed([0u8; 32]);
        Arc::new(scheme.signer(scheme.keypair_from_alias(alias).private))
    }

    /// Construct a `NodeState` for `self_id`, seeding the verifier, the node's
    /// own coherent signer, and the v1 strategy — the common test setup.
    fn node_state(self_id: &str, subscriptions: HashSet<TopicId>) -> NodeState {
        NodeState::new(
            peer(self_id),
            subscriptions,
            Arc::new(TestVerifier),
            alias_signer(self_id),
            strategy(),
        )
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
        let mut state = node_state("self", topics.iter().cloned().collect());
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
        let mut state = node_state("self", HashSet::new());
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
        let mut state = node_state("self", HashSet::new());
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
        let mut state = node_state("self", HashSet::new());
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
        let mut state = node_state("self", HashSet::from([weather.clone()]));
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
        let mut state = node_state("self", HashSet::from([weather.clone()]));
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

    // ---- Connection lifecycle (US1): helpers ----------------------------------

    /// The upstream state recorded for `(p, t)`, if any.
    fn upstream_state(state: &NodeState, p: &str, t: &str) -> Option<UpstreamState> {
        state
            .upstream_snapshot()
            .into_iter()
            .find(|(pp, tt, _)| pp == &peer(p) && tt == &topic(t))
            .map(|(_, _, st)| st)
    }

    /// Whether a downstream entry is held for `(p, t)`.
    fn has_downstream(state: &NodeState, p: &str, t: &str) -> bool {
        state.downstream_snapshot().contains(&(peer(p), topic(t)))
    }

    /// The `(to, topic)` of every `Request` send effect (asserting emitter == self).
    fn request_sends(effects: &[Effect], expected_emitter: &str) -> Vec<(PeerId, TopicId)> {
        let mut out = Vec::new();
        for effect in effects {
            if let Effect::Send {
                to,
                message: Message::Connection(cm),
            } = effect
            {
                if let ConnectionAction::Request { topic } = &cm.plain.action {
                    assert_eq!(cm.plain.emitter, peer(expected_emitter), "request emitter");
                    out.push((to.clone(), topic.clone()));
                }
            }
        }
        out
    }

    /// The `(to, topic)` of every `Accepted` send effect (asserting emitter == self).
    fn accepted_sends(effects: &[Effect], expected_emitter: &str) -> Vec<(PeerId, TopicId)> {
        let mut out = Vec::new();
        for effect in effects {
            if let Effect::Send {
                to,
                message: Message::Connection(cm),
            } = effect
            {
                if let ConnectionAction::Accepted { topic } = &cm.plain.action {
                    assert_eq!(cm.plain.emitter, peer(expected_emitter), "accepted emitter");
                    out.push((to.clone(), topic.clone()));
                }
            }
        }
        out
    }

    fn sorted_pairs(mut v: Vec<(PeerId, TopicId)>) -> Vec<(PeerId, TopicId)> {
        v.sort_by(|a, b| (a.0.to_string(), a.1.as_str()).cmp(&(b.0.to_string(), b.1.as_str())));
        v
    }

    // ---- T009: dialer side (FR-006..009, US1-AS1..4) --------------------------

    // US1-AS1/AS2: a setup event dials every candidate across the node's topics —
    // one AwaitingAccept entry and one Request (emitter self) per (peer, topic).
    #[test]
    fn setup_event_dials_all_candidates() {
        let mut state = node_state("self", HashSet::from([topic("t1")]));
        apply(&mut state, membership_joined("a", ["t1"]));
        apply(&mut state, membership_joined("b", ["t1"]));

        let effects = apply(&mut state, Event::ConnectionSetup);

        assert_eq!(
            upstream_state(&state, "a", "t1"),
            Some(UpstreamState::AwaitingAccept),
        );
        assert_eq!(
            upstream_state(&state, "b", "t1"),
            Some(UpstreamState::AwaitingAccept),
        );
        assert_eq!(
            sorted_pairs(request_sends(&effects, "self")),
            sorted_pairs(vec![(peer("a"), topic("t1")), (peer("b"), topic("t1"))]),
        );
        assert!(
            state.downstream_snapshot().is_empty(),
            "dialing adds no downstream"
        );
    }

    // US1-AS2: connections are keyed per (peer, topic) — a peer sharing two topics
    // yields two independent upstream connections.
    #[test]
    fn setup_keys_connections_per_peer_topic() {
        let mut state = node_state("self", HashSet::from([topic("t1"), topic("t2")]));
        apply(&mut state, membership_joined("a", ["t1", "t2"]));

        let effects = apply(&mut state, Event::ConnectionSetup);

        assert_eq!(
            upstream_state(&state, "a", "t1"),
            Some(UpstreamState::AwaitingAccept),
        );
        assert_eq!(
            upstream_state(&state, "a", "t2"),
            Some(UpstreamState::AwaitingAccept),
        );
        assert_eq!(
            request_sends(&effects, "self").len(),
            2,
            "one request per pair"
        );
    }

    // US1-AS4: an empty candidate view yields no requests and no entries.
    #[test]
    fn setup_with_empty_view_is_a_noop() {
        let mut state = node_state("self", HashSet::from([topic("t1")]));
        let effects = apply(&mut state, Event::ConnectionSetup);
        assert!(effects.is_empty(), "no candidates → no requests");
        assert!(state.upstream_snapshot().is_empty());
    }

    // SC-007: the node never dials itself — a self membership event sets its own
    // subscriptions (not a candidate), so self is never in the expected set.
    #[test]
    fn self_is_never_dialed() {
        let mut state = node_state("self", HashSet::new());
        apply(&mut state, membership_joined("self", ["t1"])); // own entry → subscriptions
        apply(&mut state, membership_joined("a", ["t1"])); // real candidate

        let effects = apply(&mut state, Event::ConnectionSetup);

        assert_eq!(
            upstream_state(&state, "self", "t1"),
            None,
            "self never dialed"
        );
        assert_eq!(
            request_sends(&effects, "self"),
            vec![(peer("a"), topic("t1"))],
            "only the real candidate is dialed",
        );
    }

    // Repeated-setup EC + FR-007: a recurring setup re-dials pending pairs (entry
    // kept, fresh Request), skips Active pairs, dials newly-known candidates, and
    // never removes an entry.
    #[test]
    fn repeated_setup_redials_pending_skips_active_never_removes() {
        let mut state = node_state("self", HashSet::from([topic("t1")]));
        apply(&mut state, membership_joined("a", ["t1"]));

        // First setup → a pending.
        apply(&mut state, Event::ConnectionSetup);
        assert_eq!(
            upstream_state(&state, "a", "t1"),
            Some(UpstreamState::AwaitingAccept),
        );

        // Repeat with a still pending → re-dialed (fresh Request), entry kept.
        let effects = apply(&mut state, Event::ConnectionSetup);
        assert_eq!(
            request_sends(&effects, "self"),
            vec![(peer("a"), topic("t1"))],
            "pending pair re-dialed",
        );
        assert_eq!(
            upstream_state(&state, "a", "t1"),
            Some(UpstreamState::AwaitingAccept),
        );

        // a accepts → Active. Add candidate b.
        apply(&mut state, accepted_from("a", "t1"));
        assert_eq!(
            upstream_state(&state, "a", "t1"),
            Some(UpstreamState::Active)
        );
        apply(&mut state, membership_joined("b", ["t1"]));

        // Repeat → b dialed, a (Active) left alone and still present.
        let effects = apply(&mut state, Event::ConnectionSetup);
        assert_eq!(
            request_sends(&effects, "self"),
            vec![(peer("b"), topic("t1"))],
            "Active pair not re-dialed; new candidate dialed",
        );
        assert_eq!(
            upstream_state(&state, "a", "t1"),
            Some(UpstreamState::Active)
        );
        assert_eq!(
            upstream_state(&state, "b", "t1"),
            Some(UpstreamState::AwaitingAccept),
        );
    }

    // US1-AS3 / FR-008: a membership update after setup folds into candidates but
    // creates no connection entry and returns no effects; a later setup dials it.
    #[test]
    fn membership_update_after_setup_folds_only_then_later_setup_dials() {
        let mut state = node_state("self", HashSet::from([topic("t1")]));
        apply(&mut state, membership_joined("a", ["t1"]));
        apply(&mut state, Event::ConnectionSetup);

        // New member arrives by membership update — no establishment on its own.
        let effects = apply(&mut state, membership_joined("b", ["t1"]));
        assert!(
            effects.is_empty(),
            "membership update alone returns no effects"
        );
        assert_eq!(
            upstream_state(&state, "b", "t1"),
            None,
            "no entry from membership"
        );

        // A subsequent setup event dials the new member.
        let effects = apply(&mut state, Event::ConnectionSetup);
        assert!(
            request_sends(&effects, "self").contains(&(peer("b"), topic("t1"))),
            "later setup dials the newly-known member",
        );
    }

    // ---- T010: acceptor + activation side (FR-011..015, US1-AS5..7) -----------

    // US1-AS5 / FR-012: a membership-valid Request is accepted — downstream entry
    // recorded and Accepted sent to the carried emitter.
    #[test]
    fn membership_valid_request_is_accepted() {
        let mut state = node_state("self", HashSet::from([topic("t1")]));
        apply(&mut state, membership_joined("a", ["t1"]));

        let effects = apply(&mut state, request_from("a", "t1"));

        assert!(has_downstream(&state, "a", "t1"), "downstream recorded");
        assert_eq!(
            accepted_sends(&effects, "self"),
            vec![(peer("a"), topic("t1"))],
            "Accepted sent to the carried emitter",
        );
    }

    // US1-AS7 / FR-012: a Request fails validation when the topic is not among the
    // node's own topics, or the requester is not a known member — silent drop,
    // no downstream, no reply.
    #[test]
    fn request_dropped_when_membership_validation_fails() {
        // (a) topic not among own topics.
        let mut state = node_state("self", HashSet::new());
        apply(&mut state, membership_joined("a", ["t1"]));
        let effects = apply(&mut state, request_from("a", "t1"));
        assert!(!has_downstream(&state, "a", "t1"));
        assert!(effects.is_empty(), "no reply when topic not own");

        // (b) requester not a known member.
        let mut state = node_state("self", HashSet::from([topic("t1")]));
        let effects = apply(&mut state, request_from("a", "t1"));
        assert!(!has_downstream(&state, "a", "t1"));
        assert!(effects.is_empty(), "no reply when requester not a member");
    }

    // S7 PIN (mandatory): acceptance validates the membership-derived subscription
    // set only — a Request for a topic the node is a member of but that is absent
    // from the topic registry is accepted. Registration gates delivery, not
    // acceptance (revisit-flagged).
    #[test]
    fn request_accepted_for_membership_valid_but_unregistered_topic() {
        let mut state = node_state("self", HashSet::from([topic("t1")]));
        apply(&mut state, membership_joined("a", ["t1"]));
        // Deliberately NO TopicRegistryUpdate for t1 — it is not a registered topic.
        assert!(
            state.subscriptions_snapshot().is_empty(),
            "t1 is membership-declared but not registered → not in the effective filter",
        );

        let effects = apply(&mut state, request_from("a", "t1"));

        assert!(
            has_downstream(&state, "a", "t1"),
            "acceptance succeeds on the membership-derived set despite no registration",
        );
        assert_eq!(
            accepted_sends(&effects, "self"),
            vec![(peer("a"), topic("t1"))]
        );
    }

    // FR-012 / US4-AS4: a duplicate Request from a still-valid member is an
    // idempotent re-accept (entry kept, Accepted re-sent); a re-dial that no
    // longer passes validation is dropped and the entry is left as-is.
    #[test]
    fn duplicate_request_idempotent_then_stale_on_failed_revalidation() {
        let mut state = node_state("self", HashSet::from([topic("t1")]));
        apply(&mut state, membership_joined("a", ["t1"]));
        apply(&mut state, request_from("a", "t1"));
        assert!(has_downstream(&state, "a", "t1"));

        // Duplicate while still a member → re-accepted, single entry.
        let effects = apply(&mut state, request_from("a", "t1"));
        assert_eq!(
            accepted_sends(&effects, "self"),
            vec![(peer("a"), topic("t1"))]
        );
        assert_eq!(state.downstream_snapshot().len(), 1, "still one entry");

        // a leaves the topic, then re-dials → validation fails, entry left as-is.
        apply(
            &mut state,
            Event::MembershipUpdate(MembershipEvent::left("a")),
        );
        let effects = apply(&mut state, request_from("a", "t1"));
        assert!(effects.is_empty(), "failed re-validation → no reply");
        assert!(
            has_downstream(&state, "a", "t1"),
            "existing entry left as-is"
        );
    }

    // FR-015 self-emitter EC: a control message whose carried emitter is the node
    // itself is dropped, no state change (even with a valid signature).
    #[test]
    fn self_emitter_control_message_dropped() {
        let mut state = node_state("self", HashSet::from([topic("t1")]));
        apply(&mut state, membership_joined("self", ["t1"]));
        let effects = apply(&mut state, request_from("self", "t1"));
        assert!(effects.is_empty());
        assert!(state.downstream_snapshot().is_empty(), "no self-connection");
    }

    // FR-015 invalid-signature EC: a control message failing verification is
    // dropped, no state change (here: emitter a but signed by b).
    #[test]
    fn control_invalid_signature_dropped() {
        let mut state = node_state("self", HashSet::from([topic("t1")]));
        apply(&mut state, membership_joined("a", ["t1"]));
        let effects = apply(&mut state, misattributed_request("a", "b", "t1"));
        assert!(effects.is_empty());
        assert!(
            !has_downstream(&state, "a", "t1"),
            "a request with a bad signature is dropped before acceptance",
        );
    }

    // US1-AS6 / FR-013: an Accepted matching an AwaitingAccept entry activates it.
    #[test]
    fn accepted_activates_awaiting_entry() {
        let mut state = node_state("self", HashSet::from([topic("t1")]));
        apply(&mut state, membership_joined("a", ["t1"]));
        apply(&mut state, Event::ConnectionSetup);
        assert_eq!(
            upstream_state(&state, "a", "t1"),
            Some(UpstreamState::AwaitingAccept),
        );

        let effects = apply(&mut state, accepted_from("a", "t1"));
        assert!(effects.is_empty(), "activation sends nothing");
        assert_eq!(
            upstream_state(&state, "a", "t1"),
            Some(UpstreamState::Active)
        );
    }

    // FR-013: an Accepted with no matching pending entry is dropped, no entry
    // created or modified (also covers an Accepted for an already-Active pair).
    #[test]
    fn unsolicited_accepted_dropped() {
        let mut state = node_state("self", HashSet::from([topic("t1")]));
        let effects = apply(&mut state, accepted_from("a", "t1"));
        assert!(effects.is_empty());
        assert_eq!(upstream_state(&state, "a", "t1"), None, "no entry created");
    }

    // FR-014: a Terminated for a held entry removes it (either role); a Terminated
    // for a connection not held is dropped, no state change. Never replied to.
    #[test]
    fn terminated_removes_held_entry_else_dropped() {
        let mut state = node_state("self", HashSet::from([topic("t1")]));
        apply(&mut state, membership_joined("a", ["t1"]));
        // Establish both roles with a: upstream via setup+accept, downstream via request.
        apply(&mut state, Event::ConnectionSetup);
        apply(&mut state, accepted_from("a", "t1"));
        apply(&mut state, request_from("a", "t1"));
        assert_eq!(
            upstream_state(&state, "a", "t1"),
            Some(UpstreamState::Active)
        );
        assert!(has_downstream(&state, "a", "t1"));

        // Terminated removes the matching entry in both roles, sends nothing.
        let effects = apply(&mut state, terminated_from("a", "t1"));
        assert!(effects.is_empty(), "Terminated is never replied to");
        assert_eq!(upstream_state(&state, "a", "t1"), None);
        assert!(!has_downstream(&state, "a", "t1"));

        // A second (now-unknown) Terminated is a plain drop.
        let effects = apply(&mut state, terminated_from("a", "t1"));
        assert!(effects.is_empty());
    }

    // SC-006: the full establishment lifecycle is reachable by feeding events
    // alone via a declarative ConnectionScript (no timers).
    #[test]
    fn scripted_establishment_reaches_active() {
        let mut state = node_state("self", HashSet::from([topic("t")]));
        let script = ConnectionScript::new()
            .member_joined("b", ["t"])
            .setup()
            .accepted_from("b", "t");
        for event in script {
            apply(&mut state, event);
        }
        assert_eq!(
            upstream_state(&state, "b", "t"),
            Some(UpstreamState::Active)
        );
    }
}
