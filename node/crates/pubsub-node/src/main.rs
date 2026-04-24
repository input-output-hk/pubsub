mod api;

use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use clap::Parser;
use pallas_crypto::key::ed25519::SecretKey;
use tokio::sync::RwLock;
use tracing::{info, warn};

use pubsub_types::error::PubSubError;
use pubsub_types::message::{Message, TopicId};
use pubsub_types::node::{node_id_from_key, NodeInfo};
use pubsub_types::topic::TopicConfig;
use pubsub_types::traits::*;

#[derive(clap::ValueEnum, Clone, Debug)]
enum Network {
    Mainnet,
    Preprod,
    Preview,
}

impl Network {
    /// Bech32 HRP for node identifiers on this network.
    fn bech32_hrp(&self) -> &'static str {
        match self {
            Network::Mainnet => "psnode",
            Network::Preprod | Network::Preview => "psnode_test",
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Network::Mainnet => "mainnet",
            Network::Preprod => "preprod",
            Network::Preview => "preview",
        }
    }
}

use pubsub_network::codec::CborCodec;
use pubsub_network::cyclon::{CyclonConfig, Cyclon};
use pubsub_network::dissemination::{DisseminationConfig, HybridDisseminator};
use pubsub_network::mock_chain::MockChainState;
use pubsub_network::pallas_chain::{CardanoChainState, ContractAddresses};
use pubsub_network::relay_policy::DefaultRelayPolicy;
use pubsub_network::store::HotCache;
use pubsub_network::transport::QuicTransport;
use pubsub_network::validator::SignatureValidator;
use pubsub_network::vicinity::{Vicinity, VicinityConfig};

#[derive(Parser, Debug)]
#[command(name = "pubsub-node", about = "Cardano PubSub relay node")]
struct Args {
    /// Address to bind the PubSub node to
    #[arg(short, long, default_value = "0.0.0.0:9000")]
    bind: SocketAddr,

    /// Addresses of bootstrap peers (comma-separated)
    #[arg(short, long, value_delimiter = ',')]
    peers: Vec<SocketAddr>,

    /// Node name (for logging in testnet)
    #[arg(short, long, default_value = "node-0")]
    name: String,

    /// Topics to subscribe to (comma-separated topic names)
    #[arg(short, long, value_delimiter = ',')]
    topics: Vec<String>,

    /// Cyclon gossip interval in seconds
    #[arg(long, default_value = "5")]
    cyclon_interval: u64,

    /// Vicinity gossip interval in seconds
    #[arg(long, default_value = "10")]
    vicinity_interval: u64,

    /// Path to a 32-byte Ed25519 key file for persistent node identity.
    /// Created automatically on first run if the file does not exist.
    /// Omit to use an ephemeral key (identity lost on restart).
    #[arg(long)]
    key_file: Option<PathBuf>,

    /// Cardano network this node observes.
    /// Affects the bech32 HRP used in node identifiers (psnode / psnode_test).
    #[arg(long, value_enum, default_value = "preprod")]
    network: Network,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// HTTP API port (defaults to QUIC port + 1000; set 0 to disable)
    #[arg(long)]
    http_port: Option<u16>,

    /// Public address to advertise to peers (defaults to bind address)
    #[arg(long)]
    advertise_addr: Option<SocketAddr>,

    // ── Chain state backend (all optional; omit to use mock / local testnet) ──

    /// How often (seconds) to poll the chain registry for new topics.
    /// Set to 0 to disable. Default: 300 (5 min). Only active when a chain backend is configured.
    #[arg(long, default_value = "300")]
    topic_refresh_interval: u64,

    /// Ogmios JSON-RPC URL for reading on-chain topic registry.
    /// Requires Ogmios v6.0+ (HTTP POST interface). No API key needed.
    /// Example: http://localhost:1337
    /// Also requires --topic-registry-addr, --publisher-vault-addr, --registry-policy-id.
    #[arg(long)]
    ogmios_url: Option<String>,

