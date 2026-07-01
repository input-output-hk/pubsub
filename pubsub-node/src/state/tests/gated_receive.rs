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
    state
        .upstream
        .insert((peer("b"), topic("t1")), UpstreamState::AwaitingAccept);

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
