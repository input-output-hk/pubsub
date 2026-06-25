mod common;

use std::collections::HashSet;
use std::str::FromStr;
use std::time::Duration;

use common::{
    assert_no_new_deliveries, await_delivery, build_signed_message_simple,
    two_node_fixture_with_subscriptions,
};
use pubsub_node::{
    Message, MessagePayload, MockCryptoScheme, Origin, PlainMessage, PublisherId, Signature,
    SignedMessage, Signer, TestSigner, Timestamp, TopicId,
};

fn topic(s: &str) -> TopicId {
    TopicId::from_str(s).expect("valid topic id")
}

// Settle window for negative (dropped) assertions: the drop is a recv-task-side
// decision; if the message were going to land it would do so within this window.
const SETTLE: Duration = Duration::from_millis(100);

// US1 AS-1: a signature-valid, on-topic message flows into A's arrival log.
#[tokio::test]
async fn valid_signature_message_retained() {
    let t1 = topic("t1");
    let fx = two_node_fixture_with_subscriptions(
        HashSet::from([t1.clone()]),
        HashSet::from([t1.clone()]),
    )
    .await;

    let mut scheme = MockCryptoScheme::with_seed([0u8; 32]);
    let kp = scheme.generate_keypair();
    let signer = TestSigner::new(kp.private);
    let msg = build_signed_message_simple(&signer, t1.clone(), MessagePayload::Ping(42));

    fx.b.send(fx.a.id(), msg.clone()).await.expect("send Ok");

    await_delivery(&fx.a, fx.b.id(), &msg, Duration::from_secs(1))
        .await
        .expect("A retains the valid-signature delivery");

    let record = fx.a.received_messages();
    assert_eq!(record.len(), 1, "exactly the one valid delivery");
    assert_eq!(record[0].origin, Origin::Peer(fx.b.id().clone()));
    assert_eq!(record[0].message, msg);
}

// US1 AS-2: a payload byte altered after signing breaks the signature; the
// message is dropped and never reaches A's snapshot.
#[tokio::test]
async fn payload_tampered_after_signing_dropped() {
    let t1 = topic("t1");
    let fx = two_node_fixture_with_subscriptions(
        HashSet::from([t1.clone()]),
        HashSet::from([t1.clone()]),
    )
    .await;

    let mut scheme = MockCryptoScheme::with_seed([0u8; 32]);
    let kp = scheme.generate_keypair();
    let signer = TestSigner::new(kp.private);

    let Message::Dissemination(mut signed_msg) =
        build_signed_message_simple(&signer, t1.clone(), MessagePayload::Ping(42))
    else {
        unreachable!("build_signed_message_simple yields Message::Dissemination")
    };
    // Mutate the payload without re-signing — the signature no longer matches.
    signed_msg.plain.payload = MessagePayload::Ping(43);
    let tampered = Message::Dissemination(signed_msg);

    fx.b.send(fx.a.id(), tampered).await.expect("send Ok");

    // No-trace non-event: the tampered message is dropped, so A's record never
    // grows. (The drop itself is proven by the state test
    // `authorized_but_tampered_message_dropped_at_verification`.)
    assert_no_new_deliveries(&[&fx.a], SETTLE).await;
}

// US1 AS-3: a message with a valid publisher id but a bogus (all-zero)
// signature is dropped.
#[tokio::test]
async fn bogus_signature_dropped() {
    let t1 = topic("t1");
    let fx = two_node_fixture_with_subscriptions(
        HashSet::from([t1.clone()]),
        HashSet::from([t1.clone()]),
    )
    .await;

    let mut scheme = MockCryptoScheme::with_seed([0u8; 32]);
    let kp = scheme.generate_keypair();

    let plain = PlainMessage {
        topic: t1.clone(),
        publisher_id: PublisherId::from(kp.public),
        parent_hash: None,
        sequence: 0,
        timestamp: Timestamp::from_millis(0),
        payload: MessagePayload::Ping(7),
    };
    let bogus = Message::Dissemination(SignedMessage {
        plain,
        signature: Signature::new(vec![0u8; 32]),
    });

    fx.b.send(fx.a.id(), bogus).await.expect("send Ok");

    // No-trace non-event (drop proven by `invalid_signature_message_dropped`).
    assert_no_new_deliveries(&[&fx.a], SETTLE).await;
}

// US1 AS-4: the declared publisher id does not match the key that signed the
// bytes; verification fails and the message is dropped.
#[tokio::test]
async fn publisher_id_mismatched_with_signing_key_dropped() {
    let t1 = topic("t1");
    let fx = two_node_fixture_with_subscriptions(
        HashSet::from([t1.clone()]),
        HashSet::from([t1.clone()]),
    )
    .await;

    let mut scheme = MockCryptoScheme::with_seed([0u8; 32]);
    let kp_x = scheme.generate_keypair();
    let kp_y = scheme.generate_keypair();
    let signer_x = TestSigner::new(kp_x.private);

    // Declare Y as the publisher, but sign with X's key.
    let plain = PlainMessage {
        topic: t1.clone(),
        publisher_id: PublisherId::from(kp_y.public),
        parent_hash: None,
        sequence: 0,
        timestamp: Timestamp::from_millis(0),
        payload: MessagePayload::Ping(99),
    };
    let signature = signer_x.sign(&plain.signed_bytes());
    let mismatched = Message::Dissemination(SignedMessage { plain, signature });

    fx.b.send(fx.a.id(), mismatched).await.expect("send Ok");

    // No-trace non-event (drop proven by `invalid_signature_message_dropped`).
    assert_no_new_deliveries(&[&fx.a], SETTLE).await;
}
