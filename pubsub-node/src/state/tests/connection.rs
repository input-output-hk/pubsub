use super::super::*;
use super::*;

// ---- T010: acceptor + activation side (FR-011..015, US1-AS5..7) -----------

// US1-AS5 / FR-012: a membership-valid Request is accepted — downstream entry
// recorded and Accepted sent to the carried emitter.
#[test]
fn membership_valid_request_is_accepted() {
    let mut state = node_state("self", HashSet::from([topic("t1")]));
    apply(&mut state, Event::Synced); // requests are gated on readiness
    apply(&mut state, membership_joined("a", ["t1"]));

    let effects = apply(&mut state, request_from("a", "t1"));

    assert!(has_downstream(&state, "a", "t1"), "downstream recorded");
    assert_eq!(
        accepted_sends(&effects, "self"),
        vec![(peer("a"), topic("t1"))],
        "Accepted sent to the carried emitter",
    );
}

// ADR 0031: an inbound Request before `Synced` is dropped — a partially-folded
// candidate view must not feed the acceptance decision (the fail-open gate).
// The same request after readiness is accepted.
#[test]
fn request_before_synced_is_dropped() {
    let mut state = node_state("self", HashSet::from([topic("t1")]));
    apply(&mut state, membership_joined("a", ["t1"]));

    let effects = apply(&mut state, request_from("a", "t1"));
    assert!(effects.is_empty(), "no reply before readiness");
    assert!(!has_downstream(&state, "a", "t1"), "no downstream recorded");

    apply(&mut state, Event::Synced);
    let effects = apply(&mut state, request_from("a", "t1"));
    assert!(
        has_downstream(&state, "a", "t1"),
        "accepted after readiness"
    );
    assert_eq!(
        accepted_sends(&effects, "self"),
        vec![(peer("a"), topic("t1"))],
    );
}

// US1-AS7 / FR-012: a Request fails validation when the topic is not among the
// node's own topics, or the requester is not a known member — silent drop,
// no downstream, no reply.
#[test]
fn request_dropped_when_membership_validation_fails() {
    // (a) topic not among own topics.
    let mut state = node_state("self", HashSet::new());
    apply(&mut state, Event::Synced); // past the readiness gate: membership does the refusing
    apply(&mut state, membership_joined("a", ["t1"]));
    let effects = apply(&mut state, request_from("a", "t1"));
    assert!(!has_downstream(&state, "a", "t1"));
    assert!(effects.is_empty(), "no reply when topic not own");

    // (b) requester not a known member.
    let mut state = node_state("self", HashSet::from([topic("t1")]));
    apply(&mut state, Event::Synced);
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
    apply(&mut state, Event::Synced); // past the readiness gate: membership does the refusing
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
    apply(&mut state, Event::Synced); // requests are gated on readiness
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
    apply(&mut state, Event::Heartbeat);
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
    apply(&mut state, Event::Synced); // requests are gated on readiness
    apply(&mut state, membership_joined("a", ["t1"]));
    // Establish both roles with a: upstream via setup+accept, downstream via request.
    apply(&mut state, Event::Heartbeat);
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

// ---- feature 005 (US2): bounded acceptance + rejected-dial back-fill --------

/// The destination and connection action of a `Send` effect carrying a
/// connection-control message, if any.
fn sent_action(effect: &Effect) -> Option<(&PeerId, &ConnectionAction)> {
    match effect {
        Effect::Send {
            to,
            message: Message::Connection(cm),
        } => Some((to, &cm.plain.action)),
        _ => None,
    }
}

// FR-007/FR-008 / US2: at the per-topic cap, `VerifiableBoundedAcceptance`
// refuses a further *legitimate* request with an explicit `Rejected` (not a
// severance) and records no downstream entry. A small topic (a single candidate
// ⇒ B=1) makes the edge predicate always hold, so this exercises the cap + the
// handler wiring (the predicate/bucket math is covered by the strategy's own
// unit tests).
#[test]
fn over_capacity_request_is_rejected_with_signal_not_severance() {
    let t = topic("t1");
    // target_degree = 1 ⇒ cap = ⌈1 + 3·√1⌉ = 4.
    let mut state = NodeState::new(
        peer("self"),
        BTreeSet::from([t.clone()]),
        0, // genesis: the default initial epoch nonce
        Arc::new(TestVerifier),
        alias_signer("self"),
        strategy(),
        Arc::new(ForwardToAll),
        Arc::new(VerifiableBoundedAcceptance::new(peer("self"), 1, 3)),
    );
    // Synced first (requests are gated on readiness) and before any membership,
    // so the readiness dial pass sees no candidates and pollutes no upstream.
    apply(&mut state, Event::Synced);
    apply(&mut state, reg_open("t1"));
    apply(&mut state, membership_joined("self", ["t1"]));
    apply(&mut state, membership_joined("a", ["t1"])); // sole candidate ⇒ B=1
                                                       // Pre-seed downstream to the cap (4 already-accepted peers on t1).
    for p in ["w", "x", "y", "z"] {
        with_downstream(&mut state, p, "t1");
    }

    // `a` is a member and B=1 (predicate holds), but the topic is at its cap ⇒
    // refused with an explicit Rejected, no downstream entry, no Misbehaved.
    let reject = apply(&mut state, request_from("a", "t1"));
    assert!(!state.downstream.contains(&(peer("a"), t.clone())));
    assert_eq!(reject.len(), 1);
    assert!(matches!(
        sent_action(&reject[0]),
        Some((to, ConnectionAction::Rejected { .. })) if to == &peer("a")
    ));
    assert!(!reject
        .iter()
        .any(|e| matches!(e, Effect::Misbehaved { .. })));
}

// US2: a Rejected dial removes only the matching pending upstream so the dialer
// stops waiting on an Accepted that will never come — the *only* handling. No
// retry, no back-fill: the other pending upstreams are untouched and the degree
// simply settles lower (re-forming is the future heartbeat/rotation layer's job).
#[test]
fn rejected_dial_removes_pending_upstream() {
    let t = topic("t1");
    // target_degree = 8 with 3 candidates ⇒ B = 1 ⇒ all three are dialed (small-topic path).
    let mut state = NodeState::new(
        peer("self"),
        BTreeSet::from([t.clone()]),
        0, // genesis: the default initial epoch nonce
        Arc::new(TestVerifier),
        alias_signer("self"),
        Arc::new(HashGatedConnection::new(peer("self"), 8)),
        Arc::new(ForwardToAll),
        Arc::new(AcceptFromAllCandidates),
    );
    apply(&mut state, reg_open("t1"));
    apply(&mut state, membership_joined("self", ["t1"]));
    for c in ["a", "b", "c"] {
        apply(&mut state, membership_joined(c, ["t1"]));
    }

    apply(&mut state, Event::Synced); // fires Heartbeat(0); B=1 dials all three
    let dialed: Vec<PeerId> = state.upstream.keys().map(|(p, _)| p.clone()).collect();
    assert_eq!(dialed.len(), 3, "B=1 dials every candidate");
    let rejected_peer = dialed[0].clone();

    // That peer rejects the dial: only its pending upstream is dropped; no
    // effects, no back-fill; the others remain.
    let effects = apply(&mut state, rejected_from(&rejected_peer.to_string(), "t1"));
    assert!(effects.is_empty(), "a rejection produces no effects");
    assert!(
        !state
            .upstream
            .contains_key(&(rejected_peer.clone(), t.clone())),
        "the rejected pending upstream is removed",
    );
    assert_eq!(
        state.upstream.len(),
        2,
        "no retry/back-fill; the other pending upstreams remain",
    );
}
