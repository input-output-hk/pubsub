use super::super::*;
use super::*;

// US3 / FR-013/015/016: MembershipUpdate folds into per-topic candidate
// sets; the node's own id is excluded; the transition returns no effects.
#[test]
fn membership_updates_fold_into_candidates_excluding_self() {
    let mut state = state_subscribed(vec![topic("t1"), topic("t2")]); // self_id = "self"
    let script = MembershipScript::new()
        .joined("a", ["t1"])
        .joined("b", ["t1", "t2"])
        .joined("self", ["t1"]) // own id — must be ignored
        .topics_changed("a", ["t2"], ["t1"])
        .left("b");
    for ev in script {
        assert!(apply(&mut state, Event::MembershipUpdate(ev)).is_empty());
    }
    // a moved t1->t2; b left; self never added.
    assert!(state.candidates_snapshot(&topic("t1")).is_empty());
    assert_eq!(state.candidates_snapshot(&topic("t2")), vec![peer("a")]);
}

// US2 / FR-014, SC-003: effective subscriptions = subscriptions ∩ registered.
// A subscribed topic that is not a registered topic is excluded.
#[test]
fn subscriptions_are_subscribed_intersect_registered() {
    let mut state = node_state("self", HashSet::new());
    // Topic registry registers only `weather`; membership declares both.
    apply(&mut state, reg_open("weather"));
    apply(
        &mut state,
        Event::MembershipUpdate(MembershipEvent::joined("self", ["weather", "ghosttopic"])),
    );
    assert_eq!(
        sorted(state.subscriptions_snapshot()),
        vec![topic("weather")],
        "ghosttopic is subscribed but not registered → excluded",
    );
}

// SC-001/SC-008/FR-003: a self-subscription naming an unregistered topic is
// strict-dropped (never enters the set); registering it later does NOT
// promote it — a fresh membership event is required.
#[test]
fn strict_drop_self_no_auto_promotion() {
    let mut state = node_state("self", HashSet::new());
    apply(&mut state, reg_open("weather"));
    apply(
        &mut state,
        Event::MembershipUpdate(MembershipEvent::joined("self", ["weather", "ghost"])),
    );
    assert_eq!(
        sorted(state.subscriptions_snapshot()),
        vec![topic("weather")],
        "ghost is unregistered → strict-dropped, never in the set",
    );
    assert_invariants(&state);

    apply(&mut state, reg_open("ghost"));
    assert_eq!(
        sorted(state.subscriptions_snapshot()),
        vec![topic("weather")],
        "registering ghost later must NOT auto-promote the dropped subscription",
    );

    apply(
        &mut state,
        Event::MembershipUpdate(MembershipEvent::topics_changed("self", ["ghost"], [])),
    );
    assert_eq!(
        sorted(state.subscriptions_snapshot()),
        vec![topic("ghost"), topic("weather")],
    );
    assert_invariants(&state);
}

// SC-008/FR-003a: candidate gating — a candidate (other node) on an
// unregistered topic is not recorded; candidate topics ⊆ registered.
#[test]
fn candidate_gating_drops_unregistered() {
    let mut state = node_state("self", HashSet::new());
    apply(&mut state, reg_open("weather"));
    apply(
        &mut state,
        Event::MembershipUpdate(MembershipEvent::joined("b", ["weather", "ghost"])),
    );
    assert_eq!(
        state.candidates_snapshot(&topic("weather")),
        vec![peer("b")]
    );
    assert!(
        state.candidates_snapshot(&topic("ghost")).is_empty(),
        "candidate on an unregistered topic is not recorded",
    );
    assert_invariants(&state);
}

// SC-010/FR-008: defensive fold — PublishersChanged for an unregistered
// topic does NOT create it (no or_default); only Registered creates.
#[test]
fn defensive_fold_publishers_changed_does_not_create() {
    let mut state = node_state("self", HashSet::new());
    apply(
        &mut state,
        Event::TopicRegistryUpdate(TopicRegistryEvent::PublishersChanged {
            topic: topic("ghost"),
            added: BTreeSet::from([pk(b"k1")]),
            removed: BTreeSet::new(),
        }),
    );
    assert!(
        !state.is_registered(&topic("ghost")),
        "PublishersChanged on an unknown topic must not create it",
    );
}

// SC-002/SC-003/FR-002: atomic cascade — a Removed clears the topic from
// subscriptions, candidates, AND both connection structures together.
#[test]
fn removed_cascades_to_subscriptions_candidates_and_connections() {
    let mut state = node_state("self", HashSet::new());
    apply(&mut state, reg_open("weather"));
    apply(
        &mut state,
        Event::MembershipUpdate(MembershipEvent::joined("self", ["weather"])),
    );
    apply(
        &mut state,
        Event::MembershipUpdate(MembershipEvent::joined("b", ["weather"])),
    );
    // Hold a connection in each role on weather.
    with_active_upstream(&mut state, "b", "weather");
    state.downstream.insert((peer("c"), topic("weather")));
    assert_eq!(state.subscriptions_snapshot(), vec![topic("weather")]);
    assert_eq!(
        state.candidates_snapshot(&topic("weather")),
        vec![peer("b")]
    );

    apply(
        &mut state,
        Event::TopicRegistryUpdate(TopicRegistryEvent::Removed {
            topic: topic("weather"),
        }),
    );
    assert!(
        state.subscriptions_snapshot().is_empty(),
        "cascade: subscription cleared"
    );
    assert!(
        state.candidates_snapshot(&topic("weather")).is_empty(),
        "cascade: candidates cleared",
    );
    assert_eq!(
        upstream_state(&state, "b", "weather"),
        None,
        "cascade: upstream cleared"
    );
    assert!(
        !has_downstream(&state, "c", "weather"),
        "cascade: downstream cleared",
    );
    assert!(
        !state.is_registered(&topic("weather")),
        "cascade: projection cleared"
    );
    assert_invariants(&state);
}

