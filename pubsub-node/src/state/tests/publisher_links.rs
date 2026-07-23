//! 015 publisher links: the M3 behaviours — unconditional strategy-driven
//! establishment, kind-dispatched control handling, the kind-agnostic receive
//! gate, admitting-link severance, and the origin split in fan-out.

use super::super::*;
use super::*;

// ---- T011: establishment + control lifecycle -------------------------------

// FR-002: the publisher pass dials the strategy's picks on the readiness
// heartbeat — with an EMPTY relay topology and with a full one, identically
// (publisher dials are unconditional, never contingent on relay links).
#[test]
fn publisher_dials_fire_unconditionally() {
    let dial = |seed_relay_downstream: bool| {
        let mut state = node_state_with_publishers(
            "self",
            HashSet::from([topic("t1")]),
            Arc::new(ConnectToAllCandidates),
            Arc::new(AcceptFromAllCandidates),
        );
        apply(&mut state, membership_joined("self", ["t1"]));
        apply(&mut state, membership_joined("a", ["t1"]));
        apply(&mut state, membership_joined("b", ["t1"]));
        if seed_relay_downstream {
            with_downstream(&mut state, "a", "t1");
            with_downstream(&mut state, "b", "t1");
        }
        let effects = apply(&mut state, Event::Synced); // readiness dial pass
        (
            sorted_pairs(publisher_request_sends(&effects, "self")),
            state,
        )
    };

    let (sends_empty, state) = dial(false);
    assert_eq!(
        sends_empty,
        vec![(peer("a"), topic("t1")), (peer("b"), topic("t1"))],
        "publisher Requests to every pick",
    );
    assert_eq!(
        publisher_target_state(&state, "a", "t1"),
        Some(LinkState::AwaitingAccept),
        "dial recorded with its lifecycle",
    );

    let (sends_full, _) = dial(true);
    assert_eq!(
        sends_full, sends_empty,
        "a full relay downstream changes nothing — unconditional",
    );
}

// A node with NO publisher strategy configured never dials publisher links
// (FR-014 dial side): the heartbeat emits relay Requests only.
#[test]
fn no_publisher_strategy_never_dials() {
    let mut state = node_state("self", HashSet::from([topic("t1")]));
    apply(&mut state, membership_joined("self", ["t1"]));
    apply(&mut state, membership_joined("a", ["t1"]));
    let effects = apply(&mut state, Event::Synced);
    assert!(
        publisher_request_sends(&effects, "self").is_empty(),
        "no publisher dials without a configured strategy",
    );
    assert!(state.downstream_publishers().is_empty());
}

// A publisher-kind Request on a node WITH publisher acceptance configured is
// admitted into upstream × Publisher (Active on insert) and replied to with a
// publisher-kind Accepted.
#[test]
fn publisher_request_is_accepted_into_upstream_publishers() {
    let mut state = node_state_with_publishers(
        "self",
        HashSet::from([topic("t1")]),
        Arc::new(ConnectToAllCandidates),
        Arc::new(AcceptFromAllCandidates),
    );
    apply(&mut state, Event::Synced);
    apply(&mut state, membership_joined("a", ["t1"]));

    let effects = apply(&mut state, publisher_request_from("a", "t1"));

    assert!(has_upstream_publisher(&state, "a", "t1"));
    assert!(
        state.downstream_relays().is_empty(),
        "a publisher request never lands in the relay downstream",
    );
    assert_eq!(
        publisher_accepted_sends(&effects, "self"),
        vec![(peer("a"), topic("t1"))],
    );
}

// FR-014 accept side: a node with NO publisher acceptance configured silently
// drops an inbound publisher Request — no state, no reply.
#[test]
fn publisher_request_dropped_when_publisher_links_disabled() {
    let mut state = node_state("self", HashSet::from([topic("t1")]));
    apply(&mut state, Event::Synced);
    apply(&mut state, membership_joined("a", ["t1"]));

    let effects = apply(&mut state, publisher_request_from("a", "t1"));

    assert!(effects.is_empty(), "silent drop, no reply");
    assert!(!has_upstream_publisher(&state, "a", "t1"));
    assert!(state.downstream_relays().is_empty());
}

// A publisher-kind Accepted activates the matching publisher dial in
// downstream × Publisher (not any relay entry).
#[test]
fn publisher_accepted_activates_the_publisher_dial() {
    let mut state = node_state("self", HashSet::from([topic("t1")]));
    with_publisher_target(&mut state, "a", "t1", LinkState::AwaitingAccept);

    let effects = apply(&mut state, publisher_accepted_from("a", "t1"));

    assert!(effects.is_empty(), "activation sends nothing");
    assert_eq!(
        publisher_target_state(&state, "a", "t1"),
        Some(LinkState::Active),
    );
}

