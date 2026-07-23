use super::super::*;
use super::*;

// FR-001 / US2-AS1: subscribed topic + valid signature => recorded, in
// order, with no effects and no I/O.
#[test]
fn valid_messages_recorded_in_processing_order() {
    let t1 = topic("t1");
    let mut state = state_subscribed(vec![t1.clone()]);
    // Establishment preamble: both senders are Active upstreams on t1, so
    // their payload passes the connection gate (FR-016).
    with_active_upstream(&mut state, "a", "t1");
    with_active_upstream(&mut state, "b", "t1");
    let s = signer();
    let m1 = signed_ping(&s, t1.clone(), 1);
    let m2 = signed_ping(&s, t1.clone(), 2);

    let effects = apply(
        &mut state,
        Event::MessageReceived {
            from: peer("a"),
            message: m1.clone(),
        },
    );
    assert!(effects.is_empty(), "recording produces no effects");
    let snap = state.received_snapshot();
    assert_eq!(snap.len(), 1);
    assert_eq!(snap[0].origin, Origin::Peer(peer("a")));
    assert_eq!(snap[0].message, m1);

    let effects = apply(
        &mut state,
        Event::MessageReceived {
            from: peer("b"),
            message: m2.clone(),
        },
    );
    assert!(effects.is_empty());
    let snap = state.received_snapshot();
    assert_eq!(snap.len(), 2, "second delivery appended");
    assert_eq!(snap[1].origin, Origin::Peer(peer("b")));
    assert_eq!(snap[1].message, m2);
}

// FR-002 / US2-AS2: off-topic message leaves state unchanged.
#[test]
fn off_topic_message_leaves_state_unchanged() {
    let t1 = topic("t1");
    let mut state = state_subscribed(vec![t1.clone()]);
    // a is Active on both topics — both payloads pass the gate, so this
    // genuinely exercises the subscription filter behind it (t2 is the
    // off-topic one, dropped by subscription, not by the gate).
    with_active_upstream(&mut state, "a", "t1");
    with_active_upstream(&mut state, "a", "t2");
    let s = signer();

    // One accepted delivery first, so "unchanged" is asserted against a
    // non-empty record.
    apply(
        &mut state,
        Event::MessageReceived {
            from: peer("a"),
            message: signed_ping(&s, t1, 1),
        },
    );
    let before = state.received_snapshot();

    let effects = apply(
        &mut state,
        Event::MessageReceived {
            from: peer("a"),
            message: signed_ping(&s, topic("t2"), 2),
        },
    );
    assert!(effects.is_empty());
    assert_eq!(state.received_snapshot(), before, "off-topic drop");
}

// FR-003 / FR-017: subscribed topic but invalid signature over an Active
// upstream => dropped AND severed (the signature failure past every earlier
// check is misbehavior). Detailed severance coverage is in the T021 block;
// this is the 003-era receive test, updated for the connection model.
#[test]
fn invalid_signature_message_dropped() {
    let t1 = topic("t1");
    let mut state = state_subscribed(vec![t1.clone()]);
    // a is Active on t1, so the tampered payload passes the gate and reaches
    // the signature check.
    with_active_upstream(&mut state, "a", "t1");
    let s = signer();

    let effects = apply(
        &mut state,
        Event::MessageReceived {
            from: peer("a"),
            message: tampered_ping(&s, t1, 1),
        },
    );
    assert_eq!(
        misbehaved(&effects),
        vec![(peer("a"), topic("t1"), "invalid_signature")],
        "tampered over an Active upstream severs",
    );
    assert!(
        state.received_snapshot().is_empty(),
        "tampered message never recorded"
    );
}

