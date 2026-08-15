//! Unit tests for the **symmetric** (M4) handshake — constructed reciprocity
//! (review round 5, ADR 0034): one accept decision records the link in both
//! directions on both ends; refusals leave no one-sided half; teardown and
//! severance remove both halves together.

use super::super::*;
use super::*;

fn symmetric_state(self_id: &str) -> NodeState {
    let mut state =
        node_state_symmetric(self_id, HashSet::from([topic("t1")]), accept_all(self_id));
    apply(&mut state, Event::Synced);
    apply(&mut state, membership_joined("b", ["t1"]));
    state
}

// ADR 0034 §1: accepting a symmetric request records the emitter in BOTH
// collections as an Active relay-class link, and replies Accepted under the
// symmetric vocabulary.
#[test]
fn symmetric_request_accept_records_both_directions() {
    let mut state = symmetric_state("self");

    let effects = apply(&mut state, symmetric_request_from("b", "t1"));

    assert_eq!(upstream_state(&state, "b", "t1"), Some(LinkState::Active));
    assert!(has_downstream(&state, "b", "t1"));
    let accepted = kind_sends(&effects, "self", HandshakeKind::Symmetric, |action| {
        matches!(action, ConnectionAction::Accepted { .. })
    });
    assert_eq!(accepted, vec![(peer("b"), topic("t1"))]);
}

// The dialer's mirror: an acceptance of this node's own symmetric dial
// activates the pending upstream entry AND inserts the downstream mirror.
#[test]
fn symmetric_accepted_promotes_dial_and_mirrors_downstream() {
    let mut state = symmetric_state("self");
    // Seed this node's own pending symmetric dial (relay-class entry).
    state.upstream.insert(
        LinkKey::new(topic("t1"), peer("b"), LinkKind::Relay),
        LinkState::AwaitingAccept,
    );

    let effects = apply(&mut state, symmetric_accepted_from("b", "t1"));

    assert!(effects.is_empty());
    assert_eq!(upstream_state(&state, "b", "t1"), Some(LinkState::Active));
    assert!(has_downstream(&state, "b", "t1"));
}

// The crossing race (both ends of a valid edge dial): the peer's request was
// accepted first — inserting both halves Active — and the acceptance of this
// node's own dial then arrives. It is re-affirmed idempotently, not treated
// as unsolicited, and the pair stays intact.
#[test]
fn symmetric_accepted_after_crossing_request_is_idempotent() {
    let mut state = symmetric_state("self");
    apply(&mut state, symmetric_request_from("b", "t1"));

    let effects = apply(&mut state, symmetric_accepted_from("b", "t1"));

    assert!(effects.is_empty());
    assert_eq!(upstream_state(&state, "b", "t1"), Some(LinkState::Active));
    assert!(has_downstream(&state, "b", "t1"));
}

// A symmetric acceptance matching no upstream entry at all creates nothing.
#[test]
fn unsolicited_symmetric_accept_creates_nothing() {
    let mut state = symmetric_state("self");

    let effects = apply(&mut state, symmetric_accepted_from("b", "t1"));

    assert!(effects.is_empty());
    assert_eq!(upstream_state(&state, "b", "t1"), None);
    assert!(!has_downstream(&state, "b", "t1"));
}

// Teardown atomicity: a symmetric Terminated removes both halves of the pair.
#[test]
fn symmetric_terminated_removes_both_halves() {
    let mut state = symmetric_state("self");
    apply(&mut state, symmetric_request_from("b", "t1"));

    let effects = apply(&mut state, symmetric_terminated_from("b", "t1"));

    assert!(effects.is_empty(), "Terminated is never replied to");
    assert_eq!(upstream_state(&state, "b", "t1"), None);
    assert!(!has_downstream(&state, "b", "t1"));
}

// Severance atomicity: a tampered payload over a symmetric link severs the
// admitting upstream half AND its downstream mirror — on a symmetric node
// every relay link is bidirectional by construction.
#[test]
fn severance_on_symmetric_node_removes_the_mirror() {
    let mut state = symmetric_state("self");
    apply(&mut state, symmetric_request_from("b", "t1"));

    let effects = apply(&mut state, tampered_payload_from("b", "t1", 7));

    assert_eq!(severed_kinds(&effects), vec![LinkKind::Relay]);
    assert_eq!(upstream_state(&state, "b", "t1"), None);
    assert!(!has_downstream(&state, "b", "t1"));
}

