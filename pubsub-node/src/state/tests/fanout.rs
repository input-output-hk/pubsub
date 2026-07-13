use super::super::*;
use super::*;

// US1-AS1 / FR-001..004,007,011,014,016: a valid publish records the message
// with `Origin::Local` and fans it out verbatim to every downstream on the
// topic (one `Effect::Send` each, order-insensitive).
#[test]
fn publish_records_local_and_fans_out_to_downstream() {
    let t1 = topic("t1");
    let mut state = state_subscribed(vec![t1.clone()]);
    with_downstream(&mut state, "a", "t1");
    with_downstream(&mut state, "b", "t1");
    let sm = signed(signed_ping(&signer(), t1, 1));

    let effects = handle_publish(&mut state, sm.clone());

    let snap = state.received_snapshot();
    assert_eq!(snap.len(), 1, "published message recorded");
    assert_eq!(snap[0].origin, Origin::Local, "local origin");
    assert_eq!(snap[0].message, Message::Dissemination(sm.clone()));

    let sends = signed_sends(&effects);
    assert_eq!(
        sorted_peers(sends.iter().map(|(p, _)| p.clone()).collect()),
        vec![peer("a"), peer("b")],
        "one forward per downstream on the topic",
    );
    for (_, forwarded) in &sends {
        assert_eq!(*forwarded, sm, "forward is verbatim (no re-sign)");
    }
}

// US1 / FR-016: a publish with no downstream is recorded but produces no
// effects (recording still occurs).
#[test]
fn publish_with_no_downstream_records_without_effects() {
    let t1 = topic("t1");
    let mut state = state_subscribed(vec![t1.clone()]);
    let sm = signed(signed_ping(&signer(), t1, 1));

    let effects = handle_publish(&mut state, sm);

    assert!(effects.is_empty(), "no downstream → no forwards");
    let snap = state.received_snapshot();
    assert_eq!(snap.len(), 1);
    assert_eq!(snap[0].origin, Origin::Local);
}

// US1-AS? / FR-005: proxy/injection — a validly-signed, authorized message
// from a publisher other than the node itself is accepted (publisher_id need
// not be self).
#[test]
fn publish_accepts_proxy_publisher_not_self() {
    let t1 = topic("t1");
    let mut state = state_subscribed(vec![t1.clone()]);
    // A publisher whose key is not the node's own ("self") identity.
    let other = signer_seeded([42u8; 32]);
    let sm = signed(signed_ping(&other, t1, 1));

    let effects = handle_publish(&mut state, sm);

    assert!(effects.is_empty(), "no downstream");
    let snap = state.received_snapshot();
    assert_eq!(snap.len(), 1, "proxy publish accepted (publisher != self)");
    assert_eq!(snap[0].origin, Origin::Local);
}

// US1-AS2..4 / FR-002,003: each failed-check publish is a plain drop — no
// record, no effects, and (the publish-path invariant) NO severance, even
// with downstream present.
#[test]
fn publish_drops_failed_checks_without_record_effects_or_severance() {
    let s = signer();

    // (a) topic not subscribed — downstream on the topic to prove the drop
    // precedes any fan-out.
    let mut state = state_subscribed(vec![topic("t1")]);
    with_downstream(&mut state, "a", "t2");
    let effects = handle_publish(&mut state, signed(signed_ping(&s, topic("t2"), 1)));
    assert!(effects.is_empty(), "not subscribed → no record, no fan-out");
    assert!(state.received_snapshot().is_empty());

    // (b) restricted topic, publisher not authorized.
    let weather = topic("weather");
    let mut state = node_state("self", HashSet::from([weather.clone()]));
    apply(
        &mut state,
        Event::TopicRegistryUpdate(TopicRegistryEvent::Registered {
            topic: weather.clone(),
            publishers: BTreeSet::from([signer_seeded([9u8; 32]).public_key()]),
        }),
    );
    let effects = handle_publish(&mut state, signed(signed_ping(&s, weather, 1)));
    assert!(effects.is_empty(), "unauthorized → dropped");
    assert!(state.received_snapshot().is_empty());

    // (c) invalid signature — a plain drop on the publish path (no upstream
    // to sever, and the publish path never severs).
    let mut state = state_subscribed(vec![topic("t1")]);
    with_downstream(&mut state, "a", "t1");
    let effects = handle_publish(&mut state, signed(tampered_ping(&s, topic("t1"), 1)));
    assert!(
        misbehaved(&effects).is_empty(),
        "invalid-signature publish never severs",
    );
    assert!(effects.is_empty(), "no record, no fan-out");
    assert!(state.received_snapshot().is_empty());
}

// ---- T007: receive-path fan-out + split-horizon (US2, FR-006/007/009) -----