// Edge case: an empty subscription set drops every inbound message.
#[test]
fn empty_subscription_set_drops_everything() {
    let mut state = state_subscribed(vec![]);
    // a is Active on t1, so the payloads pass the gate and are dropped by
    // the (empty) subscription filter — the behavior under test.
    with_active_upstream(&mut state, "a", "t1");
    let s = signer();

    for n in 0..3 {
        let effects = apply(
            &mut state,
            Event::MessageReceived {
                from: peer("a"),
                message: signed_ping(&s, topic("t1"), n),
            },
        );
        assert!(effects.is_empty());
    }
    assert!(state.received_snapshot().is_empty());
}

// US2-AS3: same initial state + same event sequence => same final state.
#[test]
fn transition_is_deterministic() {
    let t1 = topic("t1");
    let s = signer();
    let script = || {
        vec![
            Event::MessageReceived {
                from: peer("a"),
                message: signed_ping(&s, t1.clone(), 1),
            },
            Event::MessageReceived {
                from: peer("b"),
                message: signed_ping(&s, topic("t2"), 2),
            },
            Event::MessageReceived {
                from: peer("b"),
                message: tampered_ping(&s, t1.clone(), 3),
            },
            Event::MessageReceived {
                from: peer("c"),
                message: signed_ping(&s, t1.clone(), 4),
            },
        ]
    };

    // Both senders Active on the topics they use, so the script exercises
    // the full post-gate chain identically across the two runs.
    let seed = |state: &mut NodeState| {
        with_active_upstream(state, "a", "t1");
        with_active_upstream(state, "b", "t1");
        with_active_upstream(state, "b", "t2");
        with_active_upstream(state, "c", "t1");
    };

    // The tampered (b, t1) event severs that upstream (returning a
    // Misbehaved effect), so the per-step effects are not all empty; the
    // determinism claim is that the same script yields the same final state.
    let mut first = state_subscribed(vec![t1.clone()]);
    seed(&mut first);
    for event in script() {
        apply(&mut first, event);
    }
    let mut second = state_subscribed(vec![t1.clone()]);
    seed(&mut second);
    for event in script() {
        apply(&mut second, event);
    }

    assert_eq!(first.received_snapshot(), second.received_snapshot());
    assert_eq!(
        sorted_pairs(
            first
                .upstream_relays()
                .into_iter()
                .map(|(p, t, _)| (p, t))
                .collect()
        ),
        sorted_pairs(
            second
                .upstream_relays()
                .into_iter()
                .map(|(p, t, _)| (p, t))
                .collect()
        ),
        "the severed (b, t1) upstream is gone in both runs",
    );
    let sorted = |mut v: Vec<TopicId>| {
        v.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        v
    };
    assert_eq!(
        sorted(first.subscriptions_snapshot()),
        sorted(second.subscriptions_snapshot())
    );
}

// A self membership update changes which subsequent messages are accepted —
// the transition reads the current subscription state, not a snapshot. The
// subscription set is derived from the node's own entry on the membership
// stream; there is no local subscribe mutator (ADR 0013/0014/0015).
#[test]
fn subscription_change_affects_subsequent_transitions() {
    let t1 = topic("t1");
    let mut state = state_subscribed(vec![]); // self_id = "self", empty subscriptions
                                              // a is an Active upstream on t1 throughout — the gate is open; what
                                              // changes across the test is the subscription filter behind it.
    with_active_upstream(&mut state, "a", "t1");
    let s = signer();

    apply(
        &mut state,
        Event::MessageReceived {
            from: peer("a"),
            message: signed_ping(&s, t1.clone(), 1),
        },
    );
    assert!(state.received_snapshot().is_empty(), "not subscribed yet");

    // Chain order (strict drop): t1 is registered FIRST, then the node's own
    // entry arrives on the membership stream → admitted → t1 is now
    // effective and subsequent messages are accepted.
    apply(&mut state, reg_open("t1"));
    apply(
        &mut state,
        Event::MembershipUpdate(MembershipEvent::joined("self", ["t1"])),
    );
    apply(
        &mut state,
        Event::MessageReceived {
            from: peer("a"),
            message: signed_ping(&s, t1, 2),
        },
    );
    assert_eq!(state.received_snapshot().len(), 1, "subscribed now");
}
