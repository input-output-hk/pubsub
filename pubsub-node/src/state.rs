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

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::acceptance::ConnectionAcceptanceStrategy;
use crate::connection::{ConnectionStrategy, UpstreamState};
use crate::crypto::{MessageHash, Signer, Verifier};
use crate::event::Event;
use crate::fanout::FanoutStrategy;
use crate::message::{
    ConnectionAction, ConnectionMessage, Message, PlainConnection, PlainMessage, SignedMessage,
};
use crate::peer::PeerId;
use crate::received::{Origin, ReceivedDelivery};
use crate::subscription_registry::MembershipEvent;
use crate::topic::TopicId;
use crate::topic_registry::{TopicEntry, TopicRegistryEvent};

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
    registered_topics: HashMap<TopicId, TopicEntry>,
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
    connection_strategy: Arc<dyn ConnectionStrategy>,
    /// The fan-out policy consulted at the record point to choose which
    /// downstream peers receive a forward of a recorded message. The deliberate
    /// twin of `connection_strategy`; the v1 implementor is `ForwardToAll` (ADR 0021).
    fanout_strategy: Arc<dyn FanoutStrategy>,
    /// The inbound-acceptance policy consulted on a verified `Request` to decide
    /// whether to accept the emitter as downstream on the topic. The inbound
    /// mirror of `connection_strategy`; the v1 implementor is
    /// `AcceptFromAllCandidates` (ADR 0023).
    acceptance_strategy: Arc<dyn ConnectionAcceptanceStrategy>,
    /// Content hashes of every message already accepted, keyed by
    /// `MessageHash::of(&plain)`. The duplicate-suppression set checked at the
    /// shared record point on both paths (after signature verification): an
    /// already-present hash is dropped (`duplicate`), which bounds forwarding in
    /// cyclic meshes and suppresses a re-published / relayed-back copy. Unbounded
    /// in the in-memory model — bounding (LRU/TTL) is deferred (ADR 0021;
    /// `IMPLEMENTATION_NOTES` N-021), needed before larger / longer multi-node runs.
    seen: HashSet<MessageHash>,
    /// Whether the node has **synced** — both registries' initial snapshots are
    /// applied, so the node is at/near the chain tip (ADR 0020). `false` while
    /// `Syncing`; set once by the `Synced` transition, which also establishes
    /// connections. The behavioural mode marker the dial waits on.
    synced: bool,
}

