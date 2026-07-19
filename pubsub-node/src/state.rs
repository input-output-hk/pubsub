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

use crate::connection_state::{LinkKey, LinkKind, LinkState};
use crate::crypto::{MessageHash, Signer, Verifier};
use crate::event::Event;
use crate::message::{
    ConnectionAction, ConnectionMessage, HandshakeKind, Message, PlainConnection, PlainMessage,
    SignedMessage,
};
use crate::peer::PeerId;
use crate::received::{Origin, ReceivedDelivery};
use crate::strategies::acceptance::ConnectionAcceptanceStrategy;
use crate::strategies::config::NodeStrategies;
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
/// shell concerns (the network handle) stay on the node. Peer or registry-derived data joins this struct when a
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
    /// (`Event::MembershipUpdate`). The node's own id is never present
    /// (`IMPLEMENTATION_NOTES` N-007, closed by the config-peers removal).
    candidates: BTreeMap<TopicId, BTreeSet<PeerId>>,
    /// Registered topics → their authorized publisher keys (empty ⇒ open),
    /// folded from the topic-registry stream (`Event::TopicRegistryUpdate`).
    /// Written only by `handle_topic_registry_update`. The node's **effective**
    /// subscription set — its message accept-filter — is `subscriptions`
    /// intersected with the keys here; a subscribed topic absent here is not yet
    /// (or no longer) a legitimate topic, so its traffic is dropped.
    registered_topics: HashMap<TopicId, TopicEntry>,
    /// **Upstream** links — peers this node receives from — keyed by
    /// [`LinkKey`]. Per kind: `Relay` entries are the node's own pull dials
    /// (full [`LinkState`] lifecycle); `Publisher` entries are accepted inbound
    /// publisher links (inserted `Active` — presence means accepted). Written
    /// by the connection transitions.
    upstream: BTreeMap<LinkKey, LinkState>,
    /// **Downstream** links — peers this node sends to — keyed by [`LinkKey`].
    /// Per kind: `Relay` entries are accepted relay peers (inserted `Active`);
    /// `Publisher` entries are the node's own initiation dials (full
    /// [`LinkState`] lifecycle). Written by the connection transitions.
    downstream: BTreeMap<LinkKey, LinkState>,
    /// The current **epoch nonce** — the randomness context the verifiable edge
    /// predicate hashes (ADR 0031). Folded from the last `Epoch` event (default
    /// 0); the acceptor verifies inbound requests against it, so it is
    /// event-derived state, not a strategy field. Deliberately decoupled from
    /// the `Heartbeat` dial tick: heartbeats within one epoch re-dial the same
    /// expected set.
    epoch_nonce: u64,
    /// The node's signing identity: signs the control messages it emits
    /// (`Request`/`Accepted`/`Terminated`). Rides along as an immutable service
    /// handle beside the verifier; the transition signs inside the pure core so
    /// each `Effect::Send` carries a complete signed message (FR-011).
    signer: Arc<dyn Signer>,
    /// The relay-link selection policy consulted on a `Heartbeat`, beside the
    /// verifier (the immutable service-handle slot). The transition reads it
    /// from the `Heartbeat` arm (ADR 0018/0030).
    connection_strategy: Arc<dyn ConnectionStrategy>,
    /// The fan-out policy consulted at the record point to choose which
    /// downstream peers receive a forward of a recorded message. The deliberate
    /// twin of `connection_strategy`; the v1 implementor is `ForwardToRelays` (ADR 0021).
    fanout_strategy: Arc<dyn FanoutStrategy>,
    /// The relay-link acceptance policy consulted on a verified relay `Request`
    /// to decide whether to accept the emitter as downstream on the topic. The
    /// inbound mirror of `connection_strategy` (ADR 0023).
    acceptance_strategy: Arc<dyn ConnectionAcceptanceStrategy>,
    /// The publisher-link selection policy — a second instance of the same
    /// seam, dialing the node's standing initiation targets on the heartbeat.
    /// `None` (the default) disables publisher links entirely: no dials.
    publisher_strategy: Option<Arc<dyn ConnectionStrategy>>,
    /// The publisher-link acceptance policy — the publisher counterpart of
    /// `acceptance_strategy`, admitting into `upstream` × `Publisher` with its
    /// own disjoint capacity. `None` (the default) silently drops every inbound
    /// publisher request.
    publisher_acceptance: Option<Arc<dyn ConnectionAcceptanceStrategy>>,
    /// Whether this node's relay links use the **symmetric** (bidirectional)
    /// handshake — M4 (ADR 0034). When set, the relay dial pass speaks the
    /// symmetric vocabulary, inbound symmetric requests are admitted (one
    /// accept decision records the link in both directions), and severing a
    /// relay link removes its mirror — on a symmetric node every relay link
    /// is bidirectional by construction. `false` (every other model): inbound
    /// symmetric handshakes are dropped outright.
    symmetric_edges: bool,
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
    /// Construct the state value from already-parsed inputs. `genesis` is the
    /// initial epoch nonce (the epoch-0 stand-in for the chain-anchored beacon);
    /// an `Epoch` event replaces it. `strategies` carries the four link seams
    /// (the publisher pair optional — `None` disables publisher links).
    // Mirrors `Node::new`'s specified construction contract (see the note
    // there); a config/builder struct is the natural refactor if it grows.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        self_id: PeerId,
        subscriptions: BTreeSet<TopicId>,
        genesis: u64,
        verifier: Arc<dyn Verifier>,
        signer: Arc<dyn Signer>,
        strategies: NodeStrategies,
        fanout_strategy: Arc<dyn FanoutStrategy>,
    ) -> Self {
        Self {
            self_id,
            subscriptions,
            received: Vec::new(),
            verifier,
            candidates: BTreeMap::new(),
            registered_topics: HashMap::new(),
            upstream: BTreeMap::new(),
            downstream: BTreeMap::new(),
            epoch_nonce: genesis,
            signer,
            connection_strategy: strategies.relay_connection,
            fanout_strategy,
            acceptance_strategy: strategies.relay_acceptance,
            publisher_strategy: strategies.publisher_connection,
            publisher_acceptance: strategies.publisher_acceptance,
            symmetric_edges: strategies.symmetric_edges,
            seen: HashSet::new(),
            synced: false,
        }
    }

    /// The read-only view the strategy seams take (ADR 0031) — the one
    /// construction site for [`NodeView`].
    fn view(&self) -> NodeView<'_> {
        NodeView {
            subscriptions: &self.subscriptions,
            candidates: &self.candidates,
            upstream: &self.upstream,
            downstream: &self.downstream,
            epoch_nonce: self.epoch_nonce,
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

    /// Snapshot of the **relay upstream** links — `(peer, topic, state)`
    /// triples the node dialed as message sources, in unspecified order. A
    /// stable clone, unaffected by later events. On a node without publisher
    /// links this is exactly the pre-015 upstream snapshot.
    #[must_use]
    pub(crate) fn upstream_relays(&self) -> Vec<(PeerId, TopicId, LinkState)> {
        self.upstream
            .iter()
            .filter(|(key, _)| key.kind == LinkKind::Relay)
            .map(|(key, state)| (key.peer.clone(), key.topic.clone(), *state))
            .collect()
    }

    /// Snapshot of the **relay downstream** links — `(peer, topic)` pairs the
    /// node accepted as fan-out destinations, in unspecified order. A stable
    /// clone, unaffected by later events.
    #[must_use]
    pub(crate) fn downstream_relays(&self) -> Vec<(PeerId, TopicId)> {
        self.downstream
            .iter()
            .filter(|(key, _)| key.kind == LinkKind::Relay)
            .map(|(key, _)| (key.peer.clone(), key.topic.clone()))
            .collect()
    }

    /// Snapshot of the **publisher upstream** links — `(peer, topic)` pairs
    /// whose inbound initiation links the node accepted, in unspecified order.
    #[must_use]
    pub(crate) fn upstream_publishers(&self) -> Vec<(PeerId, TopicId)> {
        self.upstream
            .iter()
            .filter(|(key, _)| key.kind == LinkKind::Publisher)
            .map(|(key, _)| (key.peer.clone(), key.topic.clone()))
            .collect()
    }

    /// Snapshot of the **publisher downstream** links — `(peer, topic, state)`
    /// triples the node dialed as standing targets for its own publications,
    /// in unspecified order.
    #[must_use]
    pub(crate) fn downstream_publishers(&self) -> Vec<(PeerId, TopicId, LinkState)> {
        self.downstream
            .iter()
            .filter(|(key, _)| key.kind == LinkKind::Publisher)
            .map(|(key, state)| (key.peer.clone(), key.topic.clone(), *state))
            .collect()
    }
}

