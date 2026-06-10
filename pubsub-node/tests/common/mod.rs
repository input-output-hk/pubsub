// Shared test-harness module. Each integration test binary in `tests/` is
// compiled separately and may use only a subset of these helpers, so silence
// per-binary `dead_code` warnings here at the module level.
#![allow(dead_code)]

use std::collections::HashSet;
use std::str::FromStr;
use std::sync::{Arc, Once};
use std::time::Duration;

use pubsub_node::{
    InMemoryNetwork, InMemorySubscriptionRegistry, Message, MessageHash, MessagePayload, Node,
    NodeConfig, PeerEntry, PeerId, PlainMessage, PrivateKey, PublisherId, ReceivedDelivery,
    SignedMessage, Signer, SubscriptionRegistryControl, TestSigner, TestVerifier, Timestamp,
    TopicId, Verifier,
};

/// Install a process-global `tracing` subscriber that routes events through
/// Rust's test capture (`with_test_writer`). With this in place, the
/// integration-test binaries surface `tracing::info!` / `warn!` events under
/// `cargo test -- --nocapture`, matching what the quickstart promises for the
/// off-topic drop log. Defaults to the `info` level so the `message_dropped`
/// drop events are visible; override with `RUST_LOG=…` when chasing debug events.
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
    pub registry: Arc<InMemorySubscriptionRegistry>,
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
    let verifier = shared_test_verifier();
    let a_id = PeerId::from_str("node-a").expect("valid id");
    let b_id = PeerId::from_str("node-b").expect("valid id");

    // Seed the subscription registry (the source of truth for each node's
    // topics) before constructing the nodes — both look up their own entry.
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    registry
        .set_topics(a_id.clone(), a_subscriptions.iter().cloned().collect())
        .await
        .expect("seed node A topics");
    registry
        .set_topics(b_id.clone(), b_subscriptions.iter().cloned().collect())
        .await
        .expect("seed node B topics");

    let a = Node::new(
        a_id.clone(),
        NodeConfig {
            peers: vec![PeerEntry { id: b_id.clone() }],
        },
        network.clone(),
        verifier.clone(),
        registry.clone(),
    )
    .await
    .expect("construct node A");

    let b = Node::new(
        b_id,
        NodeConfig {
            peers: vec![PeerEntry { id: a_id }],
        },
        network.clone(),
        verifier,
        registry.clone(),
    )
    .await
    .expect("construct node B");

    // Both nodes derive their subscriptions from the registry stream; wait for
    // convergence so the 001/002-style send-then-observe tests are deterministic.
    let a_expected: Vec<TopicId> = a_subscriptions.into_iter().collect();
    let b_expected: Vec<TopicId> = b_subscriptions.into_iter().collect();
    await_subscriptions(&a, &a_expected, Duration::from_secs(1))
        .await
        .expect("node A subscriptions converge");
    await_subscriptions(&b, &b_expected, Duration::from_secs(1))
        .await
        .expect("node B subscriptions converge");

    TwoNodeFixture {
        network,
        registry,
        a,
        b,
    }
}

/// Build a node sharing `registry` and `network`, with its subscription-list
/// entry seeded with `topics` and a config peer list of `peers`. Centralises
/// the registry-seed-then-construct dance for the inline multi-node tests.
pub async fn node_with(
    registry: &Arc<InMemorySubscriptionRegistry>,
    network: &Arc<InMemoryNetwork>,
    id: &str,
    peers: &[&str],
    topics: &[TopicId],
) -> Node {
    let id = PeerId::from_str(id).expect("valid id");
    registry
        .set_topics(id.clone(), topics.iter().cloned().collect())
        .await
        .expect("seed node topics");
    let peers = peers
        .iter()
        .map(|p| PeerEntry {
            id: PeerId::from_str(p).expect("valid peer id"),
        })
        .collect();
    let node = Node::new(
        id,
        NodeConfig { peers },
        network.clone(),
        shared_test_verifier(),
        registry.clone(),
    )
    .await
    .expect("construct node");
    // The node derives its subscriptions from the registry stream; wait for
    // that before handing it back so send-then-observe tests are deterministic.
    await_subscriptions(&node, topics, Duration::from_secs(1))
        .await
        .expect("node subscriptions converge");
    node
}

#[derive(Debug, thiserror::Error)]
pub enum AwaitError {
    #[error("timed out after {0:?} waiting for delivery")]
    Timeout(Duration),
}

/// Poll `node.subscriptions()` until it equals `expected` (as a set) or
/// `timeout` elapses. A node derives its subscription set asynchronously from
/// the registry `watch` stream (it starts empty), so tests/fixtures wait for it
/// to converge before relying on the node's accept-filter.
pub async fn await_subscriptions(
    node: &Node,
    expected: &[TopicId],
    timeout: Duration,
) -> Result<(), AwaitError> {
    let want: std::collections::BTreeSet<TopicId> = expected.iter().cloned().collect();
    let start = tokio::time::Instant::now();
    loop {
        let got: std::collections::BTreeSet<TopicId> = node.subscriptions().into_iter().collect();
        if got == want {
            return Ok(());
        }
        if start.elapsed() >= timeout {
            return Err(AwaitError::Timeout(timeout));
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

/// Poll `node.candidates(topic)` until it equals `expected` (as a set of id
/// strings) or `timeout` elapses. Candidate-set convergence is asynchronous
/// (the registry reader drains the membership stream onto the event loop), so
/// tests wait the same way they wait for message delivery.
pub async fn await_candidates(
    node: &Node,
    topic: &TopicId,
    expected: &[&str],
    timeout: Duration,
) -> Result<(), AwaitError> {
    let want: std::collections::BTreeSet<String> =
        expected.iter().map(|s| (*s).to_string()).collect();
    let start = tokio::time::Instant::now();
    loop {
        let got: std::collections::BTreeSet<String> = node
            .candidates(topic)
            .iter()
            .map(ToString::to_string)
            .collect();
        if got == want {
            return Ok(());
        }
        if start.elapsed() >= timeout {
            return Err(AwaitError::Timeout(timeout));
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
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