// (The 013 subscribe-before-register-then-promote test is retired with 013
// SC-004: under 014 strict drop there is no promotion. Strict drop +
// no-promotion is covered by `strict_drop_self_no_auto_promotion`, and the
// removal cascade by `removed_cascades_to_subscriptions_and_candidates`.)

// US2 / SC-004 (014): a topic removed from the registry cascades out of the
// subscription set (register-first, then remove).
#[test]
fn removing_a_topic_makes_it_ineffective() {
    let mut state = node_state("self", HashSet::new());
    // Chain order: register weather, then the node subscribes to it.
    apply(&mut state, reg_open("weather"));
    apply(
        &mut state,
        Event::MembershipUpdate(MembershipEvent::joined("self", ["weather"])),
    );
    assert_eq!(state.subscriptions_snapshot(), vec![topic("weather")]);
    // Removal cascades it out of the subscription set.
    apply(
        &mut state,
        Event::TopicRegistryUpdate(TopicRegistryEvent::Removed {
            topic: topic("weather"),
        }),
    );
    assert!(
        state.subscriptions_snapshot().is_empty(),
        "removed → cascaded out of the subscription set",
    );
}

// US2 / FR-013: handle_topic_registry_update folds the registered-topics
// projection across a scripted register → publishers-changed → remove
// sequence (declarative TopicRegistryScript); every apply returns no effects.
#[test]
fn topic_registry_script_folds_projection() {
    let mut state = state_subscribed(vec![topic("weather")]);
    // state_subscribed already registered weather open; drive a script that
    // re-registers it with a publisher, rotates publishers, and removes an
    // unrelated topic.
    let script = TopicRegistryScript::new()
        .registered("weather", [pk(b"k1")])
        .publishers_changed("weather", [pk(b"k4")], [pk(b"k1")])
        .removed("other");
    for ev in script {
        assert!(apply(&mut state, Event::TopicRegistryUpdate(ev)).is_empty());
    }
    // weather stays registered (so still effective); the no-op remove of an
    // unregistered "other" is harmless.
    assert_eq!(state.subscriptions_snapshot(), vec![topic("weather")],);
}

// US3 / FR-015, SC-005: a non-open topic accepts only authorized publishers;
// an open topic accepts any. Authorization precedes signature verification —
// an unauthorized publisher with a *valid* signature is still dropped.
#[test]
fn publisher_authorization_restricted_then_open() {
    let authorized = signer();
    let outsider = signer_seeded([9u8; 32]);
    let weather = topic("weather");
    let mut state = node_state("self", HashSet::from([weather.clone()]));
    // relay is an Active upstream on weather — the gate is open; what this
    // test exercises behind it is publisher authorization.
    with_active_upstream(&mut state, "relay", "weather");
    // weather restricted to the authorized signer's key.
    apply(
        &mut state,
        Event::TopicRegistryUpdate(TopicRegistryEvent::Registered {
            topic: weather.clone(),
            publishers: BTreeSet::from([authorized.public_key()]),
        }),
    );

    // Authorized publisher, valid signature → recorded.
    apply(
        &mut state,
        Event::MessageReceived {
            from: peer("relay"),
            message: signed_ping(&authorized, weather.clone(), 1),
        },
    );
    assert_eq!(
        state.received_snapshot().len(),
        1,
        "authorized publisher accepted",
    );

    // Unauthorized publisher with a VALID signature → dropped (authorization
    // precedes verification).
    apply(
        &mut state,
        Event::MessageReceived {
            from: peer("relay"),
            message: signed_ping(&outsider, weather.clone(), 2),
        },
    );
    assert_eq!(
        state.received_snapshot().len(),
        1,
        "unauthorized publisher dropped despite a valid signature",
    );

    // Re-register weather OPEN → the outsider is now accepted.
    apply(&mut state, reg_open("weather"));
    apply(
        &mut state,
        Event::MessageReceived {
            from: peer("relay"),
            message: signed_ping(&outsider, weather, 3),
        },
    );
    assert_eq!(
        state.received_snapshot().len(),
        2,
        "open topic accepts any publisher",
    );
}

// US3 / FR-015: authorization is ordered BEFORE verification — an authorized
// publisher's *tampered* (invalid-signature) message passes the authorization
// check but is dropped at verification.
#[test]
fn authorized_but_tampered_message_dropped_at_verification() {
    let authorized = signer();
    let weather = topic("weather");
    let mut state = node_state("self", HashSet::from([weather.clone()]));
    // relay is an Active upstream on weather — the payload passes the gate
    // and the authorized publisher passes authorization, so it reaches (and
    // is dropped at) signature verification.
    with_active_upstream(&mut state, "relay", "weather");
    apply(
        &mut state,
        Event::TopicRegistryUpdate(TopicRegistryEvent::Registered {
            topic: weather.clone(),
            publishers: BTreeSet::from([authorized.public_key()]),
        }),
    );
    apply(
        &mut state,
        Event::MessageReceived {
            from: peer("relay"),
            message: tampered_ping(&authorized, weather, 1),
        },
    );
    assert!(
        state.received_snapshot().is_empty(),
        "authorized publisher but invalid signature → dropped at verify",
    );
}