// One accept decision per edge makes capped acceptance compatible with
// symmetric mode: an over-capacity refusal replies Rejected and inserts
// nothing, so no one-sided half of the pair can survive it. (A fed cap of 1;
// no gate, so only the cap refuses.)
#[test]
fn symmetric_over_capacity_refusal_leaves_no_partial_pair() {
    let mut state = node_state_symmetric(
        "self",
        HashSet::from([topic("t1")]),
        Arc::new(
            UnifiedAcceptance::new(peer("self"))
                .with_symmetric(true)
                .with_accept_cap(Some(1)),
        ),
    );
    apply(&mut state, Event::Synced);
    apply(&mut state, membership_joined("b", ["t1"]));
    apply(&mut state, membership_joined("c", ["t1"]));

    // First request fills the cap (both halves recorded).
    apply(&mut state, symmetric_request_from("b", "t1"));
    assert!(has_downstream(&state, "b", "t1"));

    // The second is refused whole: an explicit Rejected, nothing inserted.
    let effects = apply(&mut state, symmetric_request_from("c", "t1"));
    let rejected = kind_sends(&effects, "self", HandshakeKind::Symmetric, |action| {
        matches!(action, ConnectionAction::Rejected { .. })
    });
    assert_eq!(rejected, vec![(peer("c"), topic("t1"))]);
    assert_eq!(upstream_state(&state, "c", "t1"), None);
    assert!(!has_downstream(&state, "c", "t1"));
}

// ADR 0042: a crossing — a request from the peer this node's own dial is
// awaiting — is the node's own selection answered, not an admission: it
// short-circuits ahead of the policy (here a cap of 0, which admits no one)
// and spends no budget.
#[test]
fn crossing_request_bypasses_the_cap_and_spends_no_budget() {
    let mut state = node_state_symmetric(
        "self",
        HashSet::from([topic("t1")]),
        Arc::new(
            UnifiedAcceptance::new(peer("self"))
                .with_symmetric(true)
                .with_accept_cap(Some(0)),
        ),
    );
    apply(&mut state, Event::Synced);
    apply(&mut state, membership_joined("b", ["t1"]));
    apply(&mut state, membership_joined("c", ["t1"]));
    state.upstream.insert(
        LinkKey::new(topic("t1"), peer("b"), LinkKind::Relay),
        LinkState::AwaitingAccept,
    );

    // The crossing is accepted whole despite the serve-none cap.
    let effects = apply(&mut state, symmetric_request_from("b", "t1"));
    let accepted = kind_sends(&effects, "self", HandshakeKind::Symmetric, |action| {
        matches!(action, ConnectionAction::Accepted { .. })
    });
    assert_eq!(accepted, vec![(peer("b"), topic("t1"))]);
    assert_eq!(upstream_state(&state, "b", "t1"), Some(LinkState::Active));
    assert!(has_downstream(&state, "b", "t1"));

    // No budget was spent: a fresh request still sees the full (zero) cap —
    // refused because the cap is 0, not because the crossing consumed it.
    let effects = apply(&mut state, symmetric_request_from("c", "t1"));
    let rejected = kind_sends(&effects, "self", HandshakeKind::Symmetric, |action| {
        matches!(action, ConnectionAction::Rejected { .. })
    });
    assert_eq!(rejected, vec![(peer("c"), topic("t1"))]);
}

// ADR 0042: the node's own accepted dials (the mirror step) spend no budget
// — a symmetric node's cap bounds what other peers chose, never its own
// picks. With cap 1: a mirrored own dial occupies the link set, yet a fresh
// arrival is still admitted; only the SECOND fresh arrival exhausts the
// budget.
#[test]
fn own_mirrors_spend_no_budget_only_fresh_admissions_do() {
    let mut state = node_state_symmetric(
        "self",
        HashSet::from([topic("t1")]),
        Arc::new(
            UnifiedAcceptance::new(peer("self"))
                .with_symmetric(true)
                .with_accept_cap(Some(1)),
        ),
    );
    apply(&mut state, Event::Synced);
    for p in ["b", "c", "d"] {
        apply(&mut state, membership_joined(p, ["t1"]));
    }
    // This node's own dial to b, accepted: the mirror inserts both halves.
    state.upstream.insert(
        LinkKey::new(topic("t1"), peer("b"), LinkKind::Relay),
        LinkState::AwaitingAccept,
    );
    apply(&mut state, symmetric_accepted_from("b", "t1"));
    assert!(has_downstream(&state, "b", "t1"));

    // The link scan holds b, but the budget is untouched: c admits.
    apply(&mut state, symmetric_request_from("c", "t1"));
    assert!(has_downstream(&state, "c", "t1"));

    // c's admission spent the budget of 1: d is refused.
    let effects = apply(&mut state, symmetric_request_from("d", "t1"));
    let rejected = kind_sends(&effects, "self", HandshakeKind::Symmetric, |action| {
        matches!(action, ConnectionAction::Rejected { .. })
    });
    assert_eq!(rejected, vec![(peer("d"), topic("t1"))]);
}

