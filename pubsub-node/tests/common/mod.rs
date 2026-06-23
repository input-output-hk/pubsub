// Shared test-harness module. Each integration test binary in `tests/` is
// compiled separately and may use only a subset of these helpers, so silence
// per-binary `dead_code` warnings here at the module level.
#![allow(dead_code)]

use std::collections::{BTreeSet, HashMap, HashSet};
use std::str::FromStr;
use std::sync::{Arc, Once};
use std::time::Duration;

use pubsub_node::{
    AcceptFromAllCandidates, ConnectToAllCandidates, ConnectionStrategy, Event, ForwardToAll,
    InMemoryNetwork, InMemorySubscriptionRegistry, InMemoryTopicRegistry, Message, MessageHash,
    MessagePayload, MockCryptoScheme, Node, NodeConfig, Origin, PeerEntry, PeerId, PlainMessage,
    PrivateKey, PublisherId, ReceivedDelivery, SignedMessage, Signer, SubscriptionRegistryControl,
    TestSigner, TestVerifier, Timestamp, TopicId, TopicRegistryControl, UpstreamState, Verifier,
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

/// The signing identity for a node addressed by `alias`: the mock keypair for
/// that alias. `PeerId::from_str(alias)` and this signer agree by construction
/// (both derive from the alias bytes), so the identity/signer coherence check
/// passes — the node's own coherent signer.
pub fn alias_signer(alias: &str) -> Arc<dyn Signer> {
    let scheme = MockCryptoScheme::with_seed([0u8; 32]);
    Arc::new(scheme.signer(scheme.keypair_from_alias(alias).private))
}

/// Build a signed [`Message`] from explicit envelope inputs.
///
/// Constructs the [`PlainMessage`] (deriving `publisher_id` from the signer's
/// public key), signs its canonical bytes, and wraps the result in
/// `Message::Dissemination`.
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
    Message::Dissemination(SignedMessage { plain, signature })
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

/// Build a `Ping(n)` whose payload is mutated after signing, so its signature
/// no longer verifies — a tampered message for the misbehavior-severance path.
pub fn tampered_ping(topic: TopicId, n: u64) -> Message {
    let Message::Dissemination(mut signed) = ping(topic, n) else {
        unreachable!("ping yields Message::Dissemination")
    };
    signed.plain.payload = MessagePayload::Ping(n.wrapping_add(1));
    Message::Dissemination(signed)
}

/// Borrow the topic of a dissemination [`Message`].
pub fn message_topic(message: &Message) -> &TopicId {
    match message {
        Message::Dissemination(signed) => &signed.plain.topic,
        // Test fixtures only build dissemination messages; `Message` is also
        // `#[non_exhaustive]` (the `Connection` variant + future kinds), hence
        // the catch-all.
        _ => unreachable!("message_topic is only called on dissemination messages"),
    }
}

pub struct TwoNodeFixture {
    pub network: Arc<InMemoryNetwork>,
    pub registry: Arc<InMemorySubscriptionRegistry>,
    pub topic_registry: Arc<InMemoryTopicRegistry>,
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

    // Register every subscribed topic as OPEN in the shared topic registry, so
    // each node's effective set (subscriptions ∩ registered) equals its declared
    // set — the 001/002-style delivery tests behave as before. Tests that want
    // unregistered or publisher-restricted topics drive `topic_registry`
    // directly.
    let topic_registry = Arc::new(InMemoryTopicRegistry::new());
    for t in a_subscriptions.iter().chain(b_subscriptions.iter()) {
        topic_registry
            .set_topic(t.clone(), BTreeSet::new())
            .await
            .expect("register topic open");
    }

    let a = Node::new(
        a_id.clone(),
        NodeConfig {
            peers: vec![PeerEntry { id: b_id.clone() }],
        },
        network.clone(),
        alias_signer(&a_id.to_string()),
        verifier.clone(),
        registry.clone(),
        topic_registry.clone(),
        Arc::new(ConnectToAllCandidates),
        Arc::new(ForwardToAll),
        Arc::new(AcceptFromAllCandidates),
    )
    .await
    .expect("construct node A");

    let b = Node::new(
        b_id.clone(),
        NodeConfig {
            peers: vec![PeerEntry { id: a_id }],
        },
        network.clone(),
        alias_signer(&b_id.to_string()),
        verifier,
        registry.clone(),
        topic_registry.clone(),
        Arc::new(ConnectToAllCandidates),
        Arc::new(ForwardToAll),
        Arc::new(AcceptFromAllCandidates),
    )
    .await
    .expect("construct node B");

    // Both nodes derive their effective subscriptions from two registry streams;
    // wait for convergence so send-then-observe tests are deterministic.
    let shared: Vec<TopicId> = a_subscriptions
        .intersection(&b_subscriptions)
        .cloned()
        .collect();
    let a_expected: Vec<TopicId> = a_subscriptions.into_iter().collect();
    let b_expected: Vec<TopicId> = b_subscriptions.into_iter().collect();
    await_subscriptions(&a, &a_expected, Duration::from_secs(1))
        .await
        .expect("node A subscriptions converge");
    await_subscriptions(&b, &b_expected, Duration::from_secs(1))
        .await
        .expect("node B subscriptions converge");

    // Establishment preamble (004-connections): mutually connect A and B on
    // every topic they share, so payload between them passes the connection
    // gate. Disjoint-subscription fixtures share nothing and stay unconnected —
    // their cross-topic sends are dropped at the gate, as the drop tests expect.
    establish_mutual(&a, &b, &shared).await;

    TwoNodeFixture {
        network,
        registry,
        topic_registry,
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
    node_with_strategy(
        registry,
        network,
        id,
        peers,
        topics,
        Arc::new(ConnectToAllCandidates),
    )
    .await
}

/// Like [`node_with`], but with a caller-supplied connection strategy instead of
/// the default all-candidates policy. Lets a test pin which edges a node dials
/// (e.g. [`ConnectToExplicit`]) so an exact acyclic topology can be built on a
/// shared topic — the all-candidates policy over one topic can only build a full
/// mesh. Acceptance is unaffected (it still uses the real candidate set).
pub async fn node_with_strategy(
    registry: &Arc<InMemorySubscriptionRegistry>,
    network: &Arc<InMemoryNetwork>,
    id: &str,
    peers: &[&str],
    topics: &[TopicId],
    strategy: Arc<dyn ConnectionStrategy>,
) -> Node {
    let id = PeerId::from_str(id).expect("valid id");
    registry
        .set_topics(id.clone(), topics.iter().cloned().collect())
        .await
        .expect("seed node topics");
    // Register the node's topics OPEN in a topic registry so they are legitimate
    // (effective = subscriptions ∩ registered). Candidate-only tests are
    // unaffected; delivery tests need the topic registered for acceptance.
    let topic_registry = Arc::new(InMemoryTopicRegistry::new());
    for t in topics {
        topic_registry
            .set_topic(t.clone(), BTreeSet::new())
            .await
            .expect("register topic open");
    }
    let peers = peers
        .iter()
        .map(|p| PeerEntry {
            id: PeerId::from_str(p).expect("valid peer id"),
        })
        .collect();
    let signer = alias_signer(&id.to_string());
    let node = Node::new(
        id,
        NodeConfig { peers },
        network.clone(),
        signer,
        shared_test_verifier(),
        registry.clone(),
        topic_registry,
        strategy,
        Arc::new(ForwardToAll),
        Arc::new(AcceptFromAllCandidates),
    )
    .await
    .expect("construct node");
    // The node derives its effective subscriptions from the registry streams;
    // wait for convergence before handing it back so send-then-observe tests are
    // deterministic.
    await_subscriptions(&node, topics, Duration::from_secs(1))
        .await
        .expect("node subscriptions converge");
    node
}

/// Construct a node sharing the given subscription **and** topic registries,
/// with config `peers`. Unlike [`node_with`], this seeds **neither** registry —
/// the caller sets up membership (`set_topics`) and topic registration
/// (`set_topic`) explicitly, and awaits convergence itself. Used by the
/// topic-validity and multi-node topic-registry tests, which need a node
/// subscribed to more (or other) topics than are registered.
pub async fn node_sharing(
    registry: &Arc<InMemorySubscriptionRegistry>,
    topic_registry: &Arc<InMemoryTopicRegistry>,
    network: &Arc<InMemoryNetwork>,
    id: &str,
    peers: &[&str],
) -> Node {
    let peers = peers
        .iter()
        .map(|p| PeerEntry {
            id: PeerId::from_str(p).expect("valid peer id"),
        })
        .collect();
    Node::new(
        PeerId::from_str(id).expect("valid id"),
        NodeConfig { peers },
        network.clone(),
        alias_signer(id),
        shared_test_verifier(),
        registry.clone(),
        topic_registry.clone(),
        Arc::new(ConnectToAllCandidates),
        Arc::new(ForwardToAll),
        Arc::new(AcceptFromAllCandidates),
    )
    .await
    .expect("construct node")
}

#[derive(Debug, thiserror::Error)]
pub enum AwaitError {
    #[error("timed out after {0:?} waiting for delivery")]
    Timeout(Duration),
}

/// Poll `node.subscriptions()` (the effective accept-filter) until it equals
/// `expected` (as a set) or `timeout` elapses. A node derives its subscription
/// set asynchronously by folding two `watch` streams (subscription registry +
/// topic registry) — it starts empty and converges only once *both* cold-start
/// bursts have drained — so tests/fixtures wait for it before relying on the
/// accept-filter for send-then-observe.
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

/// Assert none of `nodes` records a NEW delivery within `window` — the
/// bounded-negative counterpart to the `await_*` positives. Snapshots each
/// node's record count, then polls all of them across one window: a new delivery
/// on any node fails immediately (fast, naming the node and the count delta); if
/// `window` elapses with no growth, they are treated as quiescent.
///
/// Like any time-bounded negative it cannot prove "never" (a straggler after
/// `window` is missed), so prefer a positive barrier — await a real downstream
/// event — where the topology provides one; use this only for genuinely
/// unobservable non-events (e.g. duplicate copies that are deduped and dropped).
pub async fn assert_no_new_deliveries(nodes: &[&Node], window: Duration) {
    let baselines: Vec<usize> = nodes.iter().map(|n| n.received_messages().len()).collect();
    let start = tokio::time::Instant::now();
    while start.elapsed() < window {
        for (node, &baseline) in nodes.iter().zip(&baselines) {
            let now = node.received_messages().len();
            assert_eq!(
                now,
                baseline,
                "{} recorded a new delivery within {window:?}: count {baseline} → {now}",
                node.id(),
            );
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

fn matches(
    record: &[ReceivedDelivery],
    expected_sender: &PeerId,
    expected_message: &Message,
) -> bool {
    record.iter().any(|d| {
        d.origin == Origin::Peer(expected_sender.clone()) && &d.message == expected_message
    })
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

// ---- Connection establishment helpers (004-connections) -------------------

/// Inject a setup event through the node's public event intake — the scripted
/// (timer-free) trigger for autonomous establishment. The node consults its
/// strategy and dials on the next drain of its event loop.
pub fn trigger_setup(node: &Node) {
    node.events().push(Event::ConnectionSetup);
}

/// Poll `node.upstream_connections()` until it holds `(peer, topic)` as
/// `Active`, or `timeout` elapses. Establishment is asynchronous (the request,
/// its acceptance, and the activation each cross the event loop), so tests wait
/// the same way they wait for delivery.
pub async fn await_upstream_active(
    node: &Node,
    peer: &PeerId,
    topic: &TopicId,
    timeout: Duration,
) -> Result<(), AwaitError> {
    let start = tokio::time::Instant::now();
    loop {
        let active = node
            .upstream_connections()
            .into_iter()
            .any(|(p, t, state)| &p == peer && &t == topic && state == UpstreamState::Active);
        if active {
            return Ok(());
        }
        if start.elapsed() >= timeout {
            return Err(AwaitError::Timeout(timeout));
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

/// Poll `node.downstream_connections()` until it holds `(peer, topic)`, or
/// `timeout` elapses.
pub async fn await_downstream(
    node: &Node,
    peer: &PeerId,
    topic: &TopicId,
    timeout: Duration,
) -> Result<(), AwaitError> {
    let start = tokio::time::Instant::now();
    loop {
        if node
            .downstream_connections()
            .iter()
            .any(|(p, t)| p == peer && t == topic)
        {
            return Ok(());
        }
        if start.elapsed() >= timeout {
            return Err(AwaitError::Timeout(timeout));
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

/// The establishment-helper timeout — generous, since establishment crosses the
/// event loop several times (request, accept, activate).
const ESTABLISH_TIMEOUT: Duration = Duration::from_secs(2);

/// Poll until `node` holds an upstream entry for `(peer, topic)` in any state
/// (e.g. an `AwaitingAccept` entry toward a peer that never answers), or
/// `timeout` elapses.
pub async fn await_upstream_present(
    node: &Node,
    peer: &PeerId,
    topic: &TopicId,
    timeout: Duration,
) -> Result<(), AwaitError> {
    let start = tokio::time::Instant::now();
    loop {
        if node
            .upstream_connections()
            .iter()
            .any(|(p, t, _)| p == peer && t == topic)
        {
            return Ok(());
        }
        if start.elapsed() >= timeout {
            return Err(AwaitError::Timeout(timeout));
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

/// Poll until `peer` appears in `node`'s candidate set for `topic` (a superset
/// check, unlike [`await_candidates`]' set-equality) or `timeout` elapses —
/// the precondition for a setup event to dial `peer`.
pub async fn await_candidate_present(
    node: &Node,
    topic: &TopicId,
    peer: &PeerId,
    timeout: Duration,
) -> Result<(), AwaitError> {
    let start = tokio::time::Instant::now();
    loop {
        if node.candidates(topic).iter().any(|p| p == peer) {
            return Ok(());
        }
        if start.elapsed() >= timeout {
            return Err(AwaitError::Timeout(timeout));
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

/// Mutually establish Active connections between `a` and `b` on every topic in
/// `topics`: await each end knows the other as a candidate, trigger both setup
/// events, then await the Active upstream both ways. A no-op for an empty list.
pub async fn establish_mutual(a: &Node, b: &Node, topics: &[TopicId]) {
    if topics.is_empty() {
        return;
    }
    for t in topics {
        await_candidate_present(a, t, b.id(), ESTABLISH_TIMEOUT)
            .await
            .expect("a knows b as a candidate");
        await_candidate_present(b, t, a.id(), ESTABLISH_TIMEOUT)
            .await
            .expect("b knows a as a candidate");
    }
    trigger_setup(a);
    trigger_setup(b);
    for t in topics {
        await_upstream_active(a, b.id(), t, ESTABLISH_TIMEOUT)
            .await
            .expect("a's upstream to b is Active");
        await_upstream_active(b, a.id(), t, ESTABLISH_TIMEOUT)
            .await
            .expect("b's upstream to a is Active");
    }
}

/// Poll until `node` holds no upstream or downstream connection naming `peer`
/// (in any topic), or `timeout` elapses — e.g. after a counterpart's graceful
/// shutdown, whose `Terminated` notices the node processes asynchronously.
pub async fn await_peer_forgotten(
    node: &Node,
    peer: &PeerId,
    timeout: Duration,
) -> Result<(), AwaitError> {
    let start = tokio::time::Instant::now();
    loop {
        let in_upstream = node
            .upstream_connections()
            .iter()
            .any(|(p, _, _)| p == peer);
        let in_downstream = node.downstream_connections().iter().any(|(p, _)| p == peer);
        if !in_upstream && !in_downstream {
            return Ok(());
        }
        if start.elapsed() >= timeout {
            return Err(AwaitError::Timeout(timeout));
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

/// Establish `receiver`'s Active upstream to each of `senders` on `topic`:
/// await the candidates are known, trigger the receiver's single setup event,
/// then await each upstream Active. The one-directional preamble for the
/// multi-node suites (the receiver dials its senders).
pub async fn establish_upstreams(receiver: &Node, senders: &[&Node], topic: &TopicId) {
    for sender in senders {
        await_candidate_present(receiver, topic, sender.id(), ESTABLISH_TIMEOUT)
            .await
            .expect("receiver knows the sender as a candidate");
    }
    trigger_setup(receiver);
    for sender in senders {
        await_upstream_active(receiver, sender.id(), topic, ESTABLISH_TIMEOUT)
            .await
            .expect("receiver's upstream to the sender is Active");
    }
}

// ---- Scripted acyclic topology (006 fan-out) ------------------------------

/// A connection strategy that dials a fixed, **explicit** set of `(peer, topic)`
/// edges, ignoring the discovered candidate set.
///
/// The default all-candidates policy ([`ConnectToAllCandidates`]) over a single
/// shared topic dials every co-member, so it can only build a full mesh — and
/// receive-path fan-out then circulates a payload around it (unbounded until
/// dedup; a mesh also masks relay correctness, since every node gets a direct
/// copy). Pinning each node's dialed edges lets a test build an exact **acyclic**
/// topology (a star or a line) on one topic. Acceptance is unaffected — it still
/// uses the real candidate set, so a dialed peer must still be a registry member
/// for the edge to be accepted. Test-harness only (lives here, not in `src`):
/// it is not a production strategy and never reaches the node's public surface.
pub struct ConnectToExplicit(pub Vec<(PeerId, TopicId)>);

impl ConnectionStrategy for ConnectToExplicit {
    fn expected_upstream(
        &self,
        _subscriptions: &HashSet<TopicId>,
        _candidates: &HashMap<TopicId, HashSet<PeerId>>,
    ) -> HashSet<(PeerId, TopicId)> {
        self.0.iter().cloned().collect()
    }
}

/// A fluent builder for a test node on a shared subscription registry + network
/// — the declarative front end to [`node_with_strategy`] for scripting exact
/// topologies. Defaults: no config peers, the all-candidates dial policy. The
/// dial policy is overridden by [`dials`](NodeSpec::dials) (one explicit edge) or
/// [`dials_nobody`](NodeSpec::dials_nobody) (accept-only), which is how an acyclic
/// star/line is built on a single shared topic.
///
/// ```ignore
/// let hub   = node(&registry, &network, "p").topic(&t).dials_nobody().build().await;
/// let spoke = node(&registry, &network, "d1").topic(&t).dials(&[(&hub, &t)]).build().await;
/// ```
pub struct NodeSpec<'a> {
    registry: &'a Arc<InMemorySubscriptionRegistry>,
    network: &'a Arc<InMemoryNetwork>,
    id: String,
    peers: Vec<String>,
    topics: Vec<TopicId>,
    strategy: Arc<dyn ConnectionStrategy>,
}

/// Start building a node `id` sharing `registry` and `network`.
pub fn node<'a>(
    registry: &'a Arc<InMemorySubscriptionRegistry>,
    network: &'a Arc<InMemoryNetwork>,
    id: &str,
) -> NodeSpec<'a> {
    NodeSpec {
        registry,
        network,
        id: id.to_string(),
        peers: Vec::new(),
        topics: Vec::new(),
        strategy: Arc::new(ConnectToAllCandidates),
    }
}

impl NodeSpec<'_> {
    /// Subscribe the node to one `topic` (also registered open). Repeatable.
    #[must_use]
    pub fn topic(mut self, topic: &TopicId) -> Self {
        self.topics.push(topic.clone());
        self
    }

    /// Subscribe the node to `topics` (also registered open), replacing any set.
    #[must_use]
    pub fn topics(mut self, topics: &[TopicId]) -> Self {
        self.topics = topics.to_vec();
        self
    }

    /// Set the config peer list (rarely needed — candidates come from the
    /// registry, not this list).
    #[must_use]
    pub fn peers(mut self, peers: &[&str]) -> Self {
        self.peers = peers.iter().map(|p| (*p).to_string()).collect();
        self
    }

    /// Dial exactly the given `(peer, topic)` edges and nothing else: on a setup
    /// the node sends each listed peer a connection request — creating a pending
    /// (`AwaitingAccept`) upstream and asking to receive from it. Acceptance is
    /// not implied by the dial (the entry stays pending until the peer accepts);
    /// once accepted, that peer is an upstream of this node and this node a
    /// downstream of it, so the peer fans out here. Multiple edges request several
    /// upstream sources (e.g. a diamond). Sets the dial policy to
    /// [`ConnectToExplicit`].
    #[must_use]
    pub fn dials(mut self, edges: &[(&Node, &TopicId)]) -> Self {
        self.strategy = Arc::new(ConnectToExplicit(
            edges
                .iter()
                .map(|(peer, topic)| (peer.id().clone(), (*topic).clone()))
                .collect(),
        ));
        self
    }

    /// Dial nobody — the node only accepts inbound connections (`dials(&[])`).
    #[must_use]
    pub fn dials_nobody(mut self) -> Self {
        self.strategy = Arc::new(ConnectToExplicit(vec![]));
        self
    }

    /// Construct the node and await its subscription convergence.
    pub async fn build(self) -> Node {
        let peers: Vec<&str> = self.peers.iter().map(String::as_str).collect();
        node_with_strategy(
            self.registry,
            self.network,
            &self.id,
            &peers,
            &self.topics,
            self.strategy,
        )
        .await
    }
}