// US2-AS1/AS2/AS5 / FR-006/007/009: a recorded received message is fanned out
// to every downstream on the topic EXCEPT the delivering peer (split-horizon),
// verbatim, and is recorded with `Origin::Peer(deliverer)`.
#[test]
fn received_message_fans_out_to_downstream_excluding_deliverer() {
    let t1 = topic("t1");
    let mut state = state_subscribed(vec![t1.clone()]);
    // b delivers over an Active upstream (the gate). Downstream on t1: b (the
    // deliverer — must be excluded), plus c and d (the forward targets).
    with_active_upstream(&mut state, "b", "t1");
    with_downstream(&mut state, "b", "t1");
    with_downstream(&mut state, "c", "t1");
    with_downstream(&mut state, "d", "t1");
    let sm = signed(signed_ping(&signer(), t1.clone(), 1));

    let effects = apply(
        &mut state,
        Event::MessageReceived {
            from: peer("b"),
            message: Message::Dissemination(sm.clone()),
        },
    );

    // Recorded once, attributed to the delivering peer (US2-AS1).
    let snap = state.received_snapshot();
    assert_eq!(snap.len(), 1, "received message recorded once");
    assert_eq!(
        snap[0].origin,
        Origin::Peer(peer("b")),
        "origin is the delivering peer",
    );

    // Fanned to c and d only — never back to the deliverer b (split-horizon).
    let sends = signed_sends(&effects);
    assert_eq!(
        sorted_peers(sends.iter().map(|(p, _)| p.clone()).collect()),
        vec![peer("c"), peer("d")],
        "forwarded to the other downstream, never back to the deliverer",
    );
    // Verbatim — each forward equals the received message (US2-AS5).
    for (_, forwarded) in &sends {
        assert_eq!(*forwarded, sm, "forward is verbatim (signature unchanged)");
    }
}

// US2-AS3 / FR-009: when the delivering peer is the node's ONLY downstream on
// the topic, split-horizon leaves no targets — recorded, no forwards.
#[test]
fn received_message_sole_downstream_is_deliverer_yields_no_forward() {
    let t1 = topic("t1");
    let mut state = state_subscribed(vec![t1.clone()]);
    with_active_upstream(&mut state, "b", "t1");
    with_downstream(&mut state, "b", "t1"); // b is the only downstream
    let sm = signed(signed_ping(&signer(), t1.clone(), 1));

    let effects = apply(
        &mut state,
        Event::MessageReceived {
            from: peer("b"),
            message: Message::Dissemination(sm),
        },
    );

    assert!(
        signed_sends(&effects).is_empty(),
        "sole downstream is the deliverer → no forward",
    );
    let snap = state.received_snapshot();
    assert_eq!(snap.len(), 1, "still recorded");
    assert_eq!(snap[0].origin, Origin::Peer(peer("b")));
}

// ---- T010: duplicate suppression (US3, FR-012/013/015) --------------------

// US3-AS1 / FR-012: an already-seen message redelivered over an Active
// upstream is dropped (`duplicate`) — not recorded a second time and not
// fanned out again.
#[test]
fn already_seen_received_message_is_dropped_not_refanned() {
    let t1 = topic("t1");
    let mut state = state_subscribed(vec![t1.clone()]);
    with_active_upstream(&mut state, "b", "t1");
    with_downstream(&mut state, "c", "t1"); // a downstream, to prove no re-fan
    let sm = signed(signed_ping(&signer(), t1.clone(), 1));

    // First delivery: recorded and fanned to c.
    let first = apply(
        &mut state,
        Event::MessageReceived {
            from: peer("b"),
            message: Message::Dissemination(sm.clone()),
        },
    );
    assert_eq!(
        state.received_snapshot().len(),
        1,
        "first delivery recorded"
    );
    assert_eq!(signed_sends(&first).len(), 1, "first delivery fans to c");

    // Identical redelivery over the same Active upstream: dropped duplicate.
    let second = apply(
        &mut state,
        Event::MessageReceived {
            from: peer("b"),
            message: Message::Dissemination(sm),
        },
    );
    assert!(
        second.is_empty(),
        "duplicate produces no effects (no re-fan)"
    );
    assert_eq!(
        state.received_snapshot().len(),
        1,
        "duplicate not recorded a second time",
    );
}

// US3 / FR-012, contracts §1.6: a second publish of identical content is
// dropped `duplicate` — confirming the publish path inserts into `seen`.
#[test]
fn republish_identical_content_is_dropped_duplicate() {
    let t1 = topic("t1");
    let mut state = state_subscribed(vec![t1.clone()]);
    with_downstream(&mut state, "a", "t1");
    let sm = signed(signed_ping(&signer(), t1.clone(), 1));

    let first = handle_publish(&mut state, sm.clone());
    assert_eq!(state.received_snapshot().len(), 1, "first publish recorded");
    assert_eq!(signed_sends(&first).len(), 1, "first publish fans to a");

    let second = handle_publish(&mut state, sm);
    assert!(
        second.is_empty(),
        "re-publishing identical content is a duplicate drop",
    );
    assert_eq!(
        state.received_snapshot().len(),
        1,
        "duplicate publish not recorded again",
    );
}