// FR-015: kind-scoped teardown — a publisher-kind Terminated removes the
// publisher entries for the peer/topic and leaves coexisting relay links
// (either direction) untouched; and vice versa.
#[test]
fn terminated_is_kind_scoped() {
    let mut state = node_state("self", HashSet::from([topic("t1")]));
    with_active_upstream(&mut state, "a", "t1"); // relay upstream
    with_downstream(&mut state, "a", "t1"); // relay downstream
    with_upstream_publisher(&mut state, "a", "t1"); // publisher upstream
    with_publisher_target(&mut state, "a", "t1", LinkState::Active); // publisher downstream

    apply(&mut state, publisher_terminated_from("a", "t1"));
    assert!(!has_upstream_publisher(&state, "a", "t1"));
    assert_eq!(publisher_target_state(&state, "a", "t1"), None);
    assert_eq!(
        upstream_state(&state, "a", "t1"),
        Some(LinkState::Active),
        "relay upstream survives a publisher termination",
    );
    assert!(has_downstream(&state, "a", "t1"));

    apply(&mut state, terminated_from("a", "t1"));
    assert_eq!(upstream_state(&state, "a", "t1"), None);
    assert!(!has_downstream(&state, "a", "t1"));
}

// A publisher-kind Rejected removes the pending publisher dial (the dialed
// collection for the publisher kind is downstream) and nothing else.
#[test]
fn publisher_rejected_removes_the_pending_publisher_dial() {
    let mut state = node_state("self", HashSet::from([topic("t1")]));
    with_publisher_target(&mut state, "a", "t1", LinkState::AwaitingAccept);
    with_active_upstream(&mut state, "a", "t1");

    let effects = apply(&mut state, publisher_rejected_from("a", "t1"));

    assert!(effects.is_empty());
    assert_eq!(publisher_target_state(&state, "a", "t1"), None);
    assert_eq!(
        upstream_state(&state, "a", "t1"),
        Some(LinkState::Active),
        "the relay upstream is untouched",
    );
}

// ---- T012: the receive gate + severance -------------------------------------

// FR-006 (as amended): the receive gate is kind-agnostic — an Active inbound
// publisher link admits any authentic message, the link owner's own
// publication and a foreign publisher's alike. A receiver validates a
// publisher-link arrival exactly like any message (signature, registration,
// authorization, subscription, dedup); a link's kind restricts what its
// holder sends, not what a receiver admits.
#[test]
fn publisher_link_admits_any_authentic_message() {
    let mut state = node_state("self", HashSet::from([topic("t1")]));
    with_upstream_publisher(&mut state, "a", "t1");

    // The owner's own publication: admitted and recorded.
    apply(&mut state, payload_from("a", "t1", 1));
    assert_eq!(
        state.received_snapshot().len(),
        1,
        "owner's message admitted"
    );

    // A message published by b, delivered by a over its publisher link:
    // equally admitted — no owner-binding on the receive side.
    apply(&mut state, payload_via("a", "b", "t1", 2));
    assert_eq!(
        state.received_snapshot().len(),
        2,
        "foreign publisher over a publisher link is admitted",
    );
}

// A peer with NO link at all still gets nothing through (the gate is intact).
#[test]
fn no_link_still_means_not_connected() {
    let mut state = node_state("self", HashSet::from([topic("t1")]));
    apply(&mut state, payload_from("a", "t1", 1));
    assert!(state.received_snapshot().is_empty());
}

// FR-010: an invalidly-signed payload admitted over a publisher link severs
// THAT publisher link — not a relay entry that may not exist.
#[test]
fn tampered_payload_severs_the_admitting_publisher_link() {
    let mut state = node_state("self", HashSet::from([topic("t1")]));
    with_upstream_publisher(&mut state, "a", "t1");
    with_active_upstream(&mut state, "a", "t1"); // coexisting relay upstream

    // Tampered payload published (and delivered) by the link owner: it passes
    // the gate over the RELAY link first (relay admission precedes publisher
    // admission), so the relay link is the admitting link here.
    let effects = apply(&mut state, tampered_payload_from("a", "t1", 7));
    assert_eq!(misbehaved(&effects).len(), 1);
    assert_eq!(severed_kinds(&effects), vec![LinkKind::Relay]);
    assert_eq!(
        upstream_state(&state, "a", "t1"),
        None,
        "the admitting relay link is severed",
    );
    assert!(
        has_upstream_publisher(&state, "a", "t1"),
        "the publisher link was not the admitting link and survives",
    );

    // Same again with ONLY the publisher link: now it is the admitting link.
    let mut state = node_state("self", HashSet::from([topic("t1")]));
    with_upstream_publisher(&mut state, "a", "t1");
    let effects = apply(&mut state, tampered_payload_from("a", "t1", 8));
    assert_eq!(misbehaved(&effects).len(), 1);
    assert_eq!(severed_kinds(&effects), vec![LinkKind::Publisher]);
    assert!(
        !has_upstream_publisher(&state, "a", "t1"),
        "the admitting publisher link is severed",
    );
}

