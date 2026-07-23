use super::super::*;
use super::*;

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
        Some(LinkState::AwaitingAccept),
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

// ADR 0031: the readiness gate is symmetric — a Heartbeat injected before
// Synced is dropped, exactly like an inbound Request. A dial pass over a
// partially-folded candidate view would floor B to 1 and dial everyone folded
// so far; synced acceptors verify those dials under the full view's larger B
// and drop them, each a stranded AwaitingAccept entry.
#[test]
fn heartbeat_before_synced_is_dropped() {
    let mut state = node_state("self", HashSet::from([topic("t1")]));
    apply(&mut state, membership_joined("a", ["t1"]));

    let effects = apply(&mut state, Event::Heartbeat);

    assert!(effects.is_empty(), "no dial before sync");
    assert!(
        state.upstream_relays().is_empty(),
        "no upstream entry recorded before sync",
    );
}

// ADR 0031: an Epoch event folds the nonce with no effects; the strategy reads
// the folded nonce on the next Heartbeat, so the dial set follows the epoch
// (and within one epoch a repeated Heartbeat is a pure retry pass).
#[test]
fn epoch_folds_nonce_for_the_next_heartbeat() {
    use crate::strategies::edge::is_valid_edge;

    // 30 candidates at target_degree 6 ⇒ B = 5: selection varies by nonce.
    let names: Vec<String> = (0..30).map(|i| format!("c{i:02}")).collect();
    let t = topic("t");
    let selection = |nonce: u64| -> BTreeSet<PeerId> {
        names
            .iter()
            .map(|n| peer(n))
            .filter(|c| is_valid_edge(nonce, &t, &peer("self"), c, 5))
            .collect()
    };
    let at_zero = selection(0);
    let nonce = (1..64)
        .find(|n| selection(*n) != at_zero)
        .expect("some nonce diverges from nonce 0");

    let mut state = NodeState::new(
        peer("self"),
        BTreeSet::from([t.clone()]),
        0, // genesis: the initial epoch nonce
        Arc::new(TestVerifier),
        alias_signer("self"),
        NodeStrategies::relay_only(
            Arc::new(HashGatedConnection::new(peer("self"), 6)),
            Arc::new(AcceptFromAllCandidates),
        ),
        Arc::new(ForwardToRelays),
    );
    apply(&mut state, reg_open("t"));
    apply(&mut state, membership_joined("self", ["t"]));
    for n in &names {
        apply(&mut state, membership_joined(n.as_str(), ["t"]));
    }

    // Readiness dials exactly the nonce-0 edge set.
    apply(&mut state, Event::Synced);
    let dialed: BTreeSet<PeerId> = state.upstream.keys().map(|k| k.peer.clone()).collect();
    assert_eq!(dialed, at_zero, "the readiness dial uses the genesis nonce");

    // The epoch fold emits nothing; the next Heartbeat dials the new set too
    // (expected-set membership never removes, so the union accumulates).
    let effects = apply(&mut state, Event::Epoch { nonce });
    assert!(effects.is_empty(), "the epoch fold emits nothing");
    apply(&mut state, Event::Heartbeat);
    let dialed: BTreeSet<PeerId> = state.upstream.keys().map(|k| k.peer.clone()).collect();
    let expected: BTreeSet<PeerId> = at_zero.union(&selection(nonce)).cloned().collect();
    assert_eq!(dialed, expected, "the next dial pass follows the new nonce");
}

// ---- T009: dialer side (FR-006..009, US1-AS1..4) --------------------------

// US1-AS1/AS2: a setup event dials every candidate across the node's topics —
// one AwaitingAccept entry and one Request (emitter self) per (peer, topic).
#[test]
fn setup_event_dials_all_candidates() {
    let mut state = node_state("self", HashSet::from([topic("t1")]));
    apply(&mut state, Event::Synced); // dials are gated on readiness
    apply(&mut state, membership_joined("a", ["t1"]));
    apply(&mut state, membership_joined("b", ["t1"]));

    let effects = apply(&mut state, Event::Heartbeat);

    assert_eq!(
        upstream_state(&state, "a", "t1"),
        Some(LinkState::AwaitingAccept),
    );
    assert_eq!(
        upstream_state(&state, "b", "t1"),
        Some(LinkState::AwaitingAccept),
    );
    assert_eq!(
        sorted_pairs(request_sends(&effects, "self")),
        sorted_pairs(vec![(peer("a"), topic("t1")), (peer("b"), topic("t1"))]),
    );
    assert!(
        state.downstream_relays().is_empty(),
        "dialing adds no downstream"
    );
}

