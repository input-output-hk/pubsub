// Shared test-harness module. Each integration test binary in `tests/` is
// compiled separately and may use only a subset of these helpers, so silence
// per-binary `dead_code` warnings here at the module level.
#![allow(dead_code)]

use std::collections::HashSet;
use std::str::FromStr;
use std::sync::{Arc, Once};
use std::time::Duration;

use pubsub_node::{
    InMemoryNetwork, Message, MessageHash, MessagePayload, Node, NodeConfig, PeerEntry, PeerId,
    PlainMessage, PrivateKey, PublisherId, ReceivedDelivery, SignedMessage, Signer, TestSigner,
    TestVerifier, Timestamp, TopicId, Verifier,
};

/// Install a process-global `tracing` subscriber that routes events through
/// Rust's test capture (`with_test_writer`). With this in place, the
/// integration-test binaries surface `tracing::info!` / `warn!` events under
/// `cargo test -- --nocapture`, matching what the quickstart promises for the
/// off-topic drop log. Defaults to the `info` level so the FR-011 drop event
/// is visible; override with `RUST_LOG=…` when chasing debug events.
fn init_test_tracing() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
            )
            .try_init();
    });
}

/// Sentinel topic carried by fixture-built messages. The default fixture
/// subscribes both nodes to this topic so existing 001-style tests keep
/// observing the deliveries they constructed.
pub fn test_topic() -> TopicId {
    TopicId::from_str("test").expect("valid topic id")
}

/// A fixed-key signer shared across fixture-built messages.
///
/// `TestSigner` is deterministic in its private key, so messages built from the
/// same inputs via [`build_signed_message_simple`] (and [`ping`]) compare equal
/// — the property the 001/002 tests' equality assertions rely on.
pub fn test_signer() -> TestSigner {
    TestSigner::new(PrivateKey::new(b"pubsub-node-test-fixture-signer".to_vec()))
}

/// The verifier shared by test fixtures: accepts any signature produced by a
/// [`TestSigner`] under the matching derived public key.
pub fn shared_test_verifier() -> Arc<dyn Verifier> {
    Arc::new(TestVerifier)
}

/// Build a signed [`Message`] from explicit envelope inputs.
///
/// Constructs the [`PlainMessage`] (deriving `publisher_id` from the signer's
/// public key), signs its canonical bytes, and wraps the result in
/// `Message::Signed`.
pub fn build_signed_message(
    signer: &impl Signer,
    topic: TopicId,
    payload: MessagePayload,
    sequence: u64,
    parent_hash: Option<MessageHash>,
    timestamp: Timestamp,
) -> Message {
    let plain = PlainMessage {
        topic,
        publisher_id: PublisherId::from(signer.public_key()),
        parent_hash,
        sequence,
        timestamp,
        payload,
    };
    let signature = signer.sign(&plain.signed_bytes());
    Message::Signed(SignedMessage { plain, signature })
}

/// Build a signed [`Message`] with default chain fields (`sequence = 0`,
/// `parent_hash = None`, `timestamp = 0`).
pub fn build_signed_message_simple(
    signer: &impl Signer,
    topic: TopicId,
    payload: MessagePayload,
) -> Message {
    build_signed_message(signer, topic, payload, 0, None, Timestamp::from_millis(0))
}

/// Build a signed `Ping(n)` on `topic` using the shared [`test_signer`].
///
/// The 003-era replacement for the 002 `Message::ping(topic, n)` constructor at
/// migrated call sites.
pub fn ping(topic: TopicId, n: u64) -> Message {
    build_signed_message_simple(&test_signer(), topic, MessagePayload::Ping(n))
}

/// Borrow the topic of a [`Message`] regardless of variant.
pub fn message_topic(message: &Message) -> &TopicId {
    match message {
        Message::Signed(signed) => &signed.plain.topic,
        // `Message` is #[non_exhaustive]; 003 defines only the Signed variant.
        _ => unreachable!("Message has only the Signed variant in 003"),
    }
}

pub struct TwoNodeFixture {
    pub network: Arc<InMemoryNetwork>,
    pub a: Node,
    pub b: Node,
}

/// Construct a two-node fixture with both nodes subscribed to
/// [`test_topic`]. Convenience wrapper around
/// [`two_node_fixture_with_subscriptions`] for the 001-style tests that
/// don't care about per-node subscription overrides.
pub async fn two_node_fixture() -> TwoNodeFixture {
    let default_subscriptions = HashSet::from([test_topic()]);
    two_node_fixture_with_subscriptions(default_subscriptions.clone(), default_subscriptions).await
}

/// Construct a two-node fixture with caller-supplied subscription sets for
/// node A and node B independently.
pub async fn two_node_fixture_with_subscriptions(
    a_subscriptions: HashSet<TopicId>,
    b_subscriptions: HashSet<TopicId>,
) -> TwoNodeFixture {
    init_test_tracing();
    let network = Arc::new(InMemoryNetwork::new());
    let a_id = PeerId::from_str("node-a").expect("valid id");
    let b_id = PeerId::from_str("node-b").expect("valid id");

    let a = Node::new(
        a_id.clone(),
        NodeConfig {
            peers: vec![PeerEntry { id: b_id.clone() }],
            subscribed_topics: vec![],
        },
        a_subscriptions,
        network.clone(),
    )
    .await
    .expect("construct node A");

    let b = Node::new(
        b_id,
        NodeConfig {
            peers: vec![PeerEntry { id: a_id }],
            subscribed_topics: vec![],
        },
        b_subscriptions,
        network.clone(),
    )
    .await
    .expect("construct node B");

    TwoNodeFixture { network, a, b }
}

#[derive(Debug, thiserror::Error)]
pub enum AwaitError {
    #[error("timed out after {0:?} waiting for delivery")]
    Timeout(Duration),
}

pub async fn await_delivery(
    node: &Node,
    expected_sender: &PeerId,
    expected_message: &Message,
    timeout: Duration,
) -> Result<(), AwaitError> {
    let poll_interval = Duration::from_millis(1);
    let start = tokio::time::Instant::now();
    loop {
        if matches(&node.received_messages(), expected_sender, expected_message) {
            return Ok(());
        }
        if start.elapsed() >= timeout {
            return Err(AwaitError::Timeout(timeout));
        }
        tokio::time::sleep(poll_interval).await;
    }
}

fn matches(
    record: &[ReceivedDelivery],
    expected_sender: &PeerId,
    expected_message: &Message,
) -> bool {
    record
        .iter()
        .any(|d| &d.from == expected_sender && &d.message == expected_message)
}

/// Assert that `node.subscriptions()`, treated as a set, equals `expected`.
/// Wraps the "snapshot, sort, assert as set" idiom for tests comparing
/// subscription sets.
pub fn assert_subscriptions(node: &Node, expected: &[TopicId]) {
    let mut got = node.subscriptions();
    got.sort_by(|a, b| a.as_str().cmp(b.as_str()));

    let mut want: Vec<TopicId> = expected.to_vec();
    want.sort_by(|a, b| a.as_str().cmp(b.as_str()));

    assert_eq!(
        got, want,
        "subscription set mismatch: got {got:?}, expected {want:?}",
    );
}
