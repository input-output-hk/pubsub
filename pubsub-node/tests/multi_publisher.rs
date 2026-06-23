mod common;

use std::collections::HashSet;
use std::str::FromStr;
use std::time::Duration;

use common::{await_delivery, build_signed_message_simple, two_node_fixture_with_subscriptions};
use pubsub_node::{
    Message, MessagePayload, MockCryptoScheme, Origin, PublisherId, TestSigner, TopicId,
};

fn topic(s: &str) -> TopicId {
    TopicId::from_str(s).expect("valid topic id")
}

const SETTLE: Duration = Duration::from_millis(100);

// Borrow the publisher id of a signed message.
fn publisher_of(message: &Message) -> &PublisherId {
    let Message::Dissemination(signed) = message else {
        unreachable!("only the Signed variant exists in 003")
    };
    &signed.plain.publisher_id
}

// US2 AS-1: three messages, one per distinct publisher, all on T1, all land in
// arrival order — A keys verification off each message's own publisher_id.
#[tokio::test]
async fn three_publishers_all_accepted() {
    let t1 = topic("t1");
    let fx = two_node_fixture_with_subscriptions(
        HashSet::from([t1.clone()]),
        HashSet::from([t1.clone()]),
    )
    .await;

    let mut scheme = MockCryptoScheme::with_seed([0u8; 32]);
    let alice = TestSigner::new(scheme.generate_keypair().private);
    let bob = TestSigner::new(scheme.generate_keypair().private);
    let carol = TestSigner::new(scheme.generate_keypair().private);

    let m_alice = build_signed_message_simple(&alice, t1.clone(), MessagePayload::Ping(1));
    let m_bob = build_signed_message_simple(&bob, t1.clone(), MessagePayload::Ping(2));
    let m_carol = build_signed_message_simple(&carol, t1.clone(), MessagePayload::Ping(3));

    fx.b.send(fx.a.id(), m_alice.clone()).await.expect("send");
    fx.b.send(fx.a.id(), m_bob.clone()).await.expect("send");
    fx.b.send(fx.a.id(), m_carol.clone()).await.expect("send");

    // FIFO from B: awaiting the last delivery means all three are processed.
    await_delivery(&fx.a, fx.b.id(), &m_carol, Duration::from_secs(1))
        .await
        .expect("A observes Carol's delivery");

    let record = fx.a.received_messages();
    let messages: Vec<Message> = record.iter().map(|d| d.message.clone()).collect();
    assert_eq!(
        messages,
        vec![m_alice, m_bob, m_carol],
        "all three deliveries land in arrival order",
    );
    assert!(
        record
            .iter()
            .all(|d| d.origin == Origin::Peer(fx.b.id().clone())),
        "all from B"
    );

    // Each delivery carries its own distinct publisher id.
    let p_alice = publisher_of(&record[0].message);
    let p_bob = publisher_of(&record[1].message);
    let p_carol = publisher_of(&record[2].message);
    assert_ne!(p_alice, p_bob);
    assert_ne!(p_bob, p_carol);
    assert_ne!(p_alice, p_carol);
}

// US2 AS-2: Alice's message has its publisher_id swapped to Bob's key without
// re-signing; only Bob's and Carol's (untouched) deliveries land.
#[tokio::test]
async fn mismatched_publisher_id_rejected() {
    let t1 = topic("t1");
    let fx = two_node_fixture_with_subscriptions(
        HashSet::from([t1.clone()]),
        HashSet::from([t1.clone()]),
    )
    .await;

    let mut scheme = MockCryptoScheme::with_seed([0u8; 32]);
    let alice = TestSigner::new(scheme.generate_keypair().private);
    let kp_bob = scheme.generate_keypair();
    let bob = TestSigner::new(kp_bob.private);
    let carol = TestSigner::new(scheme.generate_keypair().private);

    // Alice signs, then we relabel the message as Bob's — the signature no
    // longer matches the declared publisher's key.
    let Message::Dissemination(mut alice_signed) =
        build_signed_message_simple(&alice, t1.clone(), MessagePayload::Ping(1))
    else {
        unreachable!("Signed variant")
    };
    alice_signed.plain.publisher_id = PublisherId::from(kp_bob.public);
    let m_alice_mislabeled = Message::Dissemination(alice_signed);

    let m_bob = build_signed_message_simple(&bob, t1.clone(), MessagePayload::Ping(2));
    let m_carol = build_signed_message_simple(&carol, t1.clone(), MessagePayload::Ping(3));

    // The valid deliveries are sent first and land; the mislabeled message is
    // sent last. Under 004's misbehavior rule (FR-017) a publisher/signature
    // mismatch over the Active connection severs it — so the mislabeled message
    // is both rejected and severs A↔B; sending it last leaves Bob's and Carol's
    // genuine deliveries already recorded. (The FIFO channel orders the sends.)
    fx.b.send(fx.a.id(), m_bob.clone()).await.expect("send");
    fx.b.send(fx.a.id(), m_carol.clone()).await.expect("send");
    fx.b.send(fx.a.id(), m_alice_mislabeled)
        .await
        .expect("send");

    await_delivery(&fx.a, fx.b.id(), &m_carol, Duration::from_secs(1))
        .await
        .expect("A observes Carol's delivery");
    // Settle so the mislabeled message has been processed (rejected + severs).
    tokio::time::sleep(SETTLE).await;

    let messages: Vec<Message> =
        fx.a.received_messages()
            .into_iter()
            .map(|d| d.message)
            .collect();
    assert_eq!(
        messages,
        vec![m_bob, m_carol],
        "only the unaltered Bob and Carol deliveries land; the mismatched one is rejected",
    );
}

// US2 AS-3: 50 messages from a pool of 5 publishers, delivered in FIFO order;
// the snapshot holds exactly the 50, in arrival order.
#[tokio::test]
async fn interleaved_50_messages_5_publishers() {
    let t1 = topic("t1");
    let fx = two_node_fixture_with_subscriptions(
        HashSet::from([t1.clone()]),
        HashSet::from([t1.clone()]),
    )
    .await;

    let mut scheme = MockCryptoScheme::with_seed([0u8; 32]);
    let signers: Vec<TestSigner> = (0..5)
        .map(|_| TestSigner::new(scheme.generate_keypair().private))
        .collect();

    let mut sent = Vec::new();
    for n in 0..50u64 {
        let signer = &signers[(n % 5) as usize];
        let msg = build_signed_message_simple(signer, t1.clone(), MessagePayload::Ping(n));
        fx.b.send(fx.a.id(), msg.clone()).await.expect("send");
        sent.push(msg);
    }

    await_delivery(
        &fx.a,
        fx.b.id(),
        sent.last().expect("50 messages sent"),
        Duration::from_secs(2),
    )
    .await
    .expect("A observes the final delivery");
    // Negative barrier: no further messages should appear beyond the 50.
    tokio::time::sleep(SETTLE).await;

    let got: Vec<Message> =
        fx.a.received_messages()
            .into_iter()
            .map(|d| d.message)
            .collect();
    assert_eq!(got.len(), 50, "exactly 50 deliveries");
    assert_eq!(got, sent, "FIFO arrival order preserved across publishers");
}
