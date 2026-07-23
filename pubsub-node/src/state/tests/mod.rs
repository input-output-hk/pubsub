//! Unit tests for the pure node core (`apply` and its `handle_*` chain), split
//! by concern across submodules. These are crate-internal tests that drive
//! `apply` directly and assert on private `NodeState` and the returned
//! `Vec<Effect>`, so they live under `state::tests::*` to retain access to the
//! module's internals (each submodule reaches them via `use super::super::*`).
//!
//! Shared imports and test helpers live here; the external test-deps are
//! re-exported `pub(crate)` so each submodule picks them up via `use super::*`.

mod apply_basics;
mod connection;
mod fanout;
mod gated_receive;
mod membership;
mod publisher_links;
mod setup;
mod severance;
mod shutdown;
mod symmetric_links;

use super::*;

pub(crate) use crate::connection_state::test_support::{
    accepted_from, membership_joined, misattributed_request, payload_from, payload_via,
    publisher_accepted_from, publisher_rejected_from, publisher_request_from,
    publisher_terminated_from, rejected_from, request_from, symmetric_accepted_from,
    symmetric_rejected_from, symmetric_request_from, symmetric_terminated_from,
    tampered_payload_from, terminated_from, ConnectionScript,
};
pub(crate) use crate::crypto::mock::{MockCryptoScheme, TestSigner, TestVerifier};
pub(crate) use crate::crypto::PublicKey;
pub(crate) use crate::crypto::{Signer, Timestamp};
pub(crate) use crate::message::HandshakeKind;
pub(crate) use crate::message::{MessagePayload, PlainMessage, SignedMessage};
pub(crate) use crate::strategies::acceptance::{
    AcceptFromAllCandidates, HashGatedBoundedAcceptance,
};
pub(crate) use crate::strategies::connection::{ConnectToAllCandidates, HashGatedConnection};
pub(crate) use crate::strategies::fanout::{ForwardToAll, ForwardToRelays};
pub(crate) use crate::subscription_registry::MembershipScript;
pub(crate) use crate::topic_registry::TopicRegistryScript;
pub(crate) use std::collections::BTreeSet;
pub(crate) use std::str::FromStr;

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
        subscriptions.iter().cloned().collect(),
        0, // genesis: the default initial epoch nonce
        Arc::new(TestVerifier),
        alias_signer(self_id),
        NodeStrategies::relay_only(strategy(), Arc::new(AcceptFromAllCandidates)),
        Arc::new(ForwardToRelays),
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

// ---- Connection lifecycle (US1): helpers ----------------------------------

/// The upstream state recorded for `(p, t)`, if any.
fn upstream_state(state: &NodeState, p: &str, t: &str) -> Option<LinkState> {
    state
        .upstream_relays()
        .into_iter()
        .find(|(pp, tt, _)| pp == &peer(p) && tt == &topic(t))
        .map(|(_, _, st)| st)
}

/// Whether a downstream entry is held for `(p, t)`.
fn has_downstream(state: &NodeState, p: &str, t: &str) -> bool {
    state.downstream_relays().contains(&(peer(p), topic(t)))
}

/// The `(to, topic)` of every `Request` send effect, any handshake kind
/// (asserting emitter == self).
fn request_sends(effects: &[Effect], expected_emitter: &str) -> Vec<(PeerId, TopicId)> {
    let mut out = Vec::new();
    for effect in effects {
        if let Effect::Send { to, message } = effect {
            if let Some((_, cm)) = message.connection_parts() {
                if let ConnectionAction::Request { topic } = &cm.plain.action {
                    assert_eq!(cm.plain.emitter, peer(expected_emitter), "request emitter");
                    out.push((to.clone(), topic.clone()));
                }
            }
        }
    }
    out
}