// ADR 0042: the budget is per-epoch — the Epoch fold refunds it.
#[test]
fn epoch_rotation_refunds_the_admissions_budget() {
    let mut state = node_state_symmetric(
        "self",
        HashSet::from([topic("t1")]),
        Arc::new(
            UnifiedAcceptance::new(peer("self"))
                .with_symmetric(true)
                .with_accept_cap(Some(1)),
        ),
    );
    apply(&mut state, Event::Synced);
    for p in ["b", "c", "d"] {
        apply(&mut state, membership_joined(p, ["t1"]));
    }
    apply(&mut state, symmetric_request_from("b", "t1"));
    let effects = apply(&mut state, symmetric_request_from("c", "t1"));
    let rejected = kind_sends(&effects, "self", HandshakeKind::Symmetric, |action| {
        matches!(action, ConnectionAction::Rejected { .. })
    });
    assert_eq!(rejected, vec![(peer("c"), topic("t1"))]);

    apply(&mut state, Event::Epoch { nonce: 1 });
    apply(&mut state, symmetric_request_from("d", "t1"));
    assert!(has_downstream(&state, "d", "t1"));
}

// A symmetric Rejected drops this node's pending dial; no half was inserted
// on either end, so the edge simply does not form.
#[test]
fn symmetric_rejected_drops_the_pending_dial() {
    let mut state = symmetric_state("self");
    state.upstream.insert(
        LinkKey::new(topic("t1"), peer("b"), LinkKind::Relay),
        LinkState::AwaitingAccept,
    );

    let effects = apply(&mut state, symmetric_rejected_from("b", "t1"));

    assert!(effects.is_empty());
    assert_eq!(upstream_state(&state, "b", "t1"), None);
    assert!(!has_downstream(&state, "b", "t1"));
}

// The reverse guard: a symmetric node drops inbound RELAY handshakes
// outright — admitting a directional request would record a one-way link on
// a node whose teardown/severance mechanics assume every relay link is
// mirrored.
#[test]
fn relay_request_on_symmetric_node_is_dropped() {
    let mut state = symmetric_state("self");

    let effects = apply(&mut state, request_from("b", "t1"));

    assert!(effects.is_empty());
    assert_eq!(upstream_state(&state, "b", "t1"), None);
    assert!(!has_downstream(&state, "b", "t1"));
}

// Fail-closed: a node NOT in symmetric mode drops inbound symmetric
// handshakes outright — a directional-model node never mirrors a link.
#[test]
fn symmetric_request_on_directional_node_is_dropped() {
    let mut state = node_state("self", HashSet::from([topic("t1")]));
    apply(&mut state, Event::Synced);
    apply(&mut state, membership_joined("b", ["t1"]));

    let effects = apply(&mut state, symmetric_request_from("b", "t1"));

    assert!(effects.is_empty());
    assert_eq!(upstream_state(&state, "b", "t1"), None);
    assert!(!has_downstream(&state, "b", "t1"));
}

// Shutdown on a symmetric node: a symmetric link lives in both collections
// but is ONE link — the peer gets exactly one Terminated, under the
// symmetric vocabulary.
#[test]
fn symmetric_shutdown_notifies_each_link_once() {
    let mut state = symmetric_state("self");
    apply(&mut state, symmetric_request_from("b", "t1"));

    let effects = apply(&mut state, Event::Shutdown);

    let terminated = kind_sends(&effects, "self", HandshakeKind::Symmetric, |action| {
        matches!(action, ConnectionAction::Terminated { .. })
    });
    assert_eq!(terminated, vec![(peer("b"), topic("t1"))]);
    assert_eq!(effects.len(), 1, "no redundant second notice");
}

// The dial side of the vocabulary: a symmetric node's relay picks go out as
// symmetric-handshake Requests (the stored dial entries stay relay-class).
#[test]
fn symmetric_mode_dials_under_the_symmetric_vocabulary() {
    let mut state = node_state_symmetric("self", HashSet::from([topic("t1")]), accept_all("self"));
    apply(&mut state, membership_joined("b", ["t1"]));

    let effects = apply(&mut state, Event::Synced);

    let symmetric_requests = kind_sends(&effects, "self", HandshakeKind::Symmetric, |action| {
        matches!(action, ConnectionAction::Request { .. })
    });
    assert_eq!(symmetric_requests, vec![(peer("b"), topic("t1"))]);
    assert_eq!(
        upstream_state(&state, "b", "t1"),
        Some(LinkState::AwaitingAccept)
    );
}