    /// Blockfrost project ID for reading on-chain topic registry.
    /// Also requires --blockfrost-url (optional) and contract address flags.
    #[arg(long)]
    blockfrost_key: Option<String>,

    /// Blockfrost base URL (default: preprod).
    #[arg(long, default_value = "https://cardano-preprod.blockfrost.io/api/v0")]
    blockfrost_url: String,

    // ── Contract addresses (required when using any chain backend) ────────────

    /// Bech32 address of the deployed topic registry validator.
    #[arg(long)]
    topic_registry_addr: Option<String>,

    /// Bech32 address of the deployed node registry validator.
    #[arg(long)]
    node_registry_addr: Option<String>,

    /// Bech32 address of the deployed publisher vault validator.
    #[arg(long)]
    publisher_vault_addr: Option<String>,

    /// Hex policy ID of the registry minting policy (56 hex chars).
    #[arg(long)]
    registry_policy_id: Option<String>,
}

/// Minimal subscription manager: logs delivered messages to stdout.
struct LocalSubscriptionManager {
    subscriptions: RwLock<HashSet<TopicId>>,
}

impl LocalSubscriptionManager {
    fn new() -> Self {
        Self {
            subscriptions: RwLock::new(HashSet::new()),
        }
    }
}

#[async_trait]
impl SubscriptionManager for LocalSubscriptionManager {
    async fn subscribe(&self, topic: &TopicId) -> Result<(), PubSubError> {
        self.subscriptions.write().await.insert(topic.clone());
        Ok(())
    }

    async fn unsubscribe(&self, topic: &TopicId) -> Result<(), PubSubError> {
        self.subscriptions.write().await.remove(topic);
        Ok(())
    }

    async fn subscriptions(&self) -> HashSet<TopicId> {
        self.subscriptions.read().await.clone()
    }