/// The `(to, topic)` of every `Accepted` send effect, any handshake kind
/// (asserting emitter == self).
fn accepted_sends(effects: &[Effect], expected_emitter: &str) -> Vec<(PeerId, TopicId)> {
    let mut out = Vec::new();
    for effect in effects {
        if let Effect::Send { to, message } = effect {
            if let Some((_, cm)) = message.connection_parts() {
                if let ConnectionAction::Accepted { topic } = &cm.plain.action {
                    assert_eq!(cm.plain.emitter, peer(expected_emitter), "accepted emitter");
                    out.push((to.clone(), topic.clone()));
                }
            }
        }
    }
    out
}

fn sorted_pairs(mut v: Vec<(PeerId, TopicId)>) -> Vec<(PeerId, TopicId)> {
    v.sort_by(|a, b| (a.0.to_string(), a.1.as_str()).cmp(&(b.0.to_string(), b.1.as_str())));
    v
}

// ---- T017: connection-gated delivery (US2, FR-016/019) --------------------

/// Seed an Active relay upstream `(peer, topic)` directly — the declarative
/// stand-in for a full setup→accept handshake when a test only needs the
/// gate to be open (the test module reaches `NodeState`'s private fields).
fn with_active_upstream(state: &mut NodeState, peer_alias: &str, t: &str) {
    state.upstream.insert(
        LinkKey::new(topic(t), peer(peer_alias), LinkKind::Relay),
        LinkState::Active,
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
            Effect::Misbehaved {
                peer, topic, cause, ..
            } => Some((peer.clone(), topic.clone(), *cause)),
            Effect::Send { .. } => None,
        })
        .collect()
}

/// The link kind of every `Misbehaved` effect — which class was severed.
fn severed_kinds(effects: &[Effect]) -> Vec<LinkKind> {
    effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::Misbehaved { kind, .. } => Some(*kind),
            Effect::Send { .. } => None,
        })
        .collect()
}

/// Whether any effect is a `Send` (severance must send nothing).
fn has_send(effects: &[Effect]) -> bool {
    effects.iter().any(|e| matches!(e, Effect::Send { .. }))
}

// ---- T024: graceful shutdown & Terminated reception (US4, FR-014/020) -----

/// Seed an accepted relay downstream entry `(peer, topic)` directly.
fn with_downstream(state: &mut NodeState, peer_alias: &str, t: &str) {
    state.downstream.insert(
        LinkKey::new(topic(t), peer(peer_alias), LinkKind::Relay),
        LinkState::Active,
    );
}

// ---- 015: publisher links — helpers ----------------------------------------

/// Construct a `NodeState` like [`node_state`] but with the **publisher** seam
/// pair configured (selection + acceptance).
fn node_state_with_publishers(
    self_id: &str,
    subscriptions: HashSet<TopicId>,
    publisher_strategy: Arc<dyn ConnectionStrategy>,
    publisher_acceptance: Arc<dyn ConnectionAcceptanceStrategy>,
) -> NodeState {
    let mut state = NodeState::new(
        peer(self_id),
        subscriptions.iter().cloned().collect(),
        0,
        Arc::new(TestVerifier),
        alias_signer(self_id),
        NodeStrategies {
            relay_connection: strategy(),
            relay_acceptance: Arc::new(AcceptFromAllCandidates),
            publisher_connection: Some(publisher_strategy),
            publisher_acceptance: Some(publisher_acceptance),
            symmetric_edges: false,
        },
        Arc::new(ForwardToRelays),
    );
    for t in subscriptions {
        state
            .registered_topics
            .insert(t, TopicEntry::from_publishers(BTreeSet::new()));
    }
    state
}

// ---- 015 review round 5: symmetric (M4) handshake — helpers -----------------

