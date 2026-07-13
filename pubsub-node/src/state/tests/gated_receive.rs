use super::super::*;
use super::*;

// US2-AS1 / FR-016: a validly-signed payload from an Active upstream is
// recorded — the post-connection receive path is unchanged.
#[test]
fn payload_over_active_upstream_is_recorded() {
    let mut state = state_subscribed(vec![topic("t1")]);
    with_active_upstream(&mut state, "b", "t1");

    let effects = apply(&mut state, payload_from("b", "t1", 1));
    assert!(effects.is_empty());
    assert_eq!(state.received_snapshot().len(), 1, "admitted and recorded");
}

// US2-AS2 / SC-002: a payload from a sender with no connection is dropped
// (not_connected) — pre-connection delivery is retired.
#[test]
fn payload_without_connection_is_dropped() {
    let mut state = state_subscribed(vec![topic("t1")]);
    // No upstream seeded.
    let effects = apply(&mut state, payload_from("b", "t1", 1));
    assert!(effects.is_empty());
    assert!(
        state.received_snapshot().is_empty(),
        "no Active upstream → not_connected drop",
    );
}

// US2-AS2 / SC-002: an AwaitingAccept connection does not admit payload —
// only Active does.
#[test]
fn payload_over_awaiting_accept_is_dropped() {
    let mut state = state_subscribed(vec![topic("t1")]);
    state.insert_link_for_test(
        peer("b"),
        topic("t1"),
        LinkRole::Relay,
        LinkDirection::Out,
        LinkState::AwaitingAccept,
    );

    let effects = apply(&mut state, payload_from("b", "t1", 1));
    assert!(effects.is_empty());
    assert!(
        state.received_snapshot().is_empty(),
        "pending connection admits nothing",
    );
}

// US2-AS3: connections are per-topic — an Active upstream for t1 does not
// admit the same peer's traffic on t2.
#[test]
fn connection_is_per_topic() {
    let mut state = state_subscribed(vec![topic("t1"), topic("t2")]);
    with_active_upstream(&mut state, "b", "t1");

    // t1 from b → admitted; t2 from b → dropped (no connection for t2),
    // even though t2 is subscribed and registered.
    apply(&mut state, payload_from("b", "t1", 1));
    let effects = apply(&mut state, payload_from("b", "t2", 2));
    assert!(effects.is_empty());
    assert_eq!(
        state.received_snapshot().len(),
        1,
        "only t1 admitted; t2 has no connection",
    );
}

// US2-AS4 / FR-019: the gate is the FIRST check; the merged chain after it
// is unchanged — a tampered payload over an Active upstream reaches
// signature verification, where it is dropped and (US3, FR-017) severs the
// connection.
#[test]
fn gate_first_then_signature_check_unchanged() {
    let mut state = state_subscribed(vec![topic("t1")]);
    with_active_upstream(&mut state, "b", "t1");

    let effects = apply(&mut state, tampered_payload_from("b", "t1", 1));
    assert_eq!(
        misbehaved(&effects),
        vec![(peer("b"), topic("t1"), "invalid_signature")],
        "admitted by the gate, then severed at signature",
    );
    assert!(
        state.received_snapshot().is_empty(),
        "tampered not recorded"
    );
}

// US2-AS4 / FR-019: a payload that passes the gate but is off the
// subscription set still drops by the subscription filter (the gate keys on
// (sender, topic) independent of subscription; the filter runs after it).
#[test]
fn gate_first_then_subscription_filter_unchanged() {
    // Subscribed+registered only for t1; seed an Active upstream for the
    // unsubscribed t2 (an own-topic-drift stale state, S4).
    let mut state = state_subscribed(vec![topic("t1")]);
    with_active_upstream(&mut state, "b", "t2");

    let effects = apply(&mut state, payload_from("b", "t2", 1));
    assert!(effects.is_empty());
    assert!(
        state.received_snapshot().is_empty(),
        "passes the gate but t2 is not subscribed → topic_not_subscribed drop",
    );
}

// ---- 015: the publishing-link receive gate (ADR 0033) ----------------------

/// Seed an inbound publishing link `(peer, topic)` directly — the declarative
/// stand-in for a full publish-intent handshake.
fn with_inbound_publish_link(state: &mut NodeState, peer_alias: &str, t: &str) {
    state.insert_link_for_test(
        peer(peer_alias),
        topic(t),
        LinkRole::Publisher,
        LinkDirection::In,
        LinkState::Active,
    );
}

// 015 US2 / FR-004 dual: a payload delivered over an inbound publishing link is
// admitted when the deliverer IS the publisher (`payload_from` signs under the
// deliverer's own key).
#[test]
fn own_publish_over_inbound_publish_link_is_recorded() {
    let mut state = state_subscribed(vec![topic("t1")]);
    with_inbound_publish_link(&mut state, "p", "t1");

    let effects = apply(&mut state, payload_from("p", "t1", 1));
    assert!(effects.is_empty(), "no downstream → no forwards");
    let snap = state.received_snapshot();
    assert_eq!(snap.len(), 1, "admitted over the publishing link");
    assert_eq!(snap[0].origin, Origin::Peer(peer("p")));
}