/// Crate-internal access for the experiments framework (feature
/// `experiments`; never compiled into the default build): direct state writes
/// for the driver's pre-population fast path (016-FR-008/FR-032) and cheap
/// read accessors for driver-owned measurement (016-FR-017 — measurement never
/// reads logs). The writes mirror what the real folds produce; they perform no
/// validation and emit no effects, which is exactly the fast path's contract —
/// the faithful mode exists to assert the equivalence on small populations.
#[cfg(feature = "experiments")]
impl NodeState {
    /// Register `topic` directly — the pre-population counterpart of folding
    /// `TopicRegistryEvent::Registered` (empty `publishers` ⇒ open topic).
    pub(crate) fn prepopulate_registered_topic(
        &mut self,
        topic: TopicId,
        publishers: std::collections::BTreeSet<crate::crypto::PublicKey>,
    ) {
        self.registered_topics
            .insert(topic, TopicEntry::from_publishers(publishers));
    }

    /// Add `topic` to the node's subscription set directly. The caller keeps
    /// the maintained invariant: the topic must already be registered.
    pub(crate) fn prepopulate_subscription(&mut self, topic: TopicId) {
        self.subscriptions.insert(topic);
    }

    /// Install the full candidate set for `topic` directly. The caller keeps
    /// the fold invariants: the node's own id is never in the set, and the
    /// topic is already registered.
    pub(crate) fn prepopulate_candidates(&mut self, topic: TopicId, peers: BTreeSet<PeerId>) {
        self.candidates.insert(topic, peers);
    }