/// Construct a `NodeState` like [`node_state`] but in **symmetric** (M4)
/// mode, with an explicit relay acceptance instance (the policy the symmetric
/// handshake consults).
fn node_state_symmetric(
    self_id: &str,
    subscriptions: HashSet<TopicId>,
    relay_acceptance: Arc<dyn ConnectionAcceptanceStrategy>,
) -> NodeState {
    let mut state = NodeState::new(
        peer(self_id),
        subscriptions.iter().cloned().collect(),
        0,
        Arc::new(TestVerifier),
        alias_signer(self_id),
        NodeStrategies::relay_only(strategy(), relay_acceptance).with_symmetric_edges(true),
        Arc::new(ForwardToRelays),
    );
    for t in subscriptions {
        state
            .registered_topics
            .insert(t, TopicEntry::from_publishers(BTreeSet::new()));
    }
    state
}

/// Seed an accepted **publisher upstream** entry `(peer, topic)` directly —
/// the declarative stand-in for an accepted inbound initiation link.
fn with_upstream_publisher(state: &mut NodeState, peer_alias: &str, t: &str) {
    state.upstream.insert(
        LinkKey::new(topic(t), peer(peer_alias), LinkKind::Publisher),
        LinkState::Active,
    );
}

/// Seed a **publisher downstream** entry `(peer, topic)` directly, in the
/// given lifecycle state (the node's own initiation dial).
fn with_publisher_target(state: &mut NodeState, peer_alias: &str, t: &str, st: LinkState) {
    state.downstream.insert(
        LinkKey::new(topic(t), peer(peer_alias), LinkKind::Publisher),
        st,
    );
}

/// The `(to, topic)` of every **publisher-handshake** `Request` send effect.
fn publisher_request_sends(effects: &[Effect], expected_emitter: &str) -> Vec<(PeerId, TopicId)> {
    kind_sends(
        effects,
        expected_emitter,
        HandshakeKind::Publisher,
        |action| matches!(action, ConnectionAction::Request { .. }),
    )
}

/// The `(to, topic)` of every **publisher-handshake** `Accepted` send effect.
fn publisher_accepted_sends(effects: &[Effect], expected_emitter: &str) -> Vec<(PeerId, TopicId)> {
    kind_sends(
        effects,
        expected_emitter,
        HandshakeKind::Publisher,
        |action| matches!(action, ConnectionAction::Accepted { .. }),
    )
}

/// The `(to, topic)` of every control send under the `kind` handshake's
/// vocabulary matching `select`.
fn kind_sends(
    effects: &[Effect],
    expected_emitter: &str,
    kind: HandshakeKind,
    select: impl Fn(&ConnectionAction) -> bool,
) -> Vec<(PeerId, TopicId)> {
    let mut out = Vec::new();
    for effect in effects {
        if let Effect::Send { to, message } = effect {
            if let Some((sent_kind, cm)) = message.connection_parts() {
                if sent_kind == kind && select(&cm.plain.action) {
                    assert_eq!(cm.plain.emitter, peer(expected_emitter), "control emitter");
                    let (ConnectionAction::Request { topic }
                    | ConnectionAction::Accepted { topic }
                    | ConnectionAction::Terminated { topic }
                    | ConnectionAction::Rejected { topic }) = &cm.plain.action;
                    out.push((to.clone(), topic.clone()));
                }
            }
        }
    }
    out
}

/// The publisher-downstream state recorded for `(p, t)`, if any.
fn publisher_target_state(state: &NodeState, p: &str, t: &str) -> Option<LinkState> {
    state
        .downstream_publishers()
        .into_iter()
        .find(|(pp, tt, _)| pp == &peer(p) && tt == &topic(t))
        .map(|(_, _, st)| st)
}

/// Whether an accepted publisher upstream is held for `(p, t)`.
fn has_upstream_publisher(state: &NodeState, p: &str, t: &str) -> bool {
    state.upstream_publishers().contains(&(peer(p), topic(t)))
}

/// The `(to, topic)` of every `Terminated` send effect, any handshake kind
/// (asserting emitter).
fn terminated_sends(effects: &[Effect], expected_emitter: &str) -> Vec<(PeerId, TopicId)> {
    let mut out = Vec::new();
    for effect in effects {
        if let Effect::Send { to, message } = effect {
            if let Some((_, cm)) = message.connection_parts() {
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
    }
    out
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
