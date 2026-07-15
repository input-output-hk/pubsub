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

use crate::connection_state::{LinkDirection, LinkRole, LinkState, LinkStore, PublishInAdmission};
use crate::crypto::{MessageHash, Signer, Verifier};
use crate::event::Event;
use crate::message::{
    ConnectionAction, ConnectionMessage, Message, PlainConnection, PlainMessage, SignedMessage,
};
use crate::peer::PeerId;
use crate::received::{Origin, ReceivedDelivery};
use crate::strategies::acceptance::{Admission, ConnectionAcceptanceStrategy};

use crate::strategies::fanout::FanoutStrategy;
use crate::strategies::selection::LinkSelectionStrategy;
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
    /// The unified link store (ADR 0032/0034), cell-structured by role ×
    /// direction. Orientation is derived — `Relay`/`Out` entries are the
    /// dialed message sources (the former `upstream`), `Relay`/`In` the
    /// accepted fan-out destinations (the former `downstream`);
    /// `Publisher`/`Out` entries are the node's standing initiation links and
    /// `Publisher`/`In` the accepted inbound ones. Written by the connection
    /// transitions.
    links: LinkStore,
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
    /// The relay link-selection slot consulted on a `Heartbeat`, beside the
    /// verifier (the immutable service-handle slot). The transition reads it
    /// from the `Heartbeat` arm and tags its picks `Relay`/Out
    /// (ADR 0018/0030/0034).
    relay_selection: Arc<dyn LinkSelectionStrategy>,
    /// The fan-out policy consulted at the record point to choose which
    /// downstream peers receive a forward of a recorded message. The deliberate
    /// twin of `connection_strategy`; the v1 implementor is `ForwardToAll` (ADR 0021).
    fanout_strategy: Arc<dyn FanoutStrategy>,
    /// The inbound-acceptance policy consulted on a verified **relay**
    /// `Request` to decide whether to accept the emitter as a relay fan-out
    /// destination on the topic. The inbound mirror of `connection_strategy`;
    /// the v1 implementor is `AcceptFromAllCandidates` (ADR 0023).
    acceptance_strategy: Arc<dyn ConnectionAcceptanceStrategy>,
    /// The publish link-selection slot consulted on the same `Heartbeat`
    /// after the relay diff: the node's standing initiation links, tagged
    /// `Publisher`/Out — always established, unconditionally (the M3 model,
    /// ADR 0033/0034). The v1 default is `NoLinks`.
    publish_selection: Arc<dyn LinkSelectionStrategy>,
    /// The inbound-acceptance policy consulted on a verified **publish-intent**
    /// `Request` — the same seam contract as `acceptance_strategy`, dispatched
    /// by the request's carried role and instantiated with publish parameters
    /// (ADR 0033).
    publish_acceptance_strategy: Arc<dyn ConnectionAcceptanceStrategy>,
    /// The receive-gate admission policy for inbound initiation links —
    /// `OwnerOnly` (M3, the default) or `AnyVerified` (M5); the receive-side
    /// half of the dissemination-model knob (ADR 0035).
    publish_in_admission: PublishInAdmission,
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
    /// an `Epoch` event replaces it.
    // Mirrors `Node::new`'s specified construction contract (see the note
    // there); a config/builder struct is the natural refactor if it grows.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        self_id: PeerId,
        subscriptions: BTreeSet<TopicId>,
        genesis: u64,
        verifier: Arc<dyn Verifier>,
        signer: Arc<dyn Signer>,
        relay_selection: Arc<dyn LinkSelectionStrategy>,
        fanout_strategy: Arc<dyn FanoutStrategy>,
        acceptance_strategy: Arc<dyn ConnectionAcceptanceStrategy>,
        publish_selection: Arc<dyn LinkSelectionStrategy>,
        publish_acceptance_strategy: Arc<dyn ConnectionAcceptanceStrategy>,
        publish_in_admission: PublishInAdmission,
    ) -> Self {
        Self {
            self_id,
            subscriptions,
            received: Vec::new(),
            verifier,
            candidates: BTreeMap::new(),
            registered_topics: HashMap::new(),
            links: LinkStore::new(),
            epoch_nonce: genesis,
            signer,
            relay_selection,
            fanout_strategy,
            acceptance_strategy,
            publish_selection,
            publish_acceptance_strategy,
            publish_in_admission,
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

    /// Snapshot of the **relay upstream** links — `(peer, topic, state)`
    /// triples (the `Relay`/`Out` view of the link store, preserving the
    /// pre-015 getter semantics). A stable clone, unaffected by later events.
    #[must_use]
    pub(crate) fn upstream_snapshot(&self) -> Vec<(PeerId, TopicId, LinkState)> {
        self.links
            .iter()
            .filter(|(_, _, role, direction, _)| {
                *role == LinkRole::Relay && *direction == LinkDirection::Out
            })
            .map(|(peer, topic, _, _, state)| (peer.clone(), topic.clone(), state))
            .collect()
    }

    /// Snapshot of the **relay downstream** links — `(peer, topic)` pairs (the
    /// `Relay`/`In` view of the link store, preserving the pre-015 getter
    /// semantics). A stable clone, unaffected by later events.
    #[must_use]
    pub(crate) fn downstream_snapshot(&self) -> Vec<(PeerId, TopicId)> {
        self.links
            .iter()
            .filter(|(_, _, role, direction, _)| {
                *role == LinkRole::Relay && *direction == LinkDirection::In
            })
            .map(|(peer, topic, _, _, _)| (peer.clone(), topic.clone()))
            .collect()
    }

    /// Snapshot of the full link store — `(peer, topic, role, direction,
    /// state)` tuples in key order (feature 015). A stable clone, unaffected by
    /// later events.
    #[must_use]
    pub(crate) fn links_snapshot(
        &self,
    ) -> Vec<(PeerId, TopicId, LinkRole, LinkDirection, LinkState)> {
        self.links
            .iter()
            .map(|(peer, topic, role, direction, state)| {
                (peer.clone(), topic.clone(), role, direction, state)
            })
            .collect()
    }

    /// Test-only: record a link directly, bypassing establishment — the
    /// declarative fixture for transitions that *read* the store (fan-out,
    /// receive gate) without exercising the handshake.
    #[cfg(test)]
    pub(crate) fn insert_link_for_test(
        &mut self,
        peer: PeerId,
        topic: TopicId,
        role: LinkRole,
        direction: LinkDirection,
        state: LinkState,
    ) {
        self.links.insert(peer, topic, role, direction, state);
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
    let view = NodeView {
        subscriptions: &state.subscriptions,
        candidates: &state.candidates,
        links: &state.links,
        epoch_nonce: state.epoch_nonce,
    };
    let expected_relay = state.relay_selection.expected_links(&view);
    // The publish pass runs on the same dial tick, after the relay diff (015
    // FR-009b): the node's standing initiation links, selected unconditionally
    // — the M3 model establishes them for every node, regardless of relay
    // state (ADR 0034; m3/README.md).
    let expected_publish = state.publish_selection.expected_links(&view);
    // Clone the immutable bits the request builder needs so the loop can mutate
    // `state.links` without aliasing the whole struct.
    let self_id = state.self_id.clone();
    let signer = Arc::clone(&state.signer);

    let mut effects = Vec::new();
    let dial = |links: &mut LinkStore, peer: PeerId, topic: TopicId, role: LinkRole| {
        match links.get(&peer, &topic, role, LinkDirection::Out) {
            Some(LinkState::Active) => return None,
            Some(LinkState::AwaitingAccept) => {}
            None => {
                links.insert(
                    peer.clone(),
                    topic.clone(),
                    role,
                    LinkDirection::Out,
                    LinkState::AwaitingAccept,
                );
            }
        }
        let message = signed_connection(
            &self_id,
            signer.as_ref(),
            ConnectionAction::Request { topic, role },
        );
        Some(Effect::Send { to: peer, message })
    };
    for (peer, topic) in expected_relay {
        effects.extend(dial(&mut state.links, peer, topic, LinkRole::Relay));
    }
    for (peer, topic) in expected_publish {
        effects.extend(dial(&mut state.links, peer, topic, LinkRole::Publisher));
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
    let terminate = |peer: PeerId, topic: TopicId, role: LinkRole| Effect::Send {
        to: peer,
        message: signed_connection(
            &self_id,
            signer.as_ref(),
            ConnectionAction::Terminated { topic, role },
        ),
    };

    // One notice per held link entry, in key order (the ordered store makes
    // emission deterministic). A pair held in both directions of a role is
    // notified once per entry; the redundant notice is absorbed by the
    // counterpart's unknown-termination rule.
    let effects: Vec<Effect> = state
        .links
        .iter()
        .map(|(peer, topic, role, _, _)| (peer.clone(), topic.clone(), role))
        .collect::<Vec<_>>()
        .into_iter()
        .map(|(peer, topic, role)| terminate(peer, topic, role))
        .collect();

    state.links.clear();
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
            state.links.remove_topic(&topic);
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
        ConnectionAction::Request { topic, role } => {
            handle_connection_request(state, plain.emitter, topic, role)
        }
        ConnectionAction::Accepted { topic, role } => {
            handle_connection_accepted(state, &plain.emitter, &topic, role)
        }
        ConnectionAction::Terminated { topic, role } => {
            handle_connection_terminated(state, &plain.emitter, &topic, role)
        }
        ConnectionAction::Rejected { topic, role } => {
            handle_connection_rejected(state, &plain.emitter, &topic, role)
        }
    }
}

/// Transition for a verified `Request` from `emitter` on `topic` (FR-012),
/// dispatched by the request's carried link `role` (ADR 0032/0033).
///
/// The accept/reject *policy* is the role's injected
/// [`ConnectionAcceptanceStrategy`] — the relay slot for a relay request, the
/// publish slot for a publish-intent request (same seam contract, publish
/// parameters); the handler owns the mechanics. Policies membership-validate
/// against the **membership-derived** view (registration gates delivery, not
/// acceptance — the S7 pin): the topic must be among the node's own topics AND
/// the emitter a known member of it. An accepted request records the
/// `(emitter, topic, role, In)` link `Active` (idempotently) and replies
/// `Accepted` with the role to the carried emitter; a rejected one is dropped
/// with no state change and — except over-capacity — no reply.
fn handle_connection_request(
    state: &mut NodeState,
    emitter: PeerId,
    topic: TopicId,
    role: LinkRole,
) -> Vec<Effect> {
    // Registry-fold gate: before `Synced` the candidate view is partially
    // folded, so a bucket count derived from it can floor to 1 and the edge
    // predicate degenerate to always-true — an un-synced acceptor would fail
    // OPEN, admitting an edge the full view would reject (and the idempotent
    // re-Accept would then pin it). Drop silently until readiness. This closes
    // the pre-snapshot window only; post-sync membership deltas keep the
    // documented B-agreement assumption in play (ADR 0031).
    if !state.synced {
        tracing::info!(
            target: "pubsub_node::node",
            event = "message_dropped",
            cause = "not_synced",
            self_id = %state.self_id,
            emitter = %emitter,
            topic = %topic,
        );
        return Vec::new();
    }
    let view = NodeView {
        subscriptions: &state.subscriptions,
        candidates: &state.candidates,
        links: &state.links,
        epoch_nonce: state.epoch_nonce,
    };
    let admission = match role {
        LinkRole::Relay => state.acceptance_strategy.admit(&emitter, &topic, &view),
        LinkRole::Publisher => state
            .publish_acceptance_strategy
            .admit(&emitter, &topic, &view),
    };
    match admission {
        Admission::Accept => {
            // Idempotent: the map absorbs a duplicate; a re-dial re-sends
            // Accepted. An inbound link is Active at acceptance — the acceptor
            // has nothing to await (ADR 0032).
            state.links.insert(
                emitter.clone(),
                topic.clone(),
                role,
                LinkDirection::In,
                LinkState::Active,
            );
            let message = signed_connection(
                &state.self_id,
                state.signer.as_ref(),
                ConnectionAction::Accepted { topic, role },
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
                ConnectionAction::Rejected { topic, role },
            );
            vec![Effect::Send {
                to: emitter,
                message,
            }]
        }
    }
}

/// Transition for a verified `Accepted` from `emitter` on `topic` (FR-013),
/// keyed by the carried link `role`.
///
/// Activates the matching `AwaitingAccept` outbound entry of that role. An
/// `Accepted` with no matching pending entry (absent, already `Active`, or a
/// different role) is dropped and creates/modifies nothing.
fn handle_connection_accepted(
    state: &mut NodeState,
    emitter: &PeerId,
    topic: &TopicId,
    role: LinkRole,
) -> Vec<Effect> {
    if state.links.activate_out(emitter, topic, role) {
        return Vec::new();
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

/// Transition for a verified `Terminated` from `emitter` on `topic` (FR-014),
/// scoped to the carried link `role`.
///
/// Removes that role's matching entry in either direction (both, if both are
/// held); the other role's links between the same pair are untouched
/// (coexisting links, ADR 0032). A `Terminated` for a link not held is dropped;
/// a `Terminated` is never replied to.
fn handle_connection_terminated(
    state: &mut NodeState,
    emitter: &PeerId,
    topic: &TopicId,
    role: LinkRole,
) -> Vec<Effect> {
    let removed_out = state.links.remove(emitter, topic, role, LinkDirection::Out);
    let removed_in = state.links.remove(emitter, topic, role, LinkDirection::In);
    if !(removed_out || removed_in) {
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
/// Removes the matching `AwaitingAccept` outbound entry of the carried role so
/// the dialer stops waiting on an `Accepted` that will never come; that is the
/// **only** handling. There is no retry and no back-fill: the realized degree
/// may settle below target, and re-forming links is left to the future
/// heartbeat/reshuffle layer (retry/back-fill is a separate strategy family —
/// see `IMPLEMENTATION_NOTES`). A `Rejected` with no matching pending entry
/// (absent, already `Active`, or a different role) is dropped and changes
/// nothing. A rejection is never treated as misbehaviour.
fn handle_connection_rejected(
    state: &mut NodeState,
    emitter: &PeerId,
    topic: &TopicId,
    role: LinkRole,
) -> Vec<Effect> {
    if state.links.get(emitter, topic, role, LinkDirection::Out) == Some(LinkState::AwaitingAccept)
    {
        state.links.remove(emitter, topic, role, LinkDirection::Out);
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
/// the original [`SignedMessage`] (relays never re-sign — FR-007). The seam is
/// **origin-aware** (015 FR-005): publishing links are targets only for a
/// local origin. `exclude` is the split-horizon peer — the deliverer on the
/// receive path, `None` on the publish path.
fn fanout(
    state: &NodeState,
    topic: &TopicId,
    message: &SignedMessage,
    origin: &Origin,
    exclude: Option<&PeerId>,
) -> Vec<Effect> {
    state
        .fanout_strategy
        .targets(topic, &state.links, origin, exclude)
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
    // FR-016: admit only over a link oriented to *receive* from the deliverer —
    // an Active Relay/Out (dialed upstream, as before 015), or an inbound
    // publishing link (Publisher/In). Over a publishing link the deliverer must
    // BE the message's publisher (the receive-side dual of the origin-gated
    // send, ADR 0033): a foreign-publisher payload over a publish link is a
    // relay attempt the link's role forbids, dropped with its own cause.
    let topic = signed.plain.topic.clone();
    let relay_upstream = state
        .links
        .get(&from, &topic, LinkRole::Relay, LinkDirection::Out)
        == Some(LinkState::Active);
    let publish_inbound = state
        .links
        .get(&from, &topic, LinkRole::Publisher, LinkDirection::In)
        .is_some();
    // Owner binding under the M3 policy; the M5 policy admits any payload the
    // remaining checks pass (ADR 0035).
    let publisher_bound = match state.publish_in_admission {
        PublishInAdmission::OwnerOnly => {
            signed.plain.publisher_id.as_public_key() == from.as_public_key()
        }
        PublishInAdmission::AnyVerified => true,
    };
    if !relay_upstream {
        if publish_inbound && !publisher_bound {
            tracing::info!(
                target: "pubsub_node::node",
                event = "message_dropped",
                cause = "relay_over_publish_link",
                self_id = %state.self_id,
                from = %from,
                topic = %signed.plain.topic,
                publisher_id = %signed.plain.publisher_id,
            );
            return Vec::new();
        }
        if !publish_inbound {
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
        // a failure here is misbehavior by the deliverer. Sever silently the
        // link that ADMITTED the message — the Active relay upstream when the
        // relay gate passed, else the inbound initiation link (the ADR 0033 §5
        // dual: a publisher spamming invalid signatures over its standing link
        // loses it) — and raise the misbehavior signal (the executor logs
        // `connection_severed`); no Terminated is sent.
        if relay_upstream {
            state
                .links
                .remove(&from, &topic, LinkRole::Relay, LinkDirection::Out);
        } else {
            state
                .links
                .remove(&from, &topic, LinkRole::Publisher, LinkDirection::In);
        }
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