    /// Set the readiness flag directly. Unlike the `Synced` fold this runs no
    /// readiness dial — firing the dial tick is the driver's own phase.
    pub(crate) fn prepopulate_synced(&mut self) {
        self.synced = true;
    }

    /// Install an already-`Active` relay upstream entry directly (the
    /// scripted-topology path: the dial handshake is bypassed entirely).
    pub(crate) fn prepopulate_active_upstream(&mut self, peer: PeerId, topic: TopicId) {
        self.upstream.insert(
            LinkKey::new(topic, peer, LinkKind::Relay),
            LinkState::Active,
        );
    }

    /// Install an `Active` relay downstream entry directly (scripted-topology
    /// path).
    pub(crate) fn prepopulate_downstream(&mut self, peer: PeerId, topic: TopicId) {
        self.downstream.insert(
            LinkKey::new(topic, peer, LinkKind::Relay),
            LinkState::Active,
        );
    }

    /// Whether the content hash is in the duplicate-suppression set — i.e. the
    /// node has accepted a message with this content (published or received).
    pub(crate) fn has_seen(&self, hash: &MessageHash) -> bool {
        self.seen.contains(hash)
    }

    /// The current epoch nonce (single-epoch assertion surface, 016-FR-009).
    pub(crate) fn epoch_nonce(&self) -> u64 {
        self.epoch_nonce
    }

    /// Number of recorded deliveries, without cloning the record list.
    pub(crate) fn received_len(&self) -> usize {
        self.received.len()
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
        /// The topic the severed link was for.
        topic: TopicId,
        /// Which link class was severed (the admitting link's kind).
        kind: LinkKind,
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
        Event::Heartbeat => handle_heartbeat(state),
        Event::Epoch { nonce } => handle_epoch(state, nonce),
        Event::Shutdown => handle_shutdown(state),
    }
}

