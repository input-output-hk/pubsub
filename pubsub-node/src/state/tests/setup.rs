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