// US1-AS2: connections are keyed per (peer, topic) — a peer sharing two topics
// yields two independent upstream connections.
#[test]
fn setup_keys_connections_per_peer_topic() {
    let mut state = node_state("self", HashSet::from([topic("t1"), topic("t2")]));
    apply(&mut state, Event::Synced); // dials are gated on readiness
    apply(&mut state, membership_joined("a", ["t1", "t2"]));

    let effects = apply(&mut state, Event::Heartbeat);

    assert_eq!(
        upstream_state(&state, "a", "t1"),
        Some(LinkState::AwaitingAccept),
    );
    assert_eq!(
        upstream_state(&state, "a", "t2"),
        Some(LinkState::AwaitingAccept),
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
    apply(&mut state, Event::Synced); // dials are gated on readiness
    let effects = apply(&mut state, Event::Heartbeat);
    assert!(effects.is_empty(), "no candidates → no requests");
    assert!(state.upstream_relays().is_empty());
}

// SC-007: the node never dials itself — a self membership event sets its own
// subscriptions (not a candidate), so self is never in the expected set.
#[test]
fn self_is_never_dialed() {
    let mut state = node_state("self", HashSet::new());
    apply(&mut state, Event::Synced); // dials are gated on readiness
    apply(&mut state, reg_open("t1")); // legitimate topic (registered first)
    apply(&mut state, membership_joined("self", ["t1"])); // own entry → subscriptions
    apply(&mut state, membership_joined("a", ["t1"])); // real candidate

    let effects = apply(&mut state, Event::Heartbeat);

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
    apply(&mut state, Event::Synced); // dials are gated on readiness
    apply(&mut state, membership_joined("a", ["t1"]));

    // First setup → a pending.
    apply(&mut state, Event::Heartbeat);
    assert_eq!(
        upstream_state(&state, "a", "t1"),
        Some(LinkState::AwaitingAccept),
    );

    // Repeat with a still pending → re-dialed (fresh Request), entry kept.
    let effects = apply(&mut state, Event::Heartbeat);
    assert_eq!(
        request_sends(&effects, "self"),
        vec![(peer("a"), topic("t1"))],
        "pending pair re-dialed",
    );
    assert_eq!(
        upstream_state(&state, "a", "t1"),
        Some(LinkState::AwaitingAccept),
    );

    // a accepts → Active. Add candidate b.
    apply(&mut state, accepted_from("a", "t1"));
    assert_eq!(upstream_state(&state, "a", "t1"), Some(LinkState::Active));
    apply(&mut state, membership_joined("b", ["t1"]));

    // Repeat → b dialed, a (Active) left alone and still present.
    let effects = apply(&mut state, Event::Heartbeat);
    assert_eq!(
        request_sends(&effects, "self"),
        vec![(peer("b"), topic("t1"))],
        "Active pair not re-dialed; new candidate dialed",
    );
    assert_eq!(upstream_state(&state, "a", "t1"), Some(LinkState::Active));
    assert_eq!(
        upstream_state(&state, "b", "t1"),
        Some(LinkState::AwaitingAccept),
    );
}

// US1-AS3 / FR-008: a membership update after setup folds into candidates but
// creates no connection entry and returns no effects; a later setup dials it.
#[test]
fn membership_update_after_setup_folds_only_then_later_setup_dials() {
    let mut state = node_state("self", HashSet::from([topic("t1")]));
    apply(&mut state, Event::Synced); // dials are gated on readiness
    apply(&mut state, membership_joined("a", ["t1"]));
    apply(&mut state, Event::Heartbeat);

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
    let effects = apply(&mut state, Event::Heartbeat);
    assert!(
        request_sends(&effects, "self").contains(&(peer("b"), topic("t1"))),
        "later setup dials the newly-known member",
    );
}