    async fn deliver(&self, msg: Message) -> Result<(), PubSubError> {
        let payload = std::str::from_utf8(&msg.payload).unwrap_or("<binary>");
        info!(
            topic = %msg.topic_id,
            seq = msg.sequence_nr,
            payload,
            "Delivered message to local subscriber"
        );
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| args.log_level.clone().into()),
        )
        .with_target(false)
        .init();

    info!(name = %args.name, bind = %args.bind, "Starting PubSub node");

    let key_seed = load_or_generate_key(args.key_file.as_deref())?;
    let signing_key = SecretKey::from(key_seed);
    let public_key = signing_key.public_key();
    // NodeId is derived from the Ed25519 public key (D2 paper, Ch.3).
    // Relay nodes are identified by their key, not their address.
    let node_id = node_id_from_key(public_key.as_ref());
    info!(node_id = %node_id, "Node identity derived from public key");

    let chain_state: Arc<dyn ChainState> = build_chain_state(&args);

    // Load the active topic set from the chain registry.
    // Falls back to --topics (CLI) if the registry is empty or unreachable.
    let active_topics: Vec<TopicConfig> = match chain_state.get_all_topics().await {
        Ok(topics) if !topics.is_empty() => {
            info!(count = topics.len(), "Loaded topics from chain registry");
            topics
        }
        Ok(_) if !args.topics.is_empty() => {
            info!("Chain registry empty; using --topics CLI fallback");
            default_topics(&args.topics)
        }
        Ok(_) => {
            info!("No topics from chain and no --topics; subscribing to nothing");
            vec![]
        }
        Err(e) => {
            warn!(error = %e, "Chain state unavailable; using --topics CLI fallback");
            default_topics(&args.topics)
        }
    };

    // Keep the concrete type so we can coerce to both Transport and GossipTransport.
    let transport = Arc::new(QuicTransport::new(args.bind, &key_seed).await?);
    let transport_app: Arc<dyn Transport> = transport.clone();
    let transport_gossip: Arc<dyn pubsub_types::traits::GossipTransport> = transport.clone();

    let codec = Arc::new(CborCodec);
    let store: Arc<dyn MessageStore> = Arc::new(HotCache::with_defaults());
    let validator: Arc<dyn MessageValidator> =
        Arc::new(SignatureValidator::new(chain_state.clone()));
    let relay_policy: Arc<dyn RelayPolicy> = Arc::new(DefaultRelayPolicy);

    let advertise_addr = args.advertise_addr.unwrap_or(args.bind);
    if advertise_addr.ip().is_unspecified() {
        warn!("Bind address is 0.0.0.0 and --advertise-addr is not set; \
               peers will not be able to dial back to this node. \
               Pass --advertise-addr <public-ip>:9000 to be reachable.");
    }
    let self_info = NodeInfo {
        node_id: node_id.clone(),
        addr: advertise_addr,
        public_key: public_key.as_ref().to_vec(),
        subscribed_topics: active_topics.iter().map(|t| t.topic_id.clone()).collect(),
    };

    let cyclon_concrete = Arc::new(Cyclon::new(
        self_info.clone(),
        transport_app.clone(),
        transport_gossip.clone(),
        CyclonConfig::default(),
    ));

    // Serve inbound gossip requests in a dedicated task — completely separate
    // from the application-message receive loop.
    {
        let cyclon_serve = cyclon_concrete.clone();
        tokio::spawn(async move { cyclon_serve.serve_gossip().await });
    }

    let cyclon: Arc<dyn PeerSampler> = cyclon_concrete;

    let vicinity_concrete = Arc::new(Vicinity::new(
        self_info.clone(),
        cyclon.clone(),
        transport_gossip.clone(),
        VicinityConfig::default(),
    ));

    {
        let vicinity_serve = vicinity_concrete.clone();
        tokio::spawn(async move { vicinity_serve.serve_gossip().await });
    }

    let vicinity: Arc<dyn TopicRouter> = vicinity_concrete;

    let subscription_mgr: Arc<dyn SubscriptionManager> =
        Arc::new(LocalSubscriptionManager::new());

    let disseminator: Arc<dyn Disseminator> = Arc::new(HybridDisseminator::new(
        node_id,
        transport_app.clone(),
        codec.clone(),
        cyclon.clone(),
        vicinity.clone(),
        subscription_mgr.clone(),
        DisseminationConfig::default(),
    ));

    // Bootstrap peers come from --peers only. Relay nodes join the overlay
    // permissionlessly via Cyclon gossip (D2 Ch.3) — no on-chain registration.
    // connect_bootstrap() derives the real key-based NodeId from the peer's TLS cert.
    let bootstrap_peers: Vec<NodeInfo> = if !args.peers.is_empty() {
        let local_topic_ids: Vec<TopicId> =
            active_topics.iter().map(|t| t.topic_id.clone()).collect();
        let mut peers = Vec::new();
        for addr in &args.peers {
            match transport.connect_bootstrap(*addr).await {
                Ok(node_id) => {
                    peers.push(NodeInfo {
                        node_id,
                        addr: *addr,
                        public_key: vec![],
                        subscribed_topics: local_topic_ids.clone(),
                    });
                }
                Err(e) => warn!(addr = %addr, error = %e, "Failed to connect to bootstrap peer"),
            }
        }
        peers
    } else {
        vec![]
    };

    // ---- HTTP API ----
    let http_port = match args.http_port {
        Some(0) => None,
        Some(p) => Some(p),
        None => Some(args.bind.port().saturating_add(1000)),
    };

    let api_state = if let Some(port) = http_port {
        let (state, _tx) = api::ApiState::new(
            self_info.clone(),
            args.network.as_str().to_string(),
            args.network.bech32_hrp().to_string(),
        );
        // Seed topic names so they display immediately, not only after a message arrives.
        for tc in &active_topics {
            let hex: String = tc.topic_id.0.iter().map(|b| format!("{b:02x}")).collect();
            state.topic_names.insert(hex, tc.name.clone());
        }
        let http_addr: SocketAddr = SocketAddr::new(args.bind.ip(), port);
        let state_clone = state.clone();
        tokio::spawn(async move {
            api::start(state_clone, http_addr).await;
        });
        info!(http_port = port, "HTTP dashboard enabled");
        Some(state)
    } else {
        None
    };

    if !bootstrap_peers.is_empty() {
        info!(peer_count = bootstrap_peers.len(), "Bootstrapping with peers");
        if let Some(ref s) = api_state {
            for peer in &bootstrap_peers {
                s.record_peer_connected(&peer.node_id, &peer.addr.to_string());
            }
        }
        cyclon.bootstrap(bootstrap_peers).await?;

        // Run a few eager Cyclon cycles so the view fills quickly from the
        // seed before the periodic gossip loop takes over.  Each cycle asks
        // the seed (or the oldest known peer) for their shuffle buffer, which
        // propagates knowledge of other nodes transitively.
        for _ in 0..5 {
            tokio::time::sleep(Duration::from_millis(300)).await;
            if let Err(e) = cyclon.cycle().await {
                warn!(error = %e, "Initial bootstrap cycle failed");
            }
            let view_size = cyclon.view().await.len();
            if view_size >= 3 {
                break;
            }
        }
        let view_size = cyclon.view().await.len();
        info!(view_size, "Peer view after initial bootstrap cycles");
    }

    for tc in &active_topics {
        vicinity.join_topic(&tc.topic_id).await?;
        subscription_mgr.subscribe(&tc.topic_id).await?;
        info!(topic = %tc.name, topic_id = %tc.topic_id, "Subscribed to topic");
    }

    // Periodically refresh the topic list from chain so new topics are picked
    // up without a node restart.
    if args.topic_refresh_interval > 0
        && (args.ogmios_url.is_some() || args.blockfrost_key.is_some())
    {
        let refresh_chain = chain_state.clone();
        let refresh_vicinity = vicinity.clone();
        let refresh_subs = subscription_mgr.clone();
        let refresh_api = api_state.clone();
        let refresh_interval = Duration::from_secs(args.topic_refresh_interval);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(refresh_interval);
            interval.tick().await; // skip the immediate first tick
            loop {
                interval.tick().await;
                match refresh_chain.get_all_topics().await {
                    Ok(topics) => {
                        let current = refresh_subs.subscriptions().await;
                        for tc in &topics {
                            if !current.contains(&tc.topic_id) {
                                if let Err(e) = refresh_vicinity.join_topic(&tc.topic_id).await {
                                    warn!(topic = %tc.name, error = %e, "join_topic failed");
                                    continue;
                                }
                                if let Err(e) = refresh_subs.subscribe(&tc.topic_id).await {
                                    warn!(topic = %tc.name, error = %e, "subscribe failed");
                                    continue;
                                }
                                info!(topic = %tc.name, topic_id = %tc.topic_id, "New topic discovered on chain");
                                if let Some(ref s) = refresh_api {
                                    let hex: String =
                                        tc.topic_id.0.iter().map(|b| format!("{b:02x}")).collect();
                                    s.topic_names.insert(hex, tc.name.clone());
                                }
                            }
                        }
                    }
                    Err(e) => warn!(error = %e, "Topic refresh from chain failed"),
                }
            }
        });
    }

    let cyclon_clone = cyclon.clone();
    let cyclon_interval = Duration::from_secs(args.cyclon_interval);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(cyclon_interval);
        loop {
            interval.tick().await;
            if let Err(e) = cyclon_clone.cycle().await {
                warn!(error = %e, "Cyclon cycle failed");
            }
        }
    });

    let vicinity_clone = vicinity.clone();
    let vicinity_interval = Duration::from_secs(args.vicinity_interval);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(vicinity_interval);
        loop {
            interval.tick().await;
            if let Err(e) = vicinity_clone.cycle().await {
                warn!(error = %e, "Vicinity cycle failed");
            }
        }
    });

    let store_clone = store.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            match store_clone.evict_expired().await {
                Ok(count) if count > 0 => info!(evicted = count, "Cache eviction"),
                Err(e) => warn!(error = %e, "Cache eviction failed"),
                _ => {}
            }
        }
    });

    // Build topic-name lookup for event recording
    let topic_name_map: std::collections::HashMap<TopicId, String> = active_topics
        .iter()
        .map(|tc| (tc.topic_id.clone(), tc.name.clone()))
        .collect();

    info!("Node running. Waiting for messages...");
    loop {
        match transport_app.recv().await {
            Ok((from, data)) => {
                let msg = match codec.decode(&data) {
                    Ok(m) => m,
                    Err(e) => {
                        warn!(error = %e, "Failed to decode message");
                        continue;
                    }
                };

                if let Err(e) = validator.validate(&msg).await {
                    warn!(error = %e, "Message validation failed");
                    continue;
                }

                match relay_policy.should_relay(&msg, &from).await {
                    RelayDecision::Forward => {}
                    RelayDecision::Drop(reason) => {
                        warn!(reason = %reason, "Message dropped by relay policy");
                        continue;
                    }
                    RelayDecision::Delay(d) => {
                        tokio::time::sleep(d).await;
                    }
                }

                // Check dedup before recording to the API feed.  If the
                // store already holds this (topic, seq) pair, a duplicate
                // arrived via a second path and we should not show it twice.
                let already_seen = store.get(&msg.id()).await.ok().flatten().is_some();

                if let Err(e) = store.store(msg.clone()).await {
                    warn!(error = %e, "Failed to store message");
                }

                if !already_seen {
                    if let Some(ref s) = api_state {
                        let topic_name = topic_name_map.get(&msg.topic_id).map(String::as_str);
                        s.record_message(&from, &msg, topic_name).await;
                    }
                }

                if let Err(e) = disseminator.on_receive(&from, msg).await {
                    warn!(error = %e, "Dissemination failed");
                }
            }
            Err(e) => {
                warn!(error = %e, "Transport recv error");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

fn contract_addresses(args: &Args) -> Option<ContractAddresses> {
    Some(ContractAddresses {
        topic_registry_addr: args.topic_registry_addr.clone()?,
        node_registry_addr: args.node_registry_addr.clone().unwrap_or_default(),
        publisher_vault_addr: args.publisher_vault_addr.clone()?,
        registry_policy_id: args.registry_policy_id.clone()?,
    })
}

fn build_chain_state(args: &Args) -> Arc<dyn ChainState> {
    if let Some(url) = &args.ogmios_url {
        match contract_addresses(args) {
            Some(c) => {
                info!(url = %url, "Using Ogmios chain state backend");
                return Arc::new(CardanoChainState::ogmios(url, c));
            }
            None => warn!(
                "–-ogmios-url requires --topic-registry-addr, --publisher-vault-addr, \
                 and --registry-policy-id; falling back to mock chain state"
            ),
        }
    } else if let Some(key) = &args.blockfrost_key {
        match contract_addresses(args) {
            Some(c) => {
                info!(url = %args.blockfrost_url, "Using Blockfrost chain state backend");
                return Arc::new(CardanoChainState::blockfrost(key, &args.blockfrost_url, c));
            }
            None => warn!(
                "--blockfrost-key requires --topic-registry-addr, --publisher-vault-addr, \
                 and --registry-policy-id; falling back to mock chain state"
            ),
        }
    }

    info!("Using mock chain state (local testnet mode)");
    let mock_topics = default_topics(&args.topics);
    Arc::new(MockChainState::new(vec![], mock_topics))
}

/// Load a 32-byte Ed25519 seed from `path`, or generate and save a fresh one.
/// When `path` is `None` the key is ephemeral (not persisted).
fn load_or_generate_key(path: Option<&Path>) -> Result<[u8; 32]> {
    if let Some(p) = path {
        if p.exists() {
            let bytes = std::fs::read(p)
                .with_context(|| format!("Failed to read key file {}", p.display()))?;
            anyhow::ensure!(
                bytes.len() == 32,
                "Key file {} must be exactly 32 bytes (got {})",
                p.display(),
                bytes.len()
            );
            let mut seed = [0u8; 32];
            seed.copy_from_slice(&bytes);
            info!(path = %p.display(), "Loaded persistent node key");
            return Ok(seed);
        }
        let mut seed = [0u8; 32];
        getrandom::fill(&mut seed).expect("OS RNG failed");
        if let Some(parent) = p.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create key dir {}", parent.display()))?;
            }
        }
        std::fs::write(p, &seed)
            .with_context(|| format!("Failed to write key file {}", p.display()))?;
        info!(path = %p.display(), "Generated new node key (saved)");
        Ok(seed)
    } else {
        let mut seed = [0u8; 32];
        getrandom::fill(&mut seed).expect("OS RNG failed");
        Ok(seed)
    }
}

fn topic_id_from_name(name: &str) -> TopicId {
    use pallas_crypto::hash::Hasher;
    let hash = Hasher::<256>::hash(name.as_bytes());
    let mut id = [0u8; 32];
    id.copy_from_slice(hash.as_ref());
    TopicId(id)
}

fn default_topics(names: &[String]) -> Vec<TopicConfig> {
    names
        .iter()
        .map(|name| TopicConfig {
            topic_id: topic_id_from_name(name),
            name: name.clone(),
            description: None,
            authorized_publishers: vec![],
            retention_period: Duration::from_secs(3600),
            replication_factor: 1,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use bytes::Bytes;

    use pubsub_network::store::HotCache;
    use pubsub_types::message::{Message, PublisherCredential, PublisherId, TopicId};
    use pubsub_types::node::{NodeId, NodeInfo};
    use pubsub_types::traits::MessageStore;

    use super::api;

    fn fixture_msg() -> Message {
        Message {
            topic_id: TopicId([0xABu8; 32]),
            sequence_nr: 1,
            timestamp_ms: 0,
            publisher_id: PublisherId(PublisherCredential::ed25519(Bytes::from(vec![0xABu8; 32]))),
            signature: Bytes::new(),
            payload: Bytes::from_static(b"hello"),
            metadata: BTreeMap::new(),
        }
    }

    fn fixture_node() -> NodeInfo {
        NodeInfo {
            node_id: NodeId([0u8; 32]),
            addr: "127.0.0.1:0".parse().unwrap(),
            public_key: vec![],
            subscribed_topics: vec![],
        }
    }

    /// Runs the dedup+record pattern from the receive loop once.
    /// Returns true when the message was new (first sight).
    async fn receive_once(
        store: &HotCache,
        state: &api::ApiState,
        from: &NodeId,
        msg: &Message,
    ) -> bool {
        let already_seen = store.get(&msg.id()).await.ok().flatten().is_some();
        store.store(msg.clone()).await.unwrap();
        if !already_seen {
            state.record_message(from, msg, None).await;
        }
        !already_seen
    }

    /// Regression: the same message arriving via two gossip paths must appear
    /// exactly once in the API feed.  Before the fix, `record_message` ran
    /// before the disseminator's seen-set check, so duplicates slipped through.
    #[tokio::test]
    async fn duplicate_path_arrival_recorded_once() {
        let store = Arc::new(HotCache::with_defaults());
        let (state, _tx) = api::ApiState::new(
            fixture_node(),
            "preprod".into(),
            "psnode_test".into(),
        );

        let msg = fixture_msg();
        let peer_a = NodeId([1u8; 32]);
        let peer_b = NodeId([2u8; 32]);

        let first = receive_once(&store, &state, &peer_a, &msg).await;
        let second = receive_once(&store, &state, &peer_b, &msg).await;

        assert!(first, "first arrival should be new");
        assert!(!second, "second arrival via different path should be duplicate");

        let feed = state.recent_messages.read().await;
        assert_eq!(feed.len(), 1, "duplicate path must not add a second feed entry");
    }
}