// ---- T013: fan-out origin split ---------------------------------------------

// FR-005: a locally-published message goes to relay downstream AND Active
// publisher targets (not AwaitingAccept ones); a relayed message goes to relay
// downstream ONLY.
#[test]
fn fanout_splits_on_origin() {
    let mut state = node_state("self", HashSet::from([topic("t1")]));
    with_downstream(&mut state, "b", "t1"); // relay downstream
    with_publisher_target(&mut state, "c", "t1", LinkState::Active);
    with_publisher_target(&mut state, "d", "t1", LinkState::AwaitingAccept);

    // Local publish: b (relay) + c (Active publisher target); d is pending.
    let effects = apply(
        &mut state,
        Event::Publish(signed(signed_ping(&signer(), topic("t1"), 1))),
    );
    let targets: Vec<PeerId> = signed_sends(&effects)
        .into_iter()
        .map(|(to, _)| to)
        .collect();
    assert_eq!(
        sorted_peers(targets),
        vec![peer("b"), peer("c")],
        "local origin rides relay + Active publisher links",
    );

    // Relayed message (arrives from an Active relay upstream): relay only.
    with_active_upstream(&mut state, "a", "t1");
    let effects = apply(&mut state, payload_from("a", "t1", 2));
    let targets: Vec<PeerId> = signed_sends(&effects)
        .into_iter()
        .map(|(to, _)| to)
        .collect();
    assert_eq!(
        sorted_peers(targets),
        vec![peer("b")],
        "peer origin never crosses publisher links",
    );
}

// FR-011: a peer reachable as BOTH a relay downstream and an Active publisher
// target receives exactly one copy of a local publish.
#[test]
fn dual_kind_peer_receives_one_send() {
    let mut state = node_state("self", HashSet::from([topic("t1")]));
    with_downstream(&mut state, "b", "t1");
    with_publisher_target(&mut state, "b", "t1", LinkState::Active);

    let effects = apply(
        &mut state,
        Event::Publish(signed(signed_ping(&signer(), topic("t1"), 3))),
    );
    let sends = signed_sends(&effects);
    assert_eq!(sends.len(), 1, "one send for the dual-kind peer");
    assert_eq!(sends[0].0, peer("b"));
}

// The topic-removal cascade clears publisher entries too (FR-015 tail).
#[test]
fn topic_removal_cascades_over_publisher_links() {
    let mut state = node_state("self", HashSet::from([topic("t1")]));
    with_upstream_publisher(&mut state, "a", "t1");
    with_publisher_target(&mut state, "b", "t1", LinkState::Active);

    apply(
        &mut state,
        Event::TopicRegistryUpdate(TopicRegistryEvent::Removed { topic: topic("t1") }),
    );

    assert!(state.upstream_publishers().is_empty());
    assert!(state.downstream_publishers().is_empty());
}

// ---- US3 (M5): union fan-out -------------------------------------------------

// FR-007/011: the ForwardToAll fan-out sends EVERY held message — peer origin
// included — over relay downstream and Active publisher links, deduplicated.
#[test]
fn all_links_fanout_unions_both_kinds_for_any_origin() {
    let mut state = NodeState::new(
        peer("self"),
        BTreeSet::from([topic("t1")]),
        0,
        Arc::new(TestVerifier),
        alias_signer("self"),
        NodeStrategies::relay_only(strategy(), Arc::new(AcceptFromAllCandidates)),
        Arc::new(ForwardToAll),
    );
    state
        .registered_topics
        .insert(topic("t1"), TopicEntry::from_publishers(BTreeSet::new()));
    with_active_upstream(&mut state, "a", "t1"); // relay source
    with_downstream(&mut state, "b", "t1"); // relay destination
    with_publisher_target(&mut state, "c", "t1", LinkState::Active);
    with_publisher_target(&mut state, "d", "t1", LinkState::AwaitingAccept);
    with_publisher_target(&mut state, "b", "t1", LinkState::Active); // dual-kind peer

    // A relayed (peer-origin) message from a: forwarded over b (relay, once,
    // despite the coexisting publisher link) AND c (Active publisher target);
    // never d (pending), never back to a (split-horizon).
    let effects = apply(&mut state, payload_from("a", "t1", 30));
    let targets: Vec<PeerId> = signed_sends(&effects)
        .into_iter()
        .map(|(to, _)| to)
        .collect();
    assert_eq!(
        sorted_peers(targets),
        vec![peer("b"), peer("c")],
        "peer-origin traffic rides publisher links under all-links, deduplicated",
    );
}