impl NodeState {
    /// Construct the state value from already-parsed inputs.
    pub(crate) fn new(
        self_id: PeerId,
        subscriptions: HashSet<TopicId>,
        verifier: Arc<dyn Verifier>,
        signer: Arc<dyn Signer>,
        connection_strategy: Arc<dyn ConnectionStrategy>,
        fanout_strategy: Arc<dyn FanoutStrategy>,
        acceptance_strategy: Arc<dyn ConnectionAcceptanceStrategy>,
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
            connection_strategy,
            fanout_strategy,
            acceptance_strategy,
            seen: HashSet::new(),
            synced: false,
        }
    }

    /// Whether the node has synced (both registry snapshots applied). `false`
    /// while still replaying the registries at startup.
    #[must_use]
    pub(crate) fn is_synced(&self) -> bool {
        self.synced
    }

    /// Snapshot of every recorded delivery, in processing order.
    #[must_use]
    pub(crate) fn received_snapshot(&self) -> Vec<ReceivedDelivery> {
        self.received.clone()
    }

    /// Snapshot of the node's subscription set — the actual message
    /// accept-filter (unspecified order). This is a **maintained** set: the
    /// folds keep it a subset of the registered topics (strict drop on the
    /// membership side, atomic cascade on a topic removal), so it is returned
    /// directly — no read-time intersection. A topic here is always a
    /// registered, legitimate topic.
    #[must_use]
    pub(crate) fn subscriptions_snapshot(&self) -> Vec<TopicId> {
        self.subscriptions.iter().cloned().collect()
    }

    /// Whether `topic` is currently a registered (legitimate) topic. Read by the
    /// shell's `Node::is_registered` getter (and the state tests).
    pub(crate) fn is_registered(&self, topic: &TopicId) -> bool {
        self.registered_topics.contains_key(topic)
    }

    /// The topics for which a candidate set is held (the candidate map's keys).
    #[cfg(test)]
    pub(crate) fn candidate_topics(&self) -> Vec<TopicId> {
        self.candidates.keys().cloned().collect()
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
    #[must_use]
    pub(crate) fn upstream_snapshot(&self) -> Vec<(PeerId, TopicId, UpstreamState)> {
        self.upstream
            .iter()
            .map(|((peer, topic), state)| (peer.clone(), topic.clone(), *state))
            .collect()
    }

    /// Snapshot of the downstream connections — `(peer, topic)` pairs in
    /// unspecified order. A stable clone, unaffected by later events.
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
        Event::Publish(signed) => handle_publish(state, signed),
        Event::MembershipUpdate(update) => handle_membership_update(state, update),
        Event::TopicRegistryUpdate(update) => handle_topic_registry_update(state, update),
        Event::Synced => handle_synced(state),
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
        .connection_strategy
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

/// Transition for the `Synced` signal — the node has replayed both registries'
/// initial snapshots and is at/near the chain tip (ADR 0020).
///
/// Flips the node from `Syncing` to `Synced` (the behavioural-mode marker the
/// dial waits on) and establishes connections once, on that rising edge. The
/// registry indexer pushes `Synced` exactly once after folding both snapshots;
/// the edge guard makes a redundant `Synced` a harmless no-op.
fn handle_synced(state: &mut NodeState) -> Vec<Effect> {
    if state.synced {
        return Vec::new();
    }
    state.synced = true;
    handle_connection_setup(state)
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
/// Clears both connection structures and emits one `Terminated` notice per held
/// entry — both roles, any state, including `AwaitingAccept` upstreams (FR-020).
/// A pair held in both roles is notified once per structure (two notices; the
/// redundant one is absorbed by the counterpart's unknown-termination rule).
fn handle_shutdown(state: &mut NodeState) -> Vec<Effect> {
    let self_id = state.self_id.clone();
    let signer = Arc::clone(&state.signer);
    let terminate = |peer: PeerId, topic: TopicId| Effect::Send {
        to: peer,
        message: signed_connection(
            &self_id,
            signer.as_ref(),
            ConnectionAction::Terminated { topic },
        ),
    };

    let effects: Vec<Effect> = state
        .upstream
        .keys()
        .cloned()
        .chain(state.downstream.iter().cloned())
        .map(|(peer, topic)| terminate(peer, topic))
        .collect();

    state.upstream.clear();
    state.downstream.clear();
    effects
}

/// Transition for a topic-registry delta — the **defensive** fold.
///
/// Maintains the `registered_topics` projection (topic → authorized publishers,
/// empty ⇒ open) as the source of truth for which topics legitimately exist. The
/// fold validates rather than assumes: only `Registered` creates a topic; a
/// `PublishersChanged` for a topic that is not currently registered is dropped
/// (logged), not auto-created; a `Removed` **cascades atomically** — within this
/// one fold it drops the topic from `subscriptions`, `candidates`, and both
/// connection structures (`upstream`/`downstream`) too, so the maintained
/// invariant `subscriptions/candidates ⊆ registered_topics` holds at rest with
/// no inconsistent intermediate state and no connection survives for a topic
/// that no longer legitimately exists. Pure; returns no effects.
// ADR 0020 (amends 0016); FR-002/FR-008.
fn handle_topic_registry_update(state: &mut NodeState, event: TopicRegistryEvent) -> Vec<Effect> {
    match event {
        TopicRegistryEvent::Registered { topic, publishers } => {
            state
                .registered_topics
                .insert(topic, TopicEntry::from_publishers(publishers));
        }
        TopicRegistryEvent::PublishersChanged {
            topic,
            added,
            removed,
        } => {
            // Defensive: only a Registered topic can have its publishers changed.
            // A PublishersChanged for an unknown topic is an ordering anomaly —
            // dropped, not auto-created (no `or_default`).
            if let Some(entry) = state.registered_topics.get_mut(&topic) {
                entry.apply_publishers_diff(added, &removed);
            } else {
                log_topic_not_registered(&state.self_id, &topic);
            }
        }
        TopicRegistryEvent::Removed { topic } => {
            // Atomic cascade: the topic leaves the projection AND every structure
            // keyed on it — subscriptions, candidates, and both connection roles
            // — together, in this one fold under the lock. No partial state is
            // observable, and no connection outlives the topic's legitimacy.
            state.registered_topics.remove(&topic);
            state.subscriptions.remove(&topic);
            state.candidates.remove(&topic);
            state.upstream.retain(|(_, t), _| t != &topic);
            state.downstream.retain(|(_, t)| t != &topic);
        }
    }
    Vec::new()
}

/// Operator-visibility log for a membership topic dropped because it is not a
/// registered (legitimate) topic — the defensive enforcement of the cross-
/// registry invariant. Logs are operator UX, never a test surface.
// ADR 0020; FR-003b.
fn log_topic_not_registered(self_id: &PeerId, topic: &TopicId) {
    tracing::info!(
        target: "pubsub_node::node",
        event = "message_dropped",
        cause = "topic_not_registered",
        self_id = %self_id,
        topic = %topic,
    );
}

/// Transition for a subscription-registry membership delta — **strict drop**.
///
/// The node derives its membership-side state from this single stream: an event
/// about the node's **own** id updates its subscription set; an event about
/// **any other** node updates the per-topic candidate set. Both sides are gated
/// on the registered-topics projection (the cross-registry invariant): a topic
/// not currently registered is **dropped** — not admitted to `subscriptions`,
/// not recorded as a `candidate` — and logged. There is no declared/pending
/// buffer and no auto-promotion; under the chain follower's ordering (and the
/// registry indexer folding the topic snapshot before the membership snapshot,
/// see `crate::node`) a topic is registered before any membership event
/// references it. The dial is triggered separately by `Event::Synced` once both
/// snapshots are applied. Every arm returns no effects.
// ADR 0020 (amends 0014); FR-001/FR-003/FR-003a.
fn handle_membership_update(state: &mut NodeState, event: MembershipEvent) -> Vec<Effect> {
    match event {
        MembershipEvent::Joined { node, topics } => {
            if node == state.self_id {
                // The node's own entry *is* its subscription set — but only the
                // registered topics (strict drop of unregistered ones).
                let mut subscriptions = HashSet::new();
                for topic in topics {
                    if state.registered_topics.contains_key(&topic) {
                        subscriptions.insert(topic);
                    } else {
                        log_topic_not_registered(&state.self_id, &topic);
                    }
                }
                state.subscriptions = subscriptions;
            } else {
                for topic in topics {
                    if state.registered_topics.contains_key(&topic) {
                        state
                            .candidates
                            .entry(topic)
                            .or_default()
                            .insert(node.clone());
                    } else {
                        log_topic_not_registered(&state.self_id, &topic);
                    }
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
                    if state.registered_topics.contains_key(&topic) {
                        state.subscriptions.insert(topic);
                    } else {
                        log_topic_not_registered(&state.self_id, &topic);
                    }
                }
                for topic in &removed {
                    state.subscriptions.remove(topic);
                    // No longer interested in this topic — drop its candidates.
                    state.candidates.remove(topic);
                }
            } else {
                for topic in added {
                    if state.registered_topics.contains_key(&topic) {
                        state
                            .candidates
                            .entry(topic)
                            .or_default()
                            .insert(node.clone());
                    } else {
                        log_topic_not_registered(&state.self_id, &topic);
                    }
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
        Message::Dissemination(signed) => handle_dissemination(state, from, signed),
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
/// The accept/reject *policy* is the injected [`ConnectionAcceptanceStrategy`]
/// (the inbound mirror of the dial-side `connection_strategy`); the handler owns
/// the mechanics. The v1 `AcceptFromAllCandidates` membership-validates against
/// the **membership-derived** view (registration gates delivery, not acceptance
/// — the S7 pin): the topic must be among the node's own topics AND the emitter
/// a known member of it. An accepted request records the downstream entry
/// (idempotently) and replies `Accepted` to the carried emitter; a rejected one
/// is dropped with no state change and no reply.
fn handle_connection_request(
    state: &mut NodeState,
    emitter: PeerId,
    topic: TopicId,
) -> Vec<Effect> {
    if !state
        .acceptance_strategy
        .accepts(&emitter, &topic, &state.subscriptions, &state.candidates)
    {
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

/// The shared dissemination check chain: subscribed → registered → authorized.
///
/// Returns the drop cause if a check fails, or `None` if the message passes all
/// three. This is the middle that the publish and signed-receive paths share
/// (R9). The path-specific bits stay in the callers — the connection gate
/// (receive-only), the signature-failure *action* (sever vs plain drop), the
/// `Origin` value, and the fan-out `exclude` — as does drop *logging*: this
/// returns the cause and the caller logs it with path-appropriate fields.
fn validate_dissemination(state: &NodeState, plain: &PlainMessage) -> Option<&'static str> {
    if !state.subscriptions.contains(&plain.topic) {
        return Some("topic_not_subscribed");
    }
    // Topic-validity then authorized-publisher, in a single registry lookup:
    //  - absent ⇒ subscribed (checked above) but NOT registered, i.e. 014's
    //    cross-registry invariant `subscriptions ⊆ registered_topics` is breached.
    //    The strict-drop folds maintain that invariant, so this is unreachable in
    //    normal operation; it stays as a defensive guard (ADR 0016 as amended by
    //    0020) and warns so a breach is visible (the caller still emits the routine
    //    `message_dropped` info record with the returned cause).
    //  - present ⇒ a non-open topic accepts only its authorized keys, an open
    //    topic accepts any publisher (both encoded by the declarative `TopicEntry`
    //    predicate). Checked before signature verification (a cheap lookup).
    match state.registered_topics.get(&plain.topic) {
        None => {
            tracing::warn!(
                target: "pubsub_node::node",
                event = "invariant_violation",
                invariant = "subscriptions_subset_of_registered_topics",
                self_id = %state.self_id,
                topic = %plain.topic,
            );
            Some("topic_not_registered")
        }
        Some(entry) if !entry.is_publisher_authorized(plain.publisher_id.as_public_key()) => {
            Some("publisher_not_authorized")
        }
        Some(_) => None,
    }
}

/// Compute the verbatim fan-out effects for `message` on `topic`: one
/// [`Effect::Send`] per target the strategy selects, each carrying a clone of
/// the original [`SignedMessage`] (relays never re-sign — FR-007). `exclude` is
/// the split-horizon peer — the deliverer on the receive path, `None` on the
/// publish path.
fn fanout(
    state: &NodeState,
    topic: &TopicId,
    message: &SignedMessage,
    exclude: Option<&PeerId>,
) -> Vec<Effect> {
    state
        .fanout_strategy
        .targets(topic, &state.downstream, exclude)
        .into_iter()
        .map(|to| Effect::Send {
            to,
            message: Message::Dissemination(message.clone()),
        })
        .collect()
}

/// Record a verified message and fan it out — the shared tail of both paths
/// (R9). The caller has already run every check, including signature
/// verification, so this is the single record point.
///
/// The duplicate-suppression gate sits here (FR-012/013): keyed on the content
/// hash and checked **after** verification, so a forged message that fails
/// verification never enters `seen`. An already-seen hash is dropped
/// (`duplicate`) — not recorded, not fanned out — which bounds forwarding in a
/// cyclic mesh and suppresses a re-published / relayed-back copy (FR-015). A
/// first-seen message is marked seen, recorded with the given `origin`, then
/// forwarded to the strategy-selected downstream (split-horizon `exclude`).
/// Both the publish and receive paths route through here, so they dedup
/// identically.
fn record_and_fanout(
    state: &mut NodeState,
    signed: SignedMessage,
    origin: Origin,
    exclude: Option<&PeerId>,
) -> Vec<Effect> {
    // `insert` returns false if the hash was already present: that is the
    // duplicate, dropped before any record or fan-out.
    if !state.seen.insert(MessageHash::of(&signed.plain)) {
        tracing::info!(
            target: "pubsub_node::node",
            event = "message_dropped",
            cause = "duplicate",
            self_id = %state.self_id,
            topic = %signed.plain.topic,
            publisher_id = %signed.plain.publisher_id,
        );
        return Vec::new();
    }
    let topic = signed.plain.topic.clone();
    let effects = fanout(state, &topic, &signed, exclude);
    state.received.push(ReceivedDelivery {
        origin,
        message: Message::Dissemination(signed),
    });
    effects
}

/// Transition for a locally-originated publish (`Event::Publish`).
///
/// The receive-path checks **minus** the connection gate and severance: the
/// topic must be subscribed, registered, and the publisher authorized (proxy
/// allowed — `publisher_id` need not be the node itself), and the signature must
/// verify. A failing check is a plain `message_dropped` and **never** a
/// severance (there is no upstream to sever). A passing message is recorded with
/// [`Origin::Local`] and fanned out to every downstream on the topic (no
/// split-horizon exclusion).
// FR-001..005,007,011,016; ADR 0021 §4; data-model §2.
fn handle_publish(state: &mut NodeState, signed: SignedMessage) -> Vec<Effect> {
    if let Some(cause) = validate_dissemination(state, &signed.plain) {
        tracing::info!(
            target: "pubsub_node::node",
            event = "message_dropped",
            cause,
            self_id = %state.self_id,
            topic = %signed.plain.topic,
            publisher_id = %signed.plain.publisher_id,
        );
        return Vec::new();
    }

    if state
        .verifier
        .verify(
            signed.plain.publisher_id.as_public_key(),
            &signed.plain.signed_bytes(),
            &signed.signature,
        )
        .is_err()
    {
        // A publish has no upstream to sever — an invalid signature here is a
        // plain drop, not misbehavior (FR-004).
        tracing::info!(
            target: "pubsub_node::node",
            event = "message_dropped",
            cause = "invalid_signature",
            self_id = %state.self_id,
            topic = %signed.plain.topic,
            publisher_id = %signed.plain.publisher_id,
        );
        return Vec::new();
    }

    record_and_fanout(state, signed, Origin::Local, None)
}

/// Transition for a signed dissemination message.
///
/// Records the delivery when the **delivering peer holds an Active upstream**
/// for the message's topic (the connection gate, FR-016), its topic is
/// subscribed **and** a registered (legitimate) topic, its publisher is
/// authorized, and its signature verifies; otherwise the message is dropped
/// (with an info-level `message_dropped` event carrying the cause). A recorded
/// message is then fanned out to the node's other downstream on the topic,
/// excluding the deliverer (split-horizon) — the same record-and-forward tail
/// the publish path uses (FR-006/007/009).
// FR-016: the connection gate is the FIRST check (keyed on the delivering
// peer — a payload carries a publisher identity, not the sender's); the
// pre-existing chain runs unchanged after it — subscribed?, registered?,
// authorized?, signature? (ADR 0016). A signature failure past every earlier
// check, over an Active upstream, is misbehavior and severs (FR-017); the
// fan-out happens only past the record point.
fn handle_dissemination(state: &mut NodeState, from: PeerId, signed: SignedMessage) -> Vec<Effect> {
    // FR-016: admit only from an Active upstream for this topic.
    let connected = matches!(
        state
            .upstream
            .get(&(from.clone(), signed.plain.topic.clone())),
        Some(UpstreamState::Active),
    );
    if !connected {
        tracing::info!(
            target: "pubsub_node::node",
            event = "message_dropped",
            cause = "not_connected",
            self_id = %state.self_id,
            from = %from,
            topic = %signed.plain.topic,
        );
        return Vec::new();
    }

    // The shared subscribed → registered → authorized chain (R9); a failure is a
    // plain drop logged with the receive-path `from=` field. The connection gate
    // above and the signature-failure severance below stay path-specific.
    if let Some(cause) = validate_dissemination(state, &signed.plain) {
        tracing::info!(
            target: "pubsub_node::node",
            event = "message_dropped",
            cause,
            self_id = %state.self_id,
            from = %from,
            topic = %signed.plain.topic,
            publisher_id = %signed.plain.publisher_id,
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
        // FR-017: reaching signature verification means the connection gate,
        // subscription, registration, and authorization checks all passed — so
        // a failure here, over an Active upstream, is misbehavior. Sever
        // silently: remove the upstream entry and raise the misbehavior signal
        // (the executor logs `connection_severed`); no Terminated is sent.
        let topic = signed.plain.topic.clone();
        state.upstream.remove(&(from.clone(), topic.clone()));
        return vec![Effect::Misbehaved {
            peer: from,
            topic,
            cause: "invalid_signature",
        }];
    }

    // Record the delivery (origin = the delivering peer) and fan it out to the
    // node's other downstream on the topic, excluding the deliverer
    // (split-horizon). The shared record-and-forward tail with the publish path
    // (R9); the publish path passes `Origin::Local` and no exclusion.
    record_and_fanout(state, signed, Origin::Peer(from.clone()), Some(&from))
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
    use crate::acceptance::AcceptFromAllCandidates;
    use crate::connection::test_support::{
        accepted_from, membership_joined, misattributed_request, payload_from, request_from,
        tampered_payload_from, terminated_from, ConnectionScript,
    };
    use crate::connection::ConnectToAllCandidates;
    use crate::crypto::mock::{MockCryptoScheme, TestSigner, TestVerifier};
    use crate::crypto::PublicKey;
    use crate::crypto::{Signer, Timestamp};
    use crate::fanout::ForwardToAll;
    use crate::message::{MessagePayload, PlainMessage, SignedMessage};
    use crate::subscription_registry::MembershipScript;
    use crate::topic_registry::TopicRegistryScript;
    use std::collections::BTreeSet;

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
    /// own coherent signer, and the v1 strategy — the common test setup. Each
    /// `subscriptions` topic is also registered **open**, so it is a legitimate
    /// topic: under the 014 cross-registry invariant, membership/candidate
    /// gating and dialing only admit registered topics, so a connection or
    /// delivery test that names a topic must have it registered. Tests that
    /// specifically exercise *unregistered* topics build state explicitly and
    /// register (or omit) topics themselves.
    fn node_state(self_id: &str, subscriptions: HashSet<TopicId>) -> NodeState {
        let mut state = NodeState::new(
            peer(self_id),
            subscriptions.clone(),
            Arc::new(TestVerifier),
            alias_signer(self_id),
            strategy(),
            Arc::new(ForwardToAll),
            Arc::new(AcceptFromAllCandidates),
        );
        for t in subscriptions {
            state
                .registered_topics
                .insert(t, TopicEntry::from_publishers(BTreeSet::new()));
        }
        state
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
        Message::Dissemination(SignedMessage { plain, signature })
    }

    /// Same as [`signed_ping`] but with the payload altered after signing,
    /// so the signature no longer verifies (the suite's mismatch pattern).
    fn tampered_ping(signer: &TestSigner, topic: TopicId, n: u64) -> Message {
        let Message::Dissemination(mut sm) = signed_ping(signer, topic, n) else {
            unreachable!("signed_ping always builds a Message::Dissemination");
        };
        sm.plain.payload = MessagePayload::Ping(n.wrapping_add(1));
        Message::Dissemination(sm)
    }

    // FR-001 / US2-AS1: subscribed topic + valid signature => recorded, in
    // order, with no effects and no I/O.
    #[test]
    fn valid_messages_recorded_in_processing_order() {
        let t1 = topic("t1");
        let mut state = state_subscribed(vec![t1.clone()]);
        // Establishment preamble: both senders are Active upstreams on t1, so
        // their payload passes the connection gate (FR-016).
        with_active_upstream(&mut state, "a", "t1");
        with_active_upstream(&mut state, "b", "t1");
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
        assert!(effects.is_empty(), "recording produces no effects");
        let snap = state.received_snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].origin, Origin::Peer(peer("a")));
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
        assert_eq!(snap[1].origin, Origin::Peer(peer("b")));
        assert_eq!(snap[1].message, m2);
    }

    // FR-002 / US2-AS2: off-topic message leaves state unchanged.
    #[test]
    fn off_topic_message_leaves_state_unchanged() {
        let t1 = topic("t1");
        let mut state = state_subscribed(vec![t1.clone()]);
        // a is Active on both topics — both payloads pass the gate, so this
        // genuinely exercises the subscription filter behind it (t2 is the
        // off-topic one, dropped by subscription, not by the gate).
        with_active_upstream(&mut state, "a", "t1");
        with_active_upstream(&mut state, "a", "t2");
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

    // FR-003 / FR-017: subscribed topic but invalid signature over an Active
    // upstream => dropped AND severed (the signature failure past every earlier
    // check is misbehavior). Detailed severance coverage is in the T021 block;
    // this is the 003-era receive test, updated for the connection model.
    #[test]
    fn invalid_signature_message_dropped() {
        let t1 = topic("t1");
        let mut state = state_subscribed(vec![t1.clone()]);
        // a is Active on t1, so the tampered payload passes the gate and reaches
        // the signature check.
        with_active_upstream(&mut state, "a", "t1");
        let s = signer();

        let effects = apply(
            &mut state,
            Event::MessageReceived {
                from: peer("a"),
                message: tampered_ping(&s, t1, 1),
            },
        );
        assert_eq!(
            misbehaved(&effects),
            vec![(peer("a"), topic("t1"), "invalid_signature")],
            "tampered over an Active upstream severs",
        );
        assert!(
            state.received_snapshot().is_empty(),
            "tampered message never recorded"
        );
    }

    // Edge case: an empty subscription set drops every inbound message.
    #[test]
    fn empty_subscription_set_drops_everything() {
        let mut state = state_subscribed(vec![]);
        // a is Active on t1, so the payloads pass the gate and are dropped by
        // the (empty) subscription filter — the behavior under test.
        with_active_upstream(&mut state, "a", "t1");
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

        // Both senders Active on the topics they use, so the script exercises
        // the full post-gate chain identically across the two runs.
        let seed = |state: &mut NodeState| {
            with_active_upstream(state, "a", "t1");
            with_active_upstream(state, "b", "t1");
            with_active_upstream(state, "b", "t2");
            with_active_upstream(state, "c", "t1");
        };

        // The tampered (b, t1) event severs that upstream (returning a
        // Misbehaved effect), so the per-step effects are not all empty; the
        // determinism claim is that the same script yields the same final state.
        let mut first = state_subscribed(vec![t1.clone()]);
        seed(&mut first);
        for event in script() {
            apply(&mut first, event);
        }
        let mut second = state_subscribed(vec![t1.clone()]);
        seed(&mut second);
        for event in script() {
            apply(&mut second, event);
        }

        assert_eq!(first.received_snapshot(), second.received_snapshot());
        assert_eq!(
            sorted_pairs(
                first
                    .upstream_snapshot()
                    .into_iter()
                    .map(|(p, t, _)| (p, t))
                    .collect()
            ),
            sorted_pairs(
                second
                    .upstream_snapshot()
                    .into_iter()
                    .map(|(p, t, _)| (p, t))
                    .collect()
            ),
            "the severed (b, t1) upstream is gone in both runs",
        );
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
                                                  // a is an Active upstream on t1 throughout — the gate is open; what
                                                  // changes across the test is the subscription filter behind it.
        with_active_upstream(&mut state, "a", "t1");
        let s = signer();

        apply(
            &mut state,
            Event::MessageReceived {
                from: peer("a"),
                message: signed_ping(&s, t1.clone(), 1),
            },
        );
        assert!(state.received_snapshot().is_empty(), "not subscribed yet");

        // Chain order (strict drop): t1 is registered FIRST, then the node's own
        // entry arrives on the membership stream → admitted → t1 is now
        // effective and subsequent messages are accepted.
        apply(&mut state, reg_open("t1"));
        apply(
            &mut state,
            Event::MembershipUpdate(MembershipEvent::joined("self", ["t1"])),
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

    // ── 014: maintained invariant + strict drop + candidate gating + defensive
    // fold + atomic cascade + the membership-readiness dial trigger. These
    // assert the maintained-state model (not a read-time intersection). ──

    /// Assert both subset invariants hold for the current state.
    fn assert_invariants(state: &NodeState) {
        for t in state.subscriptions_snapshot() {
            assert!(
                state.is_registered(&t),
                "INV-1: subscription {t} not registered"
            );
        }
        for t in state.candidate_topics() {
            assert!(
                state.is_registered(&t),
                "INV-2: candidate topic {t} not registered"
            );
        }
    }

    // SC-001/SC-008/FR-003: a self-subscription naming an unregistered topic is
    // strict-dropped (never enters the set); registering it later does NOT
    // promote it — a fresh membership event is required.
    #[test]
    fn strict_drop_self_no_auto_promotion() {
        let mut state = node_state("self", HashSet::new());
        apply(&mut state, reg_open("weather"));
        apply(
            &mut state,
            Event::MembershipUpdate(MembershipEvent::joined("self", ["weather", "ghost"])),
        );
        assert_eq!(
            sorted(state.subscriptions_snapshot()),
            vec![topic("weather")],
            "ghost is unregistered → strict-dropped, never in the set",
        );
        assert_invariants(&state);

        apply(&mut state, reg_open("ghost"));
        assert_eq!(
            sorted(state.subscriptions_snapshot()),
            vec![topic("weather")],
            "registering ghost later must NOT auto-promote the dropped subscription",
        );

        apply(
            &mut state,
            Event::MembershipUpdate(MembershipEvent::topics_changed("self", ["ghost"], [])),
        );
        assert_eq!(
            sorted(state.subscriptions_snapshot()),
            vec![topic("ghost"), topic("weather")],
        );
        assert_invariants(&state);
    }

    // SC-008/FR-003a: candidate gating — a candidate (other node) on an
    // unregistered topic is not recorded; candidate topics ⊆ registered.
    #[test]
    fn candidate_gating_drops_unregistered() {
        let mut state = node_state("self", HashSet::new());
        apply(&mut state, reg_open("weather"));
        apply(
            &mut state,
            Event::MembershipUpdate(MembershipEvent::joined("b", ["weather", "ghost"])),
        );
        assert_eq!(
            state.candidates_snapshot(&topic("weather")),
            vec![peer("b")]
        );
        assert!(
            state.candidates_snapshot(&topic("ghost")).is_empty(),
            "candidate on an unregistered topic is not recorded",
        );
        assert_invariants(&state);
    }

    // SC-010/FR-008: defensive fold — PublishersChanged for an unregistered
    // topic does NOT create it (no or_default); only Registered creates.
    #[test]
    fn defensive_fold_publishers_changed_does_not_create() {
        let mut state = node_state("self", HashSet::new());
        apply(
            &mut state,
            Event::TopicRegistryUpdate(TopicRegistryEvent::PublishersChanged {
                topic: topic("ghost"),
                added: BTreeSet::from([pk(b"k1")]),
                removed: BTreeSet::new(),
            }),
        );
        assert!(
            !state.is_registered(&topic("ghost")),
            "PublishersChanged on an unknown topic must not create it",
        );
    }

    // SC-002/SC-003/FR-002: atomic cascade — a Removed clears the topic from
    // subscriptions, candidates, AND both connection structures together.
    #[test]
    fn removed_cascades_to_subscriptions_candidates_and_connections() {
        let mut state = node_state("self", HashSet::new());
        apply(&mut state, reg_open("weather"));
        apply(
            &mut state,
            Event::MembershipUpdate(MembershipEvent::joined("self", ["weather"])),
        );
        apply(
            &mut state,
            Event::MembershipUpdate(MembershipEvent::joined("b", ["weather"])),
        );
        // Hold a connection in each role on weather.
        with_active_upstream(&mut state, "b", "weather");
        state.downstream.insert((peer("c"), topic("weather")));
        assert_eq!(state.subscriptions_snapshot(), vec![topic("weather")]);
        assert_eq!(
            state.candidates_snapshot(&topic("weather")),
            vec![peer("b")]
        );

        apply(
            &mut state,
            Event::TopicRegistryUpdate(TopicRegistryEvent::Removed {
                topic: topic("weather"),
            }),
        );
        assert!(
            state.subscriptions_snapshot().is_empty(),
            "cascade: subscription cleared"
        );
        assert!(
            state.candidates_snapshot(&topic("weather")).is_empty(),
            "cascade: candidates cleared",
        );
        assert_eq!(
            upstream_state(&state, "b", "weather"),
            None,
            "cascade: upstream cleared"
        );
        assert!(
            !has_downstream(&state, "c", "weather"),
            "cascade: downstream cleared",
        );
        assert!(
            !state.is_registered(&topic("weather")),
            "cascade: projection cleared"
        );
        assert_invariants(&state);
    }

    // ADR 0020 (2026-06-18 snapshot-reshape): `Event::Synced` is the single
    // readiness signal — the registry indexer pushes it once both registry
    // snapshots are folded. Folding it flips the node to `Synced` and dials, on
    // the rising edge only.
    #[test]
    fn synced_transitions_and_dials_idempotently() {
        let mut state = node_state("self", HashSet::new());
        apply(&mut state, reg_open("t1"));
        apply(&mut state, membership_joined("self", ["t1"]));
        apply(&mut state, membership_joined("a", ["t1"]));

        // Before sync: not synced, no dial.
        assert!(!state.is_synced(), "node starts in Syncing");
        assert_eq!(
            upstream_state(&state, "a", "t1"),
            None,
            "no dial before sync"
        );

        // Synced flips the mode and dials the candidate once.
        let effects = apply(&mut state, Event::Synced);
        assert!(state.is_synced(), "Synced transitions the node to Synced");
        assert_eq!(
            upstream_state(&state, "a", "t1"),
            Some(UpstreamState::AwaitingAccept),
            "Synced dials the candidate",
        );
        assert_eq!(
            request_sends(&effects, "self"),
            vec![(peer("a"), topic("t1"))],
            "Synced returns the dial Request",
        );

        // Idempotent: a redundant Synced after the transition is a no-op.
        let effects = apply(&mut state, Event::Synced);
        assert!(
            effects.is_empty(),
            "a redundant Synced re-emits nothing (edge-guarded)",
        );
    }

    // (The 013 subscribe-before-register-then-promote test is retired with 013
    // SC-004: under 014 strict drop there is no promotion. Strict drop +
    // no-promotion is covered by `strict_drop_self_no_auto_promotion`, and the
    // removal cascade by `removed_cascades_to_subscriptions_and_candidates`.)

    // US2 / SC-004 (014): a topic removed from the registry cascades out of the
    // subscription set (register-first, then remove).
    #[test]
    fn removing_a_topic_makes_it_ineffective() {
        let mut state = node_state("self", HashSet::new());
        // Chain order: register weather, then the node subscribes to it.
        apply(&mut state, reg_open("weather"));
        apply(
            &mut state,
            Event::MembershipUpdate(MembershipEvent::joined("self", ["weather"])),
        );
        assert_eq!(state.subscriptions_snapshot(), vec![topic("weather")]);
        // Removal cascades it out of the subscription set.
        apply(
            &mut state,
            Event::TopicRegistryUpdate(TopicRegistryEvent::Removed {
                topic: topic("weather"),
            }),
        );
        assert!(
            state.subscriptions_snapshot().is_empty(),
            "removed → cascaded out of the subscription set",
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
        // relay is an Active upstream on weather — the gate is open; what this
        // test exercises behind it is publisher authorization.
        with_active_upstream(&mut state, "relay", "weather");
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
        // relay is an Active upstream on weather — the payload passes the gate
        // and the authorized publisher passes authorization, so it reaches (and
        // is dropped at) signature verification.
        with_active_upstream(&mut state, "relay", "weather");
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
        apply(&mut state, reg_open("t1")); // legitimate topic (registered first)
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

    // 014 closes the 004 S7 gap (N-015): under strict drop a topic the node has
    // not registered is never admitted to its subscription/candidate sets, so a
    // connection Request on an unregistered topic fails membership validation —
    // acceptance is now consistent with registration (no connection establishes
    // on a topic that does not legitimately exist). This supersedes 004's
    // "accept on the membership-derived set despite no registration" pin.
    #[test]
    fn request_for_unregistered_topic_is_rejected() {
        let mut state = node_state("self", HashSet::new()); // t1 deliberately unregistered
        apply(&mut state, membership_joined("a", ["t1"])); // candidate-gated out
        assert!(
            state.subscriptions_snapshot().is_empty(),
            "t1 unregistered → strict-dropped, not in the subscription set",
        );

        let effects = apply(&mut state, request_from("a", "t1"));

        assert!(
            !has_downstream(&state, "a", "t1"),
            "no connection established on an unregistered topic",
        );
        assert!(
            accepted_sends(&effects, "self").is_empty(),
            "request on an unregistered topic is not accepted",
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

    // ---- T017: connection-gated delivery (US2, FR-016/019) --------------------

    /// Seed an Active upstream `(peer, topic)` directly — the declarative
    /// stand-in for a full setup→accept handshake when a test only needs the
    /// gate to be open (the test module reaches `NodeState`'s private fields).
    fn with_active_upstream(state: &mut NodeState, peer_alias: &str, t: &str) {
        state
            .upstream
            .insert((peer(peer_alias), topic(t)), UpstreamState::Active);
    }

    // US2-AS1 / FR-016: a validly-signed payload from an Active upstream is
    // recorded — the post-connection receive path is unchanged.
    #[test]
    fn payload_over_active_upstream_is_recorded() {
        let mut state = state_subscribed(vec![topic("t1")]);
        with_active_upstream(&mut state, "b", "t1");

        let effects = apply(&mut state, payload_from("b", "t1", 1));
        assert!(effects.is_empty());
        assert_eq!(state.received_snapshot().len(), 1, "admitted and recorded");
    }

    // US2-AS2 / SC-002: a payload from a sender with no connection is dropped
    // (not_connected) — pre-connection delivery is retired.
    #[test]
    fn payload_without_connection_is_dropped() {
        let mut state = state_subscribed(vec![topic("t1")]);
        // No upstream seeded.
        let effects = apply(&mut state, payload_from("b", "t1", 1));
        assert!(effects.is_empty());
        assert!(
            state.received_snapshot().is_empty(),
            "no Active upstream → not_connected drop",
        );
    }

    // US2-AS2 / SC-002: an AwaitingAccept connection does not admit payload —
    // only Active does.
    #[test]
    fn payload_over_awaiting_accept_is_dropped() {
        let mut state = state_subscribed(vec![topic("t1")]);
        state
            .upstream
            .insert((peer("b"), topic("t1")), UpstreamState::AwaitingAccept);

        let effects = apply(&mut state, payload_from("b", "t1", 1));
        assert!(effects.is_empty());
        assert!(
            state.received_snapshot().is_empty(),
            "pending connection admits nothing",
        );
    }

    // US2-AS3: connections are per-topic — an Active upstream for t1 does not
    // admit the same peer's traffic on t2.
    #[test]
    fn connection_is_per_topic() {
        let mut state = state_subscribed(vec![topic("t1"), topic("t2")]);
        with_active_upstream(&mut state, "b", "t1");

        // t1 from b → admitted; t2 from b → dropped (no connection for t2),
        // even though t2 is subscribed and registered.
        apply(&mut state, payload_from("b", "t1", 1));
        let effects = apply(&mut state, payload_from("b", "t2", 2));
        assert!(effects.is_empty());
        assert_eq!(
            state.received_snapshot().len(),
            1,
            "only t1 admitted; t2 has no connection",
        );
    }

    // US2-AS4 / FR-019: the gate is the FIRST check; the merged chain after it
    // is unchanged — a tampered payload over an Active upstream reaches
    // signature verification, where it is dropped and (US3, FR-017) severs the
    // connection.
    #[test]
    fn gate_first_then_signature_check_unchanged() {
        let mut state = state_subscribed(vec![topic("t1")]);
        with_active_upstream(&mut state, "b", "t1");

        let effects = apply(&mut state, tampered_payload_from("b", "t1", 1));
        assert_eq!(
            misbehaved(&effects),
            vec![(peer("b"), topic("t1"), "invalid_signature")],
            "admitted by the gate, then severed at signature",
        );
        assert!(
            state.received_snapshot().is_empty(),
            "tampered not recorded"
        );
    }

    // US2-AS4 / FR-019: a payload that passes the gate but is off the
    // subscription set still drops by the subscription filter (the gate keys on
    // (sender, topic) independent of subscription; the filter runs after it).
    #[test]
    fn gate_first_then_subscription_filter_unchanged() {
        // Subscribed+registered only for t1; seed an Active upstream for the
        // unsubscribed t2 (an own-topic-drift stale state, S4).
        let mut state = state_subscribed(vec![topic("t1")]);
        with_active_upstream(&mut state, "b", "t2");

        let effects = apply(&mut state, payload_from("b", "t2", 1));
        assert!(effects.is_empty());
        assert!(
            state.received_snapshot().is_empty(),
            "passes the gate but t2 is not subscribed → topic_not_subscribed drop",
        );
    }

    // ---- T021: misbehavior severance (US3, FR-017/018) ------------------------

    /// The mock public key for an alias (the publisher key `tampered_payload_from`
    /// / `payload_from` sign under for that alias).
    fn alias_public(alias: &str) -> PublicKey {
        MockCryptoScheme::with_seed([0u8; 32])
            .keypair_from_alias(alias)
            .public
    }

    /// The `(peer, topic, cause)` of every `Misbehaved` effect.
    fn misbehaved(effects: &[Effect]) -> Vec<(PeerId, TopicId, &'static str)> {
        effects
            .iter()
            .filter_map(|effect| match effect {
                Effect::Misbehaved { peer, topic, cause } => {
                    Some((peer.clone(), topic.clone(), *cause))
                }
                Effect::Send { .. } => None,
            })
            .collect()
    }

    /// Whether any effect is a `Send` (severance must send nothing).
    fn has_send(effects: &[Effect]) -> bool {
        effects.iter().any(|e| matches!(e, Effect::Send { .. }))
    }

    // US3-AS1 / FR-017: a tampered payload over an Active upstream (having passed
    // the gate, subscription, registration, authorization) severs that upstream
    // — entry removed, one Misbehaved effect, no Send, nothing recorded.
    #[test]
    fn tampered_over_active_upstream_severs() {
        let mut state = state_subscribed(vec![topic("t1")]);
        with_active_upstream(&mut state, "b", "t1");

        let effects = apply(&mut state, tampered_payload_from("b", "t1", 1));

        assert_eq!(upstream_state(&state, "b", "t1"), None, "upstream removed");
        assert_eq!(
            misbehaved(&effects),
            vec![(peer("b"), topic("t1"), "invalid_signature")],
        );
        assert!(
            !has_send(&effects),
            "severance is silent — no Terminated sent"
        );
        assert!(
            state.received_snapshot().is_empty(),
            "tampered never recorded"
        );
    }

    // FR-017: severance fires only *past* authorization — an authorized
    // publisher's tampered message over an Active upstream is severed.
    #[test]
    fn severance_fires_past_authorization() {
        let weather = topic("weather");
        let mut state = node_state("self", HashSet::from([weather.clone()]));
        // weather restricted to b's key (the publisher tampered_payload_from
        // signs under), so authorization passes and the signature check is
        // reached.
        apply(
            &mut state,
            Event::TopicRegistryUpdate(TopicRegistryEvent::Registered {
                topic: weather.clone(),
                publishers: BTreeSet::from([alias_public("b")]),
            }),
        );
        with_active_upstream(&mut state, "b", "weather");

        let effects = apply(&mut state, tampered_payload_from("b", "weather", 1));
        assert_eq!(upstream_state(&state, "b", "weather"), None, "severed");
        assert_eq!(
            misbehaved(&effects),
            vec![(peer("b"), weather, "invalid_signature")],
        );
    }

    // US3-AS3: an invalid-signature message from a peer with no Active connection
    // is a plain not_connected drop — never a severance (a forged sender must not
    // cost the genuine peer anything).
    #[test]
    fn no_severance_without_connection() {
        let mut state = state_subscribed(vec![topic("t1")]);
        // No upstream seeded.
        let effects = apply(&mut state, tampered_payload_from("b", "t1", 1));
        assert!(
            misbehaved(&effects).is_empty(),
            "no connection → no severance"
        );
        assert!(effects.is_empty());
    }

    // US3-AS4 / FR-018: a tampered message dropped by an *earlier* check (not
    // subscribed, not registered, not authorized) never reaches the signature
    // verdict, so it never severs and leaves the entry intact.
    #[test]
    fn no_severance_when_an_earlier_check_fails() {
        // (a) topic not subscribed — Active upstream on an unsubscribed t2.
        let mut state = state_subscribed(vec![topic("t1")]);
        with_active_upstream(&mut state, "b", "t2");
        let effects = apply(&mut state, tampered_payload_from("b", "t2", 1));
        assert!(
            misbehaved(&effects).is_empty(),
            "not subscribed → no severance"
        );
        assert_eq!(
            upstream_state(&state, "b", "t2"),
            Some(UpstreamState::Active),
            "entry intact",
        );

        // (b) topic not registered — a subscribed-but-unregistered topic, which
        // 014's invariant normally makes unreachable (strict drop); constructed
        // directly here to confirm the receive-path registration guard still
        // drops (no severance) defensively if the invariant is ever violated.
        let mut state = node_state("self", HashSet::new());
        state.subscriptions.insert(topic("t1")); // bypass strict drop; t1 left unregistered
        with_active_upstream(&mut state, "b", "t1");
        let effects = apply(&mut state, tampered_payload_from("b", "t1", 1));
        assert!(
            misbehaved(&effects).is_empty(),
            "not registered → no severance"
        );
        assert_eq!(
            upstream_state(&state, "b", "t1"),
            Some(UpstreamState::Active)
        );

        // (c) publisher not authorized — restricted topic, b's key not in the set.
        let weather = topic("weather");
        let mut state = node_state("self", HashSet::from([weather.clone()]));
        apply(
            &mut state,
            Event::TopicRegistryUpdate(TopicRegistryEvent::Registered {
                topic: weather.clone(),
                publishers: BTreeSet::from([alias_public("someone-else")]),
            }),
        );
        with_active_upstream(&mut state, "b", "weather");
        let effects = apply(&mut state, tampered_payload_from("b", "weather", 1));
        assert!(
            misbehaved(&effects).is_empty(),
            "not authorized → no severance"
        );
        assert_eq!(
            upstream_state(&state, "b", "weather"),
            Some(UpstreamState::Active),
        );
    }

    // US3-AS2 / SC-003: after severance, a subsequent *valid* message from the
    // same peer on that topic is dropped not_connected (the connection is gone).
    #[test]
    fn post_severance_valid_message_is_not_connected() {
        let mut state = state_subscribed(vec![topic("t1")]);
        with_active_upstream(&mut state, "b", "t1");
        apply(&mut state, tampered_payload_from("b", "t1", 1)); // severs
        assert_eq!(upstream_state(&state, "b", "t1"), None);

        let effects = apply(&mut state, payload_from("b", "t1", 2));
        assert!(effects.is_empty());
        assert!(
            state.received_snapshot().is_empty(),
            "a valid message over the severed connection is dropped not_connected",
        );
    }

    // SC-003: severance is scoped to the one (peer, topic) — the offender's
    // other-topic connection and other peers' connections are untouched.
    #[test]
    fn severance_isolates_other_topics_and_peers() {
        let mut state = state_subscribed(vec![topic("t1"), topic("t2")]);
        with_active_upstream(&mut state, "b", "t1");
        with_active_upstream(&mut state, "b", "t2");
        with_active_upstream(&mut state, "c", "t1");

        apply(&mut state, tampered_payload_from("b", "t1", 1)); // severs (b, t1) only

        assert_eq!(upstream_state(&state, "b", "t1"), None, "severed pair gone");
        assert_eq!(
            upstream_state(&state, "b", "t2"),
            Some(UpstreamState::Active),
            "offender's other topic intact",
        );
        assert_eq!(
            upstream_state(&state, "c", "t1"),
            Some(UpstreamState::Active),
            "other peer intact",
        );
    }

    // ---- T024: graceful shutdown & Terminated reception (US4, FR-014/020) -----

    /// Seed a downstream entry `(peer, topic)` directly.
    fn with_downstream(state: &mut NodeState, peer_alias: &str, t: &str) {
        state.downstream.insert((peer(peer_alias), topic(t)));
    }

    /// The `(to, topic)` of every `Terminated` send effect (asserting emitter).
    fn terminated_sends(effects: &[Effect], expected_emitter: &str) -> Vec<(PeerId, TopicId)> {
        let mut out = Vec::new();
        for effect in effects {
            if let Effect::Send {
                to,
                message: Message::Connection(cm),
            } = effect
            {
                if let ConnectionAction::Terminated { topic } = &cm.plain.action {
                    assert_eq!(
                        cm.plain.emitter,
                        peer(expected_emitter),
                        "terminated emitter"
                    );
                    out.push((to.clone(), topic.clone()));
                }
            }
        }
        out
    }

    // US4-AS1 / FR-020: shutdown clears both structures and emits one Terminated
    // per entry in both roles, any state — including AwaitingAccept upstreams.
    #[test]
    fn shutdown_notifies_every_entry_including_awaiting_accept() {
        let mut state = node_state("self", HashSet::new());
        state
            .upstream
            .insert((peer("b"), topic("t1")), UpstreamState::Active);
        state
            .upstream
            .insert((peer("c"), topic("t1")), UpstreamState::AwaitingAccept);
        with_downstream(&mut state, "d", "t1");

        let effects = apply(&mut state, Event::Shutdown);

        assert!(state.upstream_snapshot().is_empty(), "upstream cleared");
        assert!(state.downstream_snapshot().is_empty(), "downstream cleared");
        assert_eq!(
            sorted_pairs(terminated_sends(&effects, "self")),
            sorted_pairs(vec![
                (peer("b"), topic("t1")),
                (peer("c"), topic("t1")), // the AwaitingAccept upstream is notified too
                (peer("d"), topic("t1")),
            ]),
            "one Terminated per held entry, both roles, any state",
        );
    }

    // FR-020: a pair held in BOTH roles is notified once per structure (two
    // Terminated notices — the redundant one is absorbed by the counterpart's
    // unknown-termination rule).
    #[test]
    fn shutdown_notifies_each_role_of_a_both_roles_pair() {
        let mut state = node_state("self", HashSet::new());
        state
            .upstream
            .insert((peer("b"), topic("t1")), UpstreamState::Active);
        with_downstream(&mut state, "b", "t1");

        let effects = apply(&mut state, Event::Shutdown);
        assert_eq!(
            terminated_sends(&effects, "self").len(),
            2,
            "both the upstream and downstream entry are notified",
        );
    }

    // US4-AS2 / FR-014: a Terminated removes the matching entry in either role,
    // with no reply (the reception side of graceful shutdown).
    #[test]
    fn terminated_reception_removes_either_role() {
        let mut state = node_state("self", HashSet::from([topic("t1")]));
        apply(&mut state, membership_joined("b", ["t1"]));
        state
            .upstream
            .insert((peer("b"), topic("t1")), UpstreamState::Active);
        with_downstream(&mut state, "b", "t1");

        let effects = apply(&mut state, terminated_from("b", "t1"));
        assert!(effects.is_empty(), "Terminated is never replied to");
        assert_eq!(upstream_state(&state, "b", "t1"), None, "upstream removed");
        assert!(!has_downstream(&state, "b", "t1"), "downstream removed");
    }

    // ---- T027: full-lifecycle observability (US5, SC-006/SC-007) --------------

    // SC-006: every spec-defined transition is reachable by feeding events alone
    // (timer expiry is itself an event), asserted step by step; SC-007: the node
    // never appears in its own connection state.
    #[test]
    fn full_lifecycle_reachable_by_events_alone() {
        let t = topic("t");
        let mut state = node_state("self", HashSet::from([t.clone()]));
        apply(&mut state, reg_open("t")); // registered, so payload can be admitted
        apply(&mut state, membership_joined("b", ["t"]));

        // setup → AwaitingAccept upstream + a Request.
        let e = apply(&mut state, Event::ConnectionSetup);
        assert_eq!(
            upstream_state(&state, "b", "t"),
            Some(UpstreamState::AwaitingAccept)
        );
        assert_eq!(request_sends(&e, "self"), vec![(peer("b"), t.clone())]);

        // recurring setup re-dials the still-pending pair (entry kept).
        let e = apply(&mut state, Event::ConnectionSetup);
        assert_eq!(
            request_sends(&e, "self"),
            vec![(peer("b"), t.clone())],
            "re-dial"
        );
        assert_eq!(
            upstream_state(&state, "b", "t"),
            Some(UpstreamState::AwaitingAccept)
        );

        // Accepted → Active.
        apply(&mut state, accepted_from("b", "t"));
        assert_eq!(
            upstream_state(&state, "b", "t"),
            Some(UpstreamState::Active)
        );

        // inbound Request → downstream recorded + Accepted (both roles now held).
        let e = apply(&mut state, request_from("b", "t"));
        assert!(has_downstream(&state, "b", "t"));
        assert_eq!(accepted_sends(&e, "self"), vec![(peer("b"), t.clone())]);

        // payload admitted over the Active upstream.
        apply(&mut state, payload_from("b", "t", 1));
        assert_eq!(state.received_snapshot().len(), 1, "admitted");

        // tampered payload → silent severance (upstream gone; downstream survives).
        let e = apply(&mut state, tampered_payload_from("b", "t", 2));
        assert_eq!(upstream_state(&state, "b", "t"), None, "severed");
        assert_eq!(
            misbehaved(&e),
            vec![(peer("b"), t.clone(), "invalid_signature")]
        );
        assert!(
            has_downstream(&state, "b", "t"),
            "downstream survives severance"
        );

        // Terminated → downstream removed.
        apply(&mut state, terminated_from("b", "t"));
        assert!(!has_downstream(&state, "b", "t"));

        // re-establish, then graceful shutdown clears everything with notices.
        apply(&mut state, Event::ConnectionSetup);
        apply(&mut state, accepted_from("b", "t"));
        apply(&mut state, request_from("b", "t"));
        let e = apply(&mut state, Event::Shutdown);
        assert!(state.upstream_snapshot().is_empty() && state.downstream_snapshot().is_empty());
        assert!(
            !terminated_sends(&e, "self").is_empty(),
            "shutdown notifies"
        );

        // SC-007: self never appears in either structure across the lifecycle.
        let self_peer = peer("self");
        assert!(state
            .upstream_snapshot()
            .iter()
            .all(|(p, _, _)| p != &self_peer));
        assert!(state
            .downstream_snapshot()
            .iter()
            .all(|(p, _)| p != &self_peer));

        // Determinism: the same ConnectionScript twice yields the same final state.
        let run = || {
            let mut s = node_state("self", HashSet::from([t.clone()]));
            apply(&mut s, reg_open("t"));
            let script = ConnectionScript::new()
                .member_joined("b", ["t"])
                .setup()
                .accepted_from("b", "t")
                .request_from("b", "t")
                .payload_from("b", "t", 1)
                .tampered_payload_from("b", "t", 2)
                .terminated_from("b", "t");
            for event in script {
                apply(&mut s, event);
            }
            s
        };
        let first = run();
        let second = run();
        assert_eq!(first.received_snapshot(), second.received_snapshot());
        assert_eq!(
            sorted_pairs(
                first
                    .upstream_snapshot()
                    .into_iter()
                    .map(|(p, t, _)| (p, t))
                    .collect()
            ),
            sorted_pairs(
                second
                    .upstream_snapshot()
                    .into_iter()
                    .map(|(p, t, _)| (p, t))
                    .collect()
            ),
        );
        assert_eq!(
            sorted_pairs(first.downstream_snapshot()),
            sorted_pairs(second.downstream_snapshot()),
        );
    }

    // US5-AS2/AS3 / SC-006: a request to an absent peer stays AwaitingAccept
    // indefinitely and admits nothing; SC-007: self is never dialed.
    #[test]
    fn stuck_awaiting_accept_admits_nothing_and_self_never_dialed() {
        let t = topic("t");
        let mut state = node_state("self", HashSet::from([t.clone()]));
        apply(&mut state, reg_open("t"));
        apply(&mut state, membership_joined("absent", ["t"]));

        apply(&mut state, Event::ConnectionSetup); // dials the absent peer
        assert_eq!(
            upstream_state(&state, "absent", "t"),
            Some(UpstreamState::AwaitingAccept),
        );

        // No Accepted arrives — a payload from the pending peer is not admitted.
        let e = apply(&mut state, payload_from("absent", "t", 1));
        assert!(e.is_empty());
        assert!(
            state.received_snapshot().is_empty(),
            "a pending (AwaitingAccept) connection admits nothing",
        );

        // It stays pending across a recurring setup (re-dialed, never activated).
        apply(&mut state, Event::ConnectionSetup);
        assert_eq!(
            upstream_state(&state, "absent", "t"),
            Some(UpstreamState::AwaitingAccept),
        );

        // SC-007: even with self in membership/candidates, self is never dialed.
        apply(&mut state, membership_joined("self", ["t"]));
        apply(&mut state, Event::ConnectionSetup);
        assert_eq!(upstream_state(&state, "self", "t"), None);
    }

    // ---- T003: publish + first-hop fan-out (US1, FR-001..005/007/011/016) -----

    /// Sort a peer list for order-insensitive assertions (fan-out target order
    /// is unspecified).
    fn sorted_peers(mut v: Vec<PeerId>) -> Vec<PeerId> {
        v.sort_by_key(ToString::to_string);
        v
    }

    /// The `(to, signed)` of every signed-payload `Send` effect — the fan-out
    /// forwards (distinct from the control-message sends `request_sends` etc.
    /// pick out).
    fn signed_sends(effects: &[Effect]) -> Vec<(PeerId, SignedMessage)> {
        effects
            .iter()
            .filter_map(|effect| match effect {
                Effect::Send {
                    to,
                    message: Message::Dissemination(sm),
                } => Some((to.clone(), sm.clone())),
                _ => None,
            })
            .collect()
    }

    /// The inner [`SignedMessage`] of a `signed_ping`/`tampered_ping` build.
    fn signed(message: Message) -> SignedMessage {
        let Message::Dissemination(sm) = message else {
            unreachable!("ping builders always yield Message::Dissemination");
        };
        sm
    }

    // US1-AS1 / FR-001..004,007,011,014,016: a valid publish records the message
    // with `Origin::Local` and fans it out verbatim to every downstream on the
    // topic (one `Effect::Send` each, order-insensitive).
    #[test]
    fn publish_records_local_and_fans_out_to_downstream() {
        let t1 = topic("t1");
        let mut state = state_subscribed(vec![t1.clone()]);
        with_downstream(&mut state, "a", "t1");
        with_downstream(&mut state, "b", "t1");
        let sm = signed(signed_ping(&signer(), t1, 1));

        let effects = handle_publish(&mut state, sm.clone());

        let snap = state.received_snapshot();
        assert_eq!(snap.len(), 1, "published message recorded");
        assert_eq!(snap[0].origin, Origin::Local, "local origin");
        assert_eq!(snap[0].message, Message::Dissemination(sm.clone()));

        let sends = signed_sends(&effects);
        assert_eq!(
            sorted_peers(sends.iter().map(|(p, _)| p.clone()).collect()),
            vec![peer("a"), peer("b")],
            "one forward per downstream on the topic",
        );
        for (_, forwarded) in &sends {
            assert_eq!(*forwarded, sm, "forward is verbatim (no re-sign)");
        }
    }

    // US1 / FR-016: a publish with no downstream is recorded but produces no
    // effects (recording still occurs).
    #[test]
    fn publish_with_no_downstream_records_without_effects() {
        let t1 = topic("t1");
        let mut state = state_subscribed(vec![t1.clone()]);
        let sm = signed(signed_ping(&signer(), t1, 1));

        let effects = handle_publish(&mut state, sm);

        assert!(effects.is_empty(), "no downstream → no forwards");
        let snap = state.received_snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].origin, Origin::Local);
    }

    // US1-AS? / FR-005: proxy/injection — a validly-signed, authorized message
    // from a publisher other than the node itself is accepted (publisher_id need
    // not be self).
    #[test]
    fn publish_accepts_proxy_publisher_not_self() {
        let t1 = topic("t1");
        let mut state = state_subscribed(vec![t1.clone()]);
        // A publisher whose key is not the node's own ("self") identity.
        let other = signer_seeded([42u8; 32]);
        let sm = signed(signed_ping(&other, t1, 1));

        let effects = handle_publish(&mut state, sm);

        assert!(effects.is_empty(), "no downstream");
        let snap = state.received_snapshot();
        assert_eq!(snap.len(), 1, "proxy publish accepted (publisher != self)");
        assert_eq!(snap[0].origin, Origin::Local);
    }

    // US1-AS2..4 / FR-002,003: each failed-check publish is a plain drop — no
    // record, no effects, and (the publish-path invariant) NO severance, even
    // with downstream present.
    #[test]
    fn publish_drops_failed_checks_without_record_effects_or_severance() {
        let s = signer();

        // (a) topic not subscribed — downstream on the topic to prove the drop
        // precedes any fan-out.
        let mut state = state_subscribed(vec![topic("t1")]);
        with_downstream(&mut state, "a", "t2");
        let effects = handle_publish(&mut state, signed(signed_ping(&s, topic("t2"), 1)));
        assert!(effects.is_empty(), "not subscribed → no record, no fan-out");
        assert!(state.received_snapshot().is_empty());

        // (b) restricted topic, publisher not authorized.
        let weather = topic("weather");
        let mut state = node_state("self", HashSet::from([weather.clone()]));
        apply(
            &mut state,
            Event::TopicRegistryUpdate(TopicRegistryEvent::Registered {
                topic: weather.clone(),
                publishers: BTreeSet::from([signer_seeded([9u8; 32]).public_key()]),
            }),
        );
        let effects = handle_publish(&mut state, signed(signed_ping(&s, weather, 1)));
        assert!(effects.is_empty(), "unauthorized → dropped");
        assert!(state.received_snapshot().is_empty());

        // (c) invalid signature — a plain drop on the publish path (no upstream
        // to sever, and the publish path never severs).
        let mut state = state_subscribed(vec![topic("t1")]);
        with_downstream(&mut state, "a", "t1");
        let effects = handle_publish(&mut state, signed(tampered_ping(&s, topic("t1"), 1)));
        assert!(
            misbehaved(&effects).is_empty(),
            "invalid-signature publish never severs",
        );
        assert!(effects.is_empty(), "no record, no fan-out");
        assert!(state.received_snapshot().is_empty());
    }

    // ---- T007: receive-path fan-out + split-horizon (US2, FR-006/007/009) -----

    // US2-AS1/AS2/AS5 / FR-006/007/009: a recorded received message is fanned out
    // to every downstream on the topic EXCEPT the delivering peer (split-horizon),
    // verbatim, and is recorded with `Origin::Peer(deliverer)`.
    #[test]
    fn received_message_fans_out_to_downstream_excluding_deliverer() {
        let t1 = topic("t1");
        let mut state = state_subscribed(vec![t1.clone()]);
        // b delivers over an Active upstream (the gate). Downstream on t1: b (the
        // deliverer — must be excluded), plus c and d (the forward targets).
        with_active_upstream(&mut state, "b", "t1");
        with_downstream(&mut state, "b", "t1");
        with_downstream(&mut state, "c", "t1");
        with_downstream(&mut state, "d", "t1");
        let sm = signed(signed_ping(&signer(), t1.clone(), 1));

        let effects = apply(
            &mut state,
            Event::MessageReceived {
                from: peer("b"),
                message: Message::Dissemination(sm.clone()),
            },
        );

        // Recorded once, attributed to the delivering peer (US2-AS1).
        let snap = state.received_snapshot();
        assert_eq!(snap.len(), 1, "received message recorded once");
        assert_eq!(
            snap[0].origin,
            Origin::Peer(peer("b")),
            "origin is the delivering peer",
        );

        // Fanned to c and d only — never back to the deliverer b (split-horizon).
        let sends = signed_sends(&effects);
        assert_eq!(
            sorted_peers(sends.iter().map(|(p, _)| p.clone()).collect()),
            vec![peer("c"), peer("d")],
            "forwarded to the other downstream, never back to the deliverer",
        );
        // Verbatim — each forward equals the received message (US2-AS5).
        for (_, forwarded) in &sends {
            assert_eq!(*forwarded, sm, "forward is verbatim (signature unchanged)");
        }
    }

    // US2-AS3 / FR-009: when the delivering peer is the node's ONLY downstream on
    // the topic, split-horizon leaves no targets — recorded, no forwards.
    #[test]
    fn received_message_sole_downstream_is_deliverer_yields_no_forward() {
        let t1 = topic("t1");
        let mut state = state_subscribed(vec![t1.clone()]);
        with_active_upstream(&mut state, "b", "t1");
        with_downstream(&mut state, "b", "t1"); // b is the only downstream
        let sm = signed(signed_ping(&signer(), t1.clone(), 1));

        let effects = apply(
            &mut state,
            Event::MessageReceived {
                from: peer("b"),
                message: Message::Dissemination(sm),
            },
        );

        assert!(
            signed_sends(&effects).is_empty(),
            "sole downstream is the deliverer → no forward",
        );
        let snap = state.received_snapshot();
        assert_eq!(snap.len(), 1, "still recorded");
        assert_eq!(snap[0].origin, Origin::Peer(peer("b")));
    }

    // ---- T010: duplicate suppression (US3, FR-012/013/015) --------------------

    // US3-AS1 / FR-012: an already-seen message redelivered over an Active
    // upstream is dropped (`duplicate`) — not recorded a second time and not
    // fanned out again.
    #[test]
    fn already_seen_received_message_is_dropped_not_refanned() {
        let t1 = topic("t1");
        let mut state = state_subscribed(vec![t1.clone()]);
        with_active_upstream(&mut state, "b", "t1");
        with_downstream(&mut state, "c", "t1"); // a downstream, to prove no re-fan
        let sm = signed(signed_ping(&signer(), t1.clone(), 1));

        // First delivery: recorded and fanned to c.
        let first = apply(
            &mut state,
            Event::MessageReceived {
                from: peer("b"),
                message: Message::Dissemination(sm.clone()),
            },
        );
        assert_eq!(
            state.received_snapshot().len(),
            1,
            "first delivery recorded"
        );
        assert_eq!(signed_sends(&first).len(), 1, "first delivery fans to c");

        // Identical redelivery over the same Active upstream: dropped duplicate.
        let second = apply(
            &mut state,
            Event::MessageReceived {
                from: peer("b"),
                message: Message::Dissemination(sm),
            },
        );
        assert!(
            second.is_empty(),
            "duplicate produces no effects (no re-fan)"
        );
        assert_eq!(
            state.received_snapshot().len(),
            1,
            "duplicate not recorded a second time",
        );
    }

    // US3 / FR-012, contracts §1.6: a second publish of identical content is
    // dropped `duplicate` — confirming the publish path inserts into `seen`.
    #[test]
    fn republish_identical_content_is_dropped_duplicate() {
        let t1 = topic("t1");
        let mut state = state_subscribed(vec![t1.clone()]);
        with_downstream(&mut state, "a", "t1");
        let sm = signed(signed_ping(&signer(), t1.clone(), 1));

        let first = handle_publish(&mut state, sm.clone());
        assert_eq!(state.received_snapshot().len(), 1, "first publish recorded");
        assert_eq!(signed_sends(&first).len(), 1, "first publish fans to a");

        let second = handle_publish(&mut state, sm);
        assert!(
            second.is_empty(),
            "re-publishing identical content is a duplicate drop",
        );
        assert_eq!(
            state.received_snapshot().len(),
            1,
            "duplicate publish not recorded again",
        );
    }

    // US3-AS2 / FR-015: dedup spans both paths — a message the node published
    // (and thereby seen-marked) is dropped if a peer later relays it back.
    #[test]
    fn published_message_relayed_back_is_dropped_duplicate() {
        let t1 = topic("t1");
        let mut state = state_subscribed(vec![t1.clone()]);
        with_active_upstream(&mut state, "b", "t1"); // b can deliver to us
        let sm = signed(signed_ping(&signer(), t1.clone(), 1));

        // Publish: recorded locally and seen-marked.
        handle_publish(&mut state, sm.clone());
        assert_eq!(state.received_snapshot().len(), 1, "publish recorded");

        // b relays the same content back over the Active upstream → duplicate.
        let relayed = apply(
            &mut state,
            Event::MessageReceived {
                from: peer("b"),
                message: Message::Dissemination(sm),
            },
        );
        assert!(relayed.is_empty(), "relayed-back copy produces no effects");
        assert_eq!(
            state.received_snapshot().len(),
            1,
            "the relayed-back copy is suppressed (FR-015)",
        );
    }

    // US3-AS4 / FR-013: no poisoning — an invalid-signature PUBLISH whose `plain`
    // hashes identically to a genuine message is a plain drop at verification
    // (the dedup gate sits *after* verification, so it is unreached and never
    // seen-marks). The genuine message — same content hash — is still recorded.
    #[test]
    fn invalid_signature_publish_does_not_poison_seen() {
        let t1 = topic("t1");
        let s = signer();
        let mut state = state_subscribed(vec![t1.clone()]);

        let genuine = signed(signed_ping(&s, t1.clone(), 1));
        // Same `plain` (so the same content hash) but a signature that does not
        // verify under the publisher's key — produced by a different signer.
        let impostor = signer_seeded([99u8; 32]);
        let forged = SignedMessage {
            plain: genuine.plain.clone(),
            signature: impostor.sign(&genuine.plain.signed_bytes()),
        };
        assert_eq!(
            MessageHash::of(&forged.plain),
            MessageHash::of(&genuine.plain),
            "the forged copy hashes identically to the genuine message",
        );

        // The forged publish drops at verification (publish never severs) and
        // must NOT seen-mark the shared hash.
        let dropped = handle_publish(&mut state, forged);
        assert!(dropped.is_empty(), "forged publish produces no effects");
        assert!(
            state.received_snapshot().is_empty(),
            "forged publish not recorded",
        );

        // The genuine message — identical content hash — is still recorded: the
        // failed verification did not pre-seed `seen`.
        handle_publish(&mut state, genuine);
        assert_eq!(
            state.received_snapshot().len(),
            1,
            "genuine message recorded; the seen-set was not poisoned",
        );
        assert_eq!(state.received_snapshot()[0].origin, Origin::Local);
    }
}
