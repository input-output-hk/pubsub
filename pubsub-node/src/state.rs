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

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use crate::connection_state::UpstreamState;
use crate::crypto::{MessageHash, Signer, Verifier};
use crate::event::Event;
use crate::message::{
    ConnectionAction, ConnectionMessage, Message, PlainConnection, PlainMessage, SignedMessage,
};
use crate::peer::PeerId;
use crate::received::{Origin, ReceivedDelivery};
use crate::strategies::acceptance::{Admission, ConnectionAcceptanceStrategy};
use crate::strategies::connection::ConnectionStrategy;
use crate::strategies::fanout::FanoutStrategy;
use crate::strategies::view::NodeView;
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
    subscriptions: BTreeSet<TopicId>,
    received: Vec<ReceivedDelivery>,
    verifier: Arc<dyn Verifier>,
    /// Per-topic candidate peers, folded from the subscription-registry stream
    /// (`Event::MembershipUpdate`). The node's own id is never present. This is
    /// the topic-derived peer set, distinct from the shell's static config
    /// `peers` bootstrap list (`IMPLEMENTATION_NOTES` N-007).
    candidates: BTreeMap<TopicId, BTreeSet<PeerId>>,
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
    /// The current heartbeat **interval** (offset from genesis, 0-based) — the
    /// round counter that feeds the verifiable edge predicate (ADR 0030). Folded
    /// from the last `Heartbeat` event (default 0); the acceptor verifies inbound
    /// requests against it, so it is event-derived state, not a strategy field.
    interval: u64,
    /// The node's signing identity: signs the control messages it emits
    /// (`Request`/`Accepted`/`Terminated`). Rides along as an immutable service
    /// handle beside the verifier; the transition signs inside the pure core so
    /// each `Effect::Send` carries a complete signed message (FR-011).
    signer: Arc<dyn Signer>,
    /// The connection-selection policy consulted on a `Heartbeat`, beside the
    /// verifier (the immutable service-handle slot). The transition reads it
    /// from the `Heartbeat` arm (ADR 0018/0030).
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
        subscriptions: BTreeSet<TopicId>,
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
            candidates: BTreeMap::new(),
            registered_topics: HashMap::new(),
            upstream: HashMap::new(),
            downstream: HashSet::new(),
            interval: 0,
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
        Event::Heartbeat { interval } => handle_heartbeat(state, interval),
        Event::Shutdown => handle_shutdown(state),
    }
}

/// Transition for the connection **heartbeat** (ADR 0030).
///
/// Stores the carried `interval` (so the acceptor verifies inbound requests
/// against the current round), then consults the node's connection-selection
/// strategy for the expected upstream set and applies it as the diff: dial
/// everything expected that is not already `Active`. A pair not held gains an
/// `AwaitingAccept` entry and a `Request`; a pair still at `AwaitingAccept` keeps
/// its entry and is re-requested; an `Active` pair is left alone. Expected-set
/// membership never removes anything (v1 is single-interval; cross-interval
/// rotation/teardown is deferred). The strategy reads the membership-derived
/// `subscriptions` field and the current interval (005 FR-006).
fn handle_heartbeat(state: &mut NodeState, interval: u64) -> Vec<Effect> {
    state.interval = interval;
    let view = NodeView {
        subscriptions: &state.subscriptions,
        candidates: &state.candidates,
        downstream: &state.downstream,
        interval,
    };
    let expected = state.connection_strategy.expected_upstream(&view);
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
    // v1 fires a single heartbeat (interval 0) on the readiness edge; periodic
    // heartbeats are a later feature (ADR 0030).
    handle_heartbeat(state, 0)
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
                let mut subscriptions = BTreeSet::new();
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
        ConnectionAction::Rejected { topic } => {
            handle_connection_rejected(state, &plain.emitter, &topic)
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
    let view = NodeView {
        subscriptions: &state.subscriptions,
        candidates: &state.candidates,
        downstream: &state.downstream,
        interval: state.interval,
    };
    let admission = state.acceptance_strategy.admit(&emitter, &topic, &view);
    match admission {
        Admission::Accept => {
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
        // Both silent-drop refusals: no reply, leaking nothing to the requester
        // (a non-member, or an adversary whose edge predicate does not hold this
        // interval). Distinct log causes only (ADR 0025).
        Admission::RejectMembership | Admission::RejectIllegitimate => {
            let cause = if admission == Admission::RejectMembership {
                "membership_validation_failed"
            } else {
                "illegitimate_request"
            };
            tracing::info!(
                target: "pubsub_node::node",
                event = "message_dropped",
                cause,
                self_id = %state.self_id,
                emitter = %emitter,
                topic = %topic,
            );
            Vec::new()
        }
        Admission::RejectOverCapacity => {
            // Over the per-topic cap: drop without recording downstream, but send
            // an explicit `Rejected` so the dialer drops its pending upstream
            // (ADR 0025). Not misbehaviour — no severance.
            tracing::info!(
                target: "pubsub_node::node",
                event = "message_dropped",
                cause = "downstream_capacity_reached",
                self_id = %state.self_id,
                emitter = %emitter,
                topic = %topic,
            );
            let message = signed_connection(
                &state.self_id,
                state.signer.as_ref(),
                ConnectionAction::Rejected { topic },
            );
            vec![Effect::Send {
                to: emitter,
                message,
            }]
        }
    }
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

/// Transition for a verified `Rejected` from `emitter` on `topic` — the peer
/// refused this node's dial for over-capacity (feature 005, ADR 0025).
///
/// Removes the matching `AwaitingAccept` upstream so the dialer stops waiting on
/// an `Accepted` that will never come; that is the **only** handling. There is no
/// retry and no back-fill: the realized upstream degree may settle below target,
/// and re-forming connections is left to the future heartbeat/reshuffle layer
/// (retry/back-fill is a separate strategy family — see `IMPLEMENTATION_NOTES`).
/// A `Rejected` with no matching pending entry (absent, or already `Active`) is
/// dropped and changes nothing. A rejection is never treated as misbehaviour.
fn handle_connection_rejected(
    state: &mut NodeState,
    emitter: &PeerId,
    topic: &TopicId,
) -> Vec<Effect> {
    let key = (emitter.clone(), topic.clone());
    if matches!(
        state.upstream.get(&key),
        Some(UpstreamState::AwaitingAccept)
    ) {
        state.upstream.remove(&key);
        return Vec::new();
    }
    tracing::info!(
        target: "pubsub_node::node",
        event = "message_dropped",
        cause = "unsolicited_reject",
        self_id = %state.self_id,
        emitter = %emitter,
        topic = %topic,
    );
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
mod tests;