// US3-AS2 / FR-015: dedup spans both paths — a message the node published
// (and thereby seen-marked) is dropped if a peer later relays it back.
#[test]
fn published_message_relayed_back_is_dropped_duplicate() {
    let t1 = topic("t1");
    let mut state = state_subscribed(vec![t1.clone()]);
    with_active_upstream(&mut state, "b", "t1"); // b can deliver to us
    let sm = signed(signed_ping(&signer(), t1.clone(), 1));

    // Publish: recorded locally and seen-marked.
    handle_publish(&mut state, sm.clone());
    assert_eq!(state.received_snapshot().len(), 1, "publish recorded");

    // b relays the same content back over the Active upstream → duplicate.
    let relayed = apply(
        &mut state,
        Event::MessageReceived {
            from: peer("b"),
            message: Message::Dissemination(sm),
        },
    );
    assert!(relayed.is_empty(), "relayed-back copy produces no effects");
    assert_eq!(
        state.received_snapshot().len(),
        1,
        "the relayed-back copy is suppressed (FR-015)",
    );
}

// US3-AS4 / FR-013: no poisoning — an invalid-signature PUBLISH whose `plain`
// hashes identically to a genuine message is a plain drop at verification
// (the dedup gate sits *after* verification, so it is unreached and never
// seen-marks). The genuine message — same content hash — is still recorded.
#[test]
fn invalid_signature_publish_does_not_poison_seen() {
    let t1 = topic("t1");
    let s = signer();
    let mut state = state_subscribed(vec![t1.clone()]);

    let genuine = signed(signed_ping(&s, t1.clone(), 1));
    // Same `plain` (so the same content hash) but a signature that does not
    // verify under the publisher's key — produced by a different signer.
    let impostor = signer_seeded([99u8; 32]);
    let forged = SignedMessage {
        plain: genuine.plain.clone(),
        signature: impostor.sign(&genuine.plain.signed_bytes()),
    };
    assert_eq!(
        MessageHash::of(&forged.plain),
        MessageHash::of(&genuine.plain),
        "the forged copy hashes identically to the genuine message",
    );

    // The forged publish drops at verification (publish never severs) and
    // must NOT seen-mark the shared hash.
    let dropped = handle_publish(&mut state, forged);
    assert!(dropped.is_empty(), "forged publish produces no effects");
    assert!(
        state.received_snapshot().is_empty(),
        "forged publish not recorded",
    );

    // The genuine message — identical content hash — is still recorded: the
    // failed verification did not pre-seed `seen`.
    handle_publish(&mut state, genuine);
    assert_eq!(
        state.received_snapshot().len(),
        1,
        "genuine message recorded; the seen-set was not poisoned",
    );
    assert_eq!(state.received_snapshot()[0].origin, Origin::Local);
}

// ---- 015: origin-restricted forwarding over publishing links (ADR 0033) ----

/// Seed an Active outbound publishing link `(peer, topic)` directly.
fn with_outbound_publish_link(state: &mut NodeState, peer_alias: &str, t: &str) {
    state.insert_link_for_test(
        peer(peer_alias),
        topic(t),
        LinkRole::Publisher,
        LinkDirection::Out,
        LinkState::Active,
    );
}

// 015 US2-AS1 / FR-005/SC-002: a locally-published message goes to the relay
// downstream AND the active outbound publishing links.
#[test]
fn publish_targets_relay_downstream_and_publish_links() {
    let t1 = topic("t1");
    let mut state = state_subscribed(vec![t1.clone()]);
    with_downstream(&mut state, "a", "t1");
    with_outbound_publish_link(&mut state, "b", "t1");
    let sm = signed(signed_ping(&signer(), t1, 1));

    let effects = handle_publish(&mut state, sm);
    let sends = signed_sends(&effects);
    assert_eq!(
        sorted_peers(sends.iter().map(|(p, _)| p.clone()).collect()),
        vec![peer("a"), peer("b")],
        "publish reaches the relay downstream and the publishing link",
    );
}

// 015 US2-AS2 / FR-005/SC-002: a relayed (Origin::Peer) message never targets a
// publishing link — publishing links carry only the node's own publishes.
#[test]
fn relayed_message_never_targets_publish_links() {
    let t1 = topic("t1");
    let mut state = state_subscribed(vec![t1.clone()]);
    with_active_upstream(&mut state, "x", "t1");
    with_downstream(&mut state, "a", "t1");
    with_outbound_publish_link(&mut state, "b", "t1");
    let sm = signed(signed_ping(&signer(), t1, 1));

    let effects = apply(
        &mut state,
        Event::MessageReceived {
            from: peer("x"),
            message: Message::Dissemination(sm),
        },
    );
    let sends = signed_sends(&effects);
    assert_eq!(
        sorted_peers(sends.iter().map(|(p, _)| p.clone()).collect()),
        vec![peer("a")],
        "the relayed copy goes to the relay downstream only",
    );
}