// 015 R5 / ADR 0033: a payload from a DIFFERENT publisher delivered over a
// publishing link is a relay attempt the link's role forbids — dropped
// (relay_over_publish_link), never recorded.
#[test]
fn foreign_publisher_over_publish_link_is_dropped() {
    let mut state = state_subscribed(vec![topic("t1")]);
    with_inbound_publish_link(&mut state, "p", "t1");

    // A message published (signed) by "q", delivered by "p" over p's
    // publishing link: rewrap q's signed payload in a frame from p.
    let Event::MessageReceived { message, .. } = payload_from("q", "t1", 1) else {
        unreachable!("payload_from yields MessageReceived")
    };
    let effects = apply(
        &mut state,
        Event::MessageReceived {
            from: peer("p"),
            message,
        },
    );
    assert!(effects.is_empty());
    assert!(
        state.received_snapshot().is_empty(),
        "publishing links do not relay — foreign-publisher payload dropped",
    );
}

// 015 FR-011/SC-005: the same published message arriving over BOTH a publishing
// link and a relay upstream is recorded once — content-hash dedup holds across
// link roles.
#[test]
fn duplicate_across_publish_and_relay_paths_is_recorded_once() {
    let mut state = state_subscribed(vec![topic("t1")]);
    with_inbound_publish_link(&mut state, "p", "t1");
    with_active_upstream(&mut state, "x", "t1");

    // First copy: p pushes its own publish over the publishing link.
    apply(&mut state, payload_from("p", "t1", 7));
    assert_eq!(state.received_snapshot().len(), 1);

    // Second copy: the identical message relayed by x over the relay upstream.
    let Event::MessageReceived { message, .. } = payload_from("p", "t1", 7) else {
        unreachable!("payload_from yields MessageReceived")
    };
    let effects = apply(
        &mut state,
        Event::MessageReceived {
            from: peer("x"),
            message,
        },
    );
    assert!(effects.is_empty(), "duplicate is not re-fanned");
    assert_eq!(
        state.received_snapshot().len(),
        1,
        "content-hash dedup suppresses the second copy across roles",
    );
}

// ---- 015 / ADR 0035: the M5 receive-gate policy (any-verified) ------------

fn state_with_any_verified(topics: Vec<TopicId>) -> NodeState {
    let mut state = NodeState::new(
        peer("self"),
        topics.iter().cloned().collect(),
        0, // genesis: the default initial epoch nonce
        Arc::new(TestVerifier),
        alias_signer("self"),
        strategy(),
        Arc::new(ForwardToAll),
        Arc::new(AcceptFromAllCandidates),
        Arc::new(NoLinks),
        Arc::new(AcceptFromAllCandidates),
        PublishInAdmission::AnyVerified,
    );
    for t in topics {
        apply(
            &mut state,
            Event::TopicRegistryUpdate(TopicRegistryEvent::Registered {
                topic: t,
                publishers: BTreeSet::new(),
            }),
        );
        // Subscription via the membership stream (registered-topics gate).
    }
    apply(&mut state, membership_joined("self", ["t1"]));
    state
}

// ADR 0035 / M5: under any-verified, a FOREIGN-publisher payload delivered
// over an inbound standing link is admitted and recorded — the k_out links
// relay everything.
#[test]
fn any_verified_admits_foreign_publisher_over_publish_link() {
    let mut state = state_with_any_verified(vec![topic("t1")]);
    with_inbound_publish_link(&mut state, "p", "t1");

    let Event::MessageReceived { message, .. } = payload_from("q", "t1", 1) else {
        unreachable!("payload_from yields MessageReceived")
    };
    apply(
        &mut state,
        Event::MessageReceived {
            from: peer("p"),
            message,
        },
    );
    let snap = state.received_snapshot();
    assert_eq!(snap.len(), 1, "admitted under any-verified");
    assert_eq!(snap[0].origin, Origin::Peer(peer("p")));
}

// ADR 0035: the relaxed gate does not relax severance — an invalidly-signed
// payload over the standing link still severs it.
#[test]
fn any_verified_still_severs_on_invalid_signature() {
    let mut state = state_with_any_verified(vec![topic("t1")]);
    with_inbound_publish_link(&mut state, "p", "t1");

    let effects = apply(&mut state, tampered_payload_from("p", "t1", 1));
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::Misbehaved { .. })),
        "misbehaviour raised",
    );
    assert!(
        state.links_snapshot().is_empty(),
        "the admitting standing link is severed",
    );
}
