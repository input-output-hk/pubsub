use super::super::*;
use super::*;

// US3-AS1 / FR-017: a tampered payload over an Active upstream (having passed
// the gate, subscription, registration, authorization) severs that upstream
// — entry removed, one Misbehaved effect, no Send, nothing recorded.
#[test]
fn tampered_over_active_upstream_severs() {
    let mut state = state_subscribed(vec![topic("t1")]);
    with_active_upstream(&mut state, "b", "t1");

    let effects = apply(&mut state, tampered_payload_from("b", "t1", 1));

    assert_eq!(upstream_state(&state, "b", "t1"), None, "upstream removed");
    assert_eq!(
        misbehaved(&effects),
        vec![(peer("b"), topic("t1"), "invalid_signature")],
    );
    assert!(
        !has_send(&effects),
        "severance is silent — no Terminated sent"
    );
    assert!(
        state.received_snapshot().is_empty(),
        "tampered never recorded"
    );
}

// FR-017: severance fires only *past* authorization — an authorized
// publisher's tampered message over an Active upstream is severed.
#[test]
fn severance_fires_past_authorization() {
    let weather = topic("weather");
    let mut state = node_state("self", HashSet::from([weather.clone()]));
    // weather restricted to b's key (the publisher tampered_payload_from
    // signs under), so authorization passes and the signature check is
    // reached.
    apply(
        &mut state,
        Event::TopicRegistryUpdate(TopicRegistryEvent::Registered {
            topic: weather.clone(),
            publishers: BTreeSet::from([alias_public("b")]),
        }),
    );
    with_active_upstream(&mut state, "b", "weather");

    let effects = apply(&mut state, tampered_payload_from("b", "weather", 1));
    assert_eq!(upstream_state(&state, "b", "weather"), None, "severed");
    assert_eq!(
        misbehaved(&effects),
        vec![(peer("b"), weather, "invalid_signature")],
    );
}

// US3-AS3: an invalid-signature message from a peer with no Active connection
// is a plain not_connected drop — never a severance (a forged sender must not
// cost the genuine peer anything).
#[test]
fn no_severance_without_connection() {
    let mut state = state_subscribed(vec![topic("t1")]);
    // No upstream seeded.
    let effects = apply(&mut state, tampered_payload_from("b", "t1", 1));
    assert!(
        misbehaved(&effects).is_empty(),
        "no connection → no severance"
    );
    assert!(effects.is_empty());
}

// US3-AS4 / FR-018: a tampered message dropped by an *earlier* check (not
// subscribed, not registered, not authorized) never reaches the signature
// verdict, so it never severs and leaves the entry intact.
#[test]
fn no_severance_when_an_earlier_check_fails() {
    // (a) topic not subscribed — Active upstream on an unsubscribed t2.
    let mut state = state_subscribed(vec![topic("t1")]);
    with_active_upstream(&mut state, "b", "t2");
    let effects = apply(&mut state, tampered_payload_from("b", "t2", 1));
    assert!(
        misbehaved(&effects).is_empty(),
        "not subscribed → no severance"
    );
    assert_eq!(
        upstream_state(&state, "b", "t2"),
        Some(UpstreamState::Active),
        "entry intact",
    );

    // (b) topic not registered — a subscribed-but-unregistered topic, which
    // 014's invariant normally makes unreachable (strict drop); constructed
    // directly here to confirm the receive-path registration guard still
    // drops (no severance) defensively if the invariant is ever violated.
    let mut state = node_state("self", HashSet::new());
    state.subscriptions.insert(topic("t1")); // bypass strict drop; t1 left unregistered
    with_active_upstream(&mut state, "b", "t1");
    let effects = apply(&mut state, tampered_payload_from("b", "t1", 1));
    assert!(
        misbehaved(&effects).is_empty(),
        "not registered → no severance"
    );
    assert_eq!(
        upstream_state(&state, "b", "t1"),
        Some(UpstreamState::Active)
    );

    // (c) publisher not authorized — restricted topic, b's key not in the set.
    let weather = topic("weather");
    let mut state = node_state("self", HashSet::from([weather.clone()]));
    apply(
        &mut state,
        Event::TopicRegistryUpdate(TopicRegistryEvent::Registered {
            topic: weather.clone(),
            publishers: BTreeSet::from([alias_public("someone-else")]),
        }),
    );
    with_active_upstream(&mut state, "b", "weather");
    let effects = apply(&mut state, tampered_payload_from("b", "weather", 1));
    assert!(
        misbehaved(&effects).is_empty(),
        "not authorized → no severance"
    );
    assert_eq!(
        upstream_state(&state, "b", "weather"),
        Some(UpstreamState::Active),
    );
}

// US3-AS2 / SC-003: after severance, a subsequent *valid* message from the
// same peer on that topic is dropped not_connected (the connection is gone).
#[test]
fn post_severance_valid_message_is_not_connected() {
    let mut state = state_subscribed(vec![topic("t1")]);
    with_active_upstream(&mut state, "b", "t1");
    apply(&mut state, tampered_payload_from("b", "t1", 1)); // severs
    assert_eq!(upstream_state(&state, "b", "t1"), None);

    let effects = apply(&mut state, payload_from("b", "t1", 2));
    assert!(effects.is_empty());
    assert!(
        state.received_snapshot().is_empty(),
        "a valid message over the severed connection is dropped not_connected",
    );
}

// SC-003: severance is scoped to the one (peer, topic) — the offender's
// other-topic connection and other peers' connections are untouched.
#[test]
fn severance_isolates_other_topics_and_peers() {
    let mut state = state_subscribed(vec![topic("t1"), topic("t2")]);
    with_active_upstream(&mut state, "b", "t1");
    with_active_upstream(&mut state, "b", "t2");
    with_active_upstream(&mut state, "c", "t1");

    apply(&mut state, tampered_payload_from("b", "t1", 1)); // severs (b, t1) only

    assert_eq!(upstream_state(&state, "b", "t1"), None, "severed pair gone");
    assert_eq!(
        upstream_state(&state, "b", "t2"),
        Some(UpstreamState::Active),
        "offender's other topic intact",
    );
    assert_eq!(
        upstream_state(&state, "c", "t1"),
        Some(UpstreamState::Active),
        "other peer intact",
    );
}