/// Transition for the connection **heartbeat** — the dial tick, or "round"
/// (ADR 0030/0031).
///
/// Consults the node's connection-selection strategy for the expected upstream
/// set over the current epoch nonce and applies it as the diff: dial everything
/// expected that is not already `Active`. A pair not held gains an
/// `AwaitingAccept` entry and a `Request`; a pair still at `AwaitingAccept` keeps
/// its entry and is re-requested; an `Active` pair is left alone. Expected-set
/// membership never removes anything (cross-epoch rotation/teardown is
/// deferred). Within one epoch the expected set is stable, so a repeated
/// heartbeat is a pure retry pass. The strategy reads the membership-derived
/// `subscriptions` field and the current epoch nonce (005 FR-006).
///
/// Gated on readiness, symmetric to the inbound-request gate (ADR 0031): a
/// dial pass over a partially-folded candidate view floors B to 1 and dials
/// everyone folded so far — synced acceptors verify those dials under the full
/// view's larger B and silently drop them, each a stranded `AwaitingAccept`
/// entry. `handle_synced` flips the flag before its readiness dial, so the
/// production path is unaffected; only an injected pre-sync heartbeat is
/// dropped.
fn handle_heartbeat(state: &mut NodeState) -> Vec<Effect> {
    if !state.synced {
        tracing::info!(
            target: "pubsub_node::node",
            event = "heartbeat_dropped",
            cause = "not_synced",
            self_id = %state.self_id,
        );
        return Vec::new();
    }
    let view = state.view();
    let expected_relay = state.connection_strategy.expected_links(&view);
    // The publisher pass runs unconditionally whenever a publisher strategy is
    // configured — its picks never depend on the relay topology (M3: standing
    // initiation links are always established).
    let expected_publish = state
        .publisher_strategy
        .as_ref()
        .map(|strategy| strategy.expected_links(&view));
    // Clone the immutable bits the request builder needs so the loop can mutate
    // the link maps without aliasing the whole struct.
    let self_id = state.self_id.clone();
    let signer = Arc::clone(&state.signer);
    // A symmetric node's relay picks are dialed under the symmetric
    // vocabulary — one handshake establishes the link in both directions
    // (ADR 0034); the stored entries stay relay-class either way.
    let relay_handshake = if state.symmetric_edges {
        HandshakeKind::Symmetric
    } else {
        HandshakeKind::Relay
    };

    let mut effects = Vec::new();
    // Relay pass: dials land in `upstream` (the node will receive).
    for (peer, topic) in expected_relay {
        let key = LinkKey::new(topic.clone(), peer.clone(), LinkKind::Relay);
        match state.upstream.get(&key).copied() {
            Some(LinkState::Active) => continue,
            Some(LinkState::AwaitingAccept) => {}
            None => {
                state.upstream.insert(key, LinkState::AwaitingAccept);
            }
        }
        let message = signed_connection(
            &self_id,
            signer.as_ref(),
            relay_handshake,
            ConnectionAction::Request { topic },
        );
        effects.push(Effect::Send { to: peer, message });
    }
    // Publisher pass: dials land in `downstream` (the node will send its own
    // publications). Same diff-and-retry semantics as the relay pass.
    for (peer, topic) in expected_publish.into_iter().flatten() {
        let key = LinkKey::new(topic.clone(), peer.clone(), LinkKind::Publisher);
        match state.downstream.get(&key).copied() {
            Some(LinkState::Active) => continue,
            Some(LinkState::AwaitingAccept) => {}
            None => {
                state.downstream.insert(key, LinkState::AwaitingAccept);
            }
        }
        let message = signed_connection(
            &self_id,
            signer.as_ref(),
            HandshakeKind::Publisher,
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
    // v1 fires a single heartbeat on the readiness edge; periodic heartbeats
    // are a later feature (ADR 0030/0031).
    handle_heartbeat(state)
}

/// Transition for a new **epoch** (ADR 0031): fold the carried nonce — the
/// randomness context the verifiable edge predicate hashes on both seams.
///
/// Produces no effects: whether to re-dial under the new nonce is the driver's
/// choice, via a following `Heartbeat`. Keeping the fold and the dial separate
/// is what lets a driver run a two-phase barrier (advance every node's epoch,
/// then dial), narrowing the cross-node verification skew an epoch change opens.
fn handle_epoch(state: &mut NodeState, nonce: u64) -> Vec<Effect> {
    state.epoch_nonce = nonce;
    Vec::new()
}

/// Build a control message signed by the node's own signer, with the node's
/// own id as the carried emitter, wrapped in the `kind` handshake's message
/// variant (the signature binds emitter, action, topic, and handshake kind).
fn signed_connection(
    self_id: &PeerId,
    signer: &dyn Signer,
    kind: HandshakeKind,
    action: ConnectionAction,
) -> Message {
    let plain = PlainConnection {
        emitter: self_id.clone(),
        action,
    };
    let signature = signer.sign(&plain.signed_bytes(kind));
    Message::connection(kind, ConnectionMessage { plain, signature })
}

/// Transition for the graceful-shutdown trigger.
///
/// Clears both link collections and emits one `Terminated` notice per held
/// **link** — both directions, both kinds, any state, including
/// `AwaitingAccept` dials. The notices are deduplicated by [`LinkKey`]: a link
/// held in both directions (every symmetric link, by construction) is one
/// link and gets one notice — the counterpart's teardown removes both halves
/// either way. A peer holding links of both kinds still gets one notice per
/// kind (distinct keys).
fn handle_shutdown(state: &mut NodeState) -> Vec<Effect> {
    let self_id = state.self_id.clone();
    let signer = Arc::clone(&state.signer);
    // Each notice speaks the vocabulary its link was established under: on a
    // symmetric node the relay-class links are symmetric-handshake links.
    let symmetric = state.symmetric_edges;
    let terminate = |key: LinkKey| {
        let handshake = match key.kind {
            LinkKind::Relay if symmetric => HandshakeKind::Symmetric,
            LinkKind::Relay => HandshakeKind::Relay,
            LinkKind::Publisher => HandshakeKind::Publisher,
        };
        Effect::Send {
            to: key.peer,
            message: signed_connection(
                &self_id,
                signer.as_ref(),
                handshake,
                ConnectionAction::Terminated { topic: key.topic },
            ),
        }
    };

    let keys: BTreeSet<LinkKey> = state
        .upstream
        .keys()
        .chain(state.downstream.keys())
        .cloned()
        .collect();
    let effects: Vec<Effect> = keys.into_iter().map(terminate).collect();

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
            // keyed on it — subscriptions, candidates, and both link collections
            // (both kinds) — together, in this one fold under the lock. No
            // partial state is observable, and no link outlives the topic's
            // legitimacy.
            state.registered_topics.remove(&topic);
            state.subscriptions.remove(&topic);
            state.candidates.remove(&topic);
            state.upstream.retain(|key, _| key.topic != topic);
            state.downstream.retain(|key, _| key.topic != topic);
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

/// Transition for an inbound network message: dispatches per message
/// vocabulary — the connection variants route straight to their handshake's
/// handler module (ADR 0034), so no handler recovers the handshake kind by
/// testing a field mid-flight.
fn handle_message_received(state: &mut NodeState, from: PeerId, message: Message) -> Vec<Effect> {
    tracing::debug!(
        target: "pubsub_node::node",
        from = %from,
        "recv",
    );

    match message {
        Message::Dissemination(signed) => handle_dissemination(state, from, signed),
        Message::RelayConnection(connection) => handlers::relay::handle(state, connection),
        Message::PublisherConnection(connection) => handlers::publisher::handle(state, connection),
        Message::SymmetricConnection(connection) => handlers::symmetric::handle(state, connection),
    }
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
    origin: &Origin,
    exclude: Option<&PeerId>,
) -> Vec<Effect> {
    state
        .fanout_strategy
        .targets(topic, &state.downstream, origin, exclude)
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
    let effects = fanout(state, &topic, &signed, &origin, exclude);
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
/// Records the delivery when the delivering peer holds an **admitting link**
/// for the message's topic — any `Active` upstream entry, either kind — its
/// topic is subscribed **and** a registered (legitimate) topic, its publisher
/// is authorized, and its signature verifies; otherwise the message is dropped
/// (with an info-level `message_dropped` event carrying the cause). A recorded
/// message is then fanned out per the fan-out strategy, excluding the
/// deliverer (split-horizon) — the same record-and-forward tail the publish
/// path uses.
// The link gate is the FIRST check (keyed on the delivering peer — a payload
// carries a publisher identity, not the sender's); the pre-existing chain
// runs unchanged after it — subscribed?, registered?, authorized?, signature?
// (ADR 0016/0032). A signature failure past every earlier check severs the
// ADMITTING link (ADR 0032 §5); the fan-out happens only past the record
// point.
fn handle_dissemination(state: &mut NodeState, from: PeerId, signed: SignedMessage) -> Vec<Effect> {
    // The link gate is kind-agnostic: any Active upstream entry held with the
    // delivering peer for the message's topic admits — the only restriction a
    // receiver can soundly enforce is checkable from the signed bytes alone,
    // and a link's kind restricts what its holder SENDS (M3's exclusivity is
    // the sender-side fan-out policy), not what a receiver admits. The key
    // that admitted the message is remembered: a signature failure past every
    // later check severs exactly that link.
    let admitting_key = [LinkKind::Relay, LinkKind::Publisher]
        .into_iter()
        .map(|kind| LinkKey::new(signed.plain.topic.clone(), from.clone(), kind))
        .find(|key| matches!(state.upstream.get(key), Some(LinkState::Active)));
    let Some(admitting_key) = admitting_key else {
        tracing::info!(
            target: "pubsub_node::node",
            event = "message_dropped",
            cause = "not_connected",
            self_id = %state.self_id,
            from = %from,
            topic = %signed.plain.topic,
        );
        return Vec::new();
    };

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
        // Reaching signature verification means the link gate, subscription,
        // registration, and authorization checks all passed — so a failure
        // here, over the admitting link, is misbehavior. Sever silently: remove
        // the entry that admitted the message and raise the misbehavior signal
        // (the executor logs `connection_severed`); no Terminated is sent.
        let topic = signed.plain.topic.clone();
        let kind = admitting_key.kind;
        state.upstream.remove(&admitting_key);
        // Teardown atomicity of the symmetric establishment protocol: on a
        // symmetric node every relay link is bidirectional by construction,
        // so severing the admitting half removes its mirror too (ADR 0034).
        if state.symmetric_edges && kind == LinkKind::Relay {
            state.downstream.remove(&admitting_key);
        }
        return vec![Effect::Misbehaved {
            peer: from,
            topic,
            kind,
            cause: "invalid_signature",
        }];
    }

    // Record the delivery (origin = the delivering peer) and fan it out to the
    // node's other downstream on the topic, excluding the deliverer
    // (split-horizon). The shared record-and-forward tail with the publish path
    // (R9); the publish path passes `Origin::Local` and no exclusion.
    record_and_fanout(state, signed, Origin::Peer(from.clone()), Some(&from))
}

// Per-handshake connection-control handlers (relay / publisher / symmetric),
// dispatched by message vocabulary from `handle_message_received` (ADR 0034).
mod handlers;

// Synchronous state-machine tests: construct a NodeState, apply scripted
// events, assert on state and returned effects after each step. No async
// runtime, no channels, no tasks; never asserts on log output (constitution:
// logs are operator UX). Covers FR-001/002/003/004/013, US2-AS1..3, and the
// empty-subscription edge case.
#[cfg(test)]
mod tests;
