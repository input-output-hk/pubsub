use super::super::*;
use super::*;

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
    apply(&mut state, Event::Synced); // requests are gated on readiness
    apply(&mut state, reg_open("t")); // registered, so payload can be admitted
    apply(&mut state, membership_joined("b", ["t"]));

    // setup → AwaitingAccept upstream + a Request.
    let e = apply(&mut state, Event::Heartbeat { interval: 0 });
    assert_eq!(
        upstream_state(&state, "b", "t"),
        Some(UpstreamState::AwaitingAccept)
    );
    assert_eq!(request_sends(&e, "self"), vec![(peer("b"), t.clone())]);

    // recurring setup re-dials the still-pending pair (entry kept).
    let e = apply(&mut state, Event::Heartbeat { interval: 0 });
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
    apply(&mut state, Event::Heartbeat { interval: 0 });
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
}

// SC-006: the same ConnectionScript twice yields the same final state.
#[test]
fn scripted_lifecycle_is_deterministic() {
    let t = topic("t");
    let run = || {
        let mut s = node_state("self", HashSet::from([t.clone()]));
        apply(&mut s, reg_open("t"));
        let script = ConnectionScript::new()
            .synced() // requests are gated on readiness; effect-free before membership
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

    apply(&mut state, Event::Heartbeat { interval: 0 }); // dials the absent peer
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
    apply(&mut state, Event::Heartbeat { interval: 0 });
    assert_eq!(
        upstream_state(&state, "absent", "t"),
        Some(UpstreamState::AwaitingAccept),
    );

    // SC-007: even with self in membership/candidates, self is never dialed.
    apply(&mut state, membership_joined("self", ["t"]));
    apply(&mut state, Event::Heartbeat { interval: 0 });
    assert_eq!(upstream_state(&state, "self", "t"), None);
}
