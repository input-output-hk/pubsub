mod common;

use std::collections::HashSet;
use std::str::FromStr;
use std::time::Duration;

use common::{await_delivery, build_signed_message_simple, two_node_fixture_with_subscriptions};
use pubsub_node::{
    Message, MessagePayload, MockCryptoScheme, PlainMessage, PublisherId, Signature, SignedMessage,
    TestSigner, Timestamp, TopicId,
};

fn topic(s: &str) -> TopicId {
    TopicId::from_str(s).expect("valid topic id")
}

const SETTLE: Duration = Duration::from_millis(100);

// Build a signed-shaped message with a deliberately bogus (all-zero) signature
// on `t`, using a freshly generated, valid-looking publisher id.
fn bogus_signature_message(t: TopicId, n: u64) -> Message {
    let mut scheme = MockCryptoScheme::with_seed([7u8; 32]);
    let kp = scheme.generate_keypair();
    let plain = PlainMessage {
        topic: t,
        publisher_id: PublisherId::from(kp.public),
        parent_hash: None,
        sequence: 0,
        timestamp: Timestamp::from_millis(0),
        payload: MessagePayload::Ping(n),
    };
    Message::Dissemination(SignedMessage {
        plain,
        signature: Signature::new(vec![0u8; 32]),
    })
}

// A is subscribed to {T1}; B's set is irrelevant (it only sends).
async fn fixture_a_t1() -> common::TwoNodeFixture {
    let t1 = topic("t1");
    two_node_fixture_with_subscriptions(HashSet::from([t1.clone()]), HashSet::from([t1])).await
}

// US3 AS-1: valid + on-topic → appears.
#[tokio::test]
async fn valid_on_topic_message_appears_in_snapshot() {
    let fx = fixture_a_t1().await;
    let mut scheme = MockCryptoScheme::with_seed([0u8; 32]);
    let signer = TestSigner::new(scheme.generate_keypair().private);
    let msg = build_signed_message_simple(&signer, topic("t1"), MessagePayload::Ping(1));

    fx.b.send(fx.a.id(), msg.clone()).await.expect("send");
    await_delivery(&fx.a, fx.b.id(), &msg, Duration::from_secs(1))
        .await
        .expect("A retains the valid on-topic delivery");

    let record = fx.a.received_messages();
    assert_eq!(record.len(), 1);
    assert_eq!(record[0].message, msg);
}

// US3 AS-2 (post-004): valid + off-topic (T2) → dropped. A is connected to B on
// T1 only; it has no connection on T2 (a node cannot connect on a topic it is
// not subscribed to), so the connection gate — now the outermost receive check
// — drops the message as `not_connected`, before the subscription filter is
// reached. The observable outcome (absent from the snapshot) is unchanged.
#[tokio::test]
async fn valid_off_topic_message_dropped_with_cause_not_connected() {
    let fx = fixture_a_t1().await;
    let mut scheme = MockCryptoScheme::with_seed([0u8; 32]);
    let signer = TestSigner::new(scheme.generate_keypair().private);
    let msg = build_signed_message_simple(&signer, topic("t2"), MessagePayload::Ping(2));

    fx.b.send(fx.a.id(), msg).await.expect("send");
    tokio::time::sleep(SETTLE).await;

    assert!(
        fx.a.received_messages().is_empty(),
        "off-topic message does not reach the snapshot",
    );
}

// US3 AS-3: invalid + on-topic (T1) → dropped (cause invalid_signature).
#[tokio::test]
async fn invalid_on_topic_message_dropped_with_cause_invalid_signature() {
    let fx = fixture_a_t1().await;
    let msg = bogus_signature_message(topic("t1"), 3);

    fx.b.send(fx.a.id(), msg).await.expect("send");
    tokio::time::sleep(SETTLE).await;

    assert!(
        fx.a.received_messages().is_empty(),
        "invalid-signature on-topic message does not reach the snapshot",
    );
}

// US3 AS-4 (post-004 ordering): off-topic AND invalid → dropped. The connection
// gate is now the outermost check, so it fires before either the subscription
// filter or signature verification — the message drops as `not_connected`
// regardless of its (invalid) signature. (Pre-004 this asserted the topic
// filter rejected ahead of signature; the gate is now the first filter.)
#[tokio::test]
async fn invalid_off_topic_message_dropped_with_cause_not_connected() {
    let fx = fixture_a_t1().await;
    let msg = bogus_signature_message(topic("t2"), 4);

    fx.b.send(fx.a.id(), msg).await.expect("send");
    tokio::time::sleep(SETTLE).await;

    assert!(
        fx.a.received_messages().is_empty(),
        "off-topic + invalid message does not reach the snapshot",
    );
}
