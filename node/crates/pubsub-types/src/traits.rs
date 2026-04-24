use std::collections::HashSet;

use async_trait::async_trait;
use bytes::Bytes;

use crate::error::PubSubError;
use crate::message::{Message, MessageId, TopicId};
use crate::node::{NodeId, NodeInfo, PeerDescriptor};
use crate::topic::TopicConfig;

// =============================================================================
// Network Layer Traits
// =============================================================================

/// Transport layer — manages connections to other PubSub nodes.
/// Phase 1: QUIC. Later: WebTransport, TCP fallback.
#[async_trait]
pub trait Transport: Send + Sync + 'static {
    /// Send raw bytes to a specific peer
    async fn send(&self, peer: &NodeId, data: &[u8]) -> Result<(), PubSubError>;

    /// Receive raw bytes from any connected peer.
    /// Returns (sender, data).
    async fn recv(&self) -> Result<(NodeId, Vec<u8>), PubSubError>;

    /// Establish a connection to a peer
    async fn connect(&self, info: &NodeInfo) -> Result<(), PubSubError>;

    /// Disconnect from a peer
    async fn disconnect(&self, peer: &NodeId) -> Result<(), PubSubError>;

    /// Currently connected peers
    async fn connected_peers(&self) -> Vec<NodeId>;
}

/// Protocol discriminator byte prepended to all gossip messages so inbound
/// requests can be routed to the correct handler without a shared channel.
pub const GOSSIP_CYCLON: u8 = 0x01;
pub const GOSSIP_VICINITY: u8 = 0x02;

/// Gossip transport — bidirectional request/response exchanges over QUIC
/// bidirectional streams, completely separate from the unidirectional
/// application-message channel used by [`Transport`].
///
/// Each gossip round-trip opens its own QUIC bidirectional stream: the
/// initiator writes the request (with a one-byte `tag` prefix), reads the
/// response from the same stream, and closes it.  Responses are therefore
/// *never* co-mingled with application messages on the shared receive channel.
///
/// Incoming gossip is routed to per-protocol channels by the transport based
/// on the leading tag byte.  Pass `GOSSIP_CYCLON` or `GOSSIP_VICINITY` to
/// `next_inbound_gossip` to receive only the messages for that protocol.
///
/// The `oneshot::Sender` returned by `next_inbound_gossip` is how the
/// handler returns its response: send the bytes through the channel and the
/// transport writes them back to the peer on the same stream.
pub type InboundGossip = (NodeId, Vec<u8>, tokio::sync::oneshot::Sender<Vec<u8>>);

#[async_trait]
pub trait GossipTransport: Send + Sync + 'static {
    /// Initiate a round-trip with `peer` at `addr`:
    ///   - `tag`     one of `GOSSIP_CYCLON` / `GOSSIP_VICINITY`
    ///   - `request` protocol payload (tag is prepended by the transport)
    ///
    /// Blocks until the peer's response arrives on the same bidirectional stream.
    /// If `peer` is not yet connected the transport connects to `addr` first.
    async fn gossip_exchange(
        &self,
        peer: &NodeId,
        addr: std::net::SocketAddr,
        tag: u8,
        request: Vec<u8>,
    ) -> Result<Vec<u8>, PubSubError>;

    /// Block until an inbound gossip request for `tag` arrives from any peer.
    /// The caller must send a response through the returned oneshot sender;
    /// dropping the sender closes the stream without a reply.
    async fn next_inbound_gossip(&self, tag: u8) -> Result<InboundGossip, PubSubError>;
}

/// Peer sampling — maintains the overlay and provides random peer views.
/// Phase 1: Cyclon.
#[async_trait]
pub trait PeerSampler: Send + Sync + 'static {
    /// Get a random sample of peers from the current view
    async fn sample(&self, count: usize) -> Vec<PeerDescriptor>;

    /// Get the current full view
    async fn view(&self) -> Vec<PeerDescriptor>;

    /// Run one gossip exchange cycle (called periodically)
    async fn cycle(&self) -> Result<(), PubSubError>;

    /// Bootstrap the view with initial peers
    async fn bootstrap(&self, peers: Vec<NodeInfo>) -> Result<(), PubSubError>;
}

/// Topic navigation — efficient routing to topic subscribers.
/// Phase 1: Vicinity with circular topic ordering.
#[async_trait]
pub trait TopicRouter: Send + Sync + 'static {
    /// Find peers subscribed to a given topic (or closest to it in the topic ring)
    async fn find_topic_peers(
        &self,
        topic: &TopicId,
        max_results: usize,
    ) -> Vec<PeerDescriptor>;

    /// Return all nodes known to subscribe to this topic.
    ///
    /// Used by the Harary graph builder: H(t,n) requires the complete set of
    /// n subscribers to guarantee t-connectivity.  Vicinity's peer_topics map
    /// is the authoritative local view of per-topic membership.
    async fn get_topic_subscribers(&self, topic: &TopicId) -> Vec<NodeId>;

    /// Announce that this node subscribes to a topic
    async fn join_topic(&self, topic: &TopicId) -> Result<(), PubSubError>;

    /// Leave a topic
    async fn leave_topic(&self, topic: &TopicId) -> Result<(), PubSubError>;

    /// Run one Vicinity gossip cycle (called periodically)
    async fn cycle(&self) -> Result<(), PubSubError>;
}

/// Message dissemination within a topic — Harary graph + random links.
#[async_trait]
pub trait Disseminator: Send + Sync + 'static {
    /// Disseminate a message to all subscribers of its topic
    async fn disseminate(&self, msg: &Message) -> Result<(), PubSubError>;

    /// Handle a message received from a peer (forward if not seen before)
    async fn on_receive(
        &self,
        from: &NodeId,
        msg: Message,
    ) -> Result<(), PubSubError>;

    /// Get the Harary neighbors and random links for a topic
    async fn topic_links(&self, topic: &TopicId) -> TopicLinks;
}

/// The set of links maintained for a single topic's dissemination
#[derive(Debug, Clone, Default)]
pub struct TopicLinks {
    /// Deterministic neighbors (guaranteed delivery)
    pub neighbors: Vec<NodeId>,
    /// Random links (fast propagation)
    pub random_links: Vec<NodeId>,
}

// =============================================================================
// Message Processing Traits
// =============================================================================

/// Codec — serialize/deserialize messages.
/// Phase 1: CBOR. Later: per-topic encoding.
pub trait Codec: Send + Sync + 'static {
    fn encode(&self, msg: &Message) -> Result<Vec<u8>, PubSubError>;
    fn decode(&self, data: &[u8]) -> Result<Message, PubSubError>;
}

/// Message validation — verify signatures and authorization.
#[async_trait]
pub trait MessageValidator: Send + Sync + 'static {
    /// Validate a message: check signature, check publisher is authorized for topic.
    /// Returns Ok(()) if valid, Err with reason if not.
    async fn validate(&self, msg: &Message) -> Result<(), PubSubError>;
}

/// Relay policy — decide whether to forward a received message.
/// Phase 1: forward everything valid. Later: rate-limiting, reputation, BFT checks.
#[async_trait]
pub trait RelayPolicy: Send + Sync + 'static {
    /// Decide whether to relay a message. Called after validation passes.
    async fn should_relay(&self, msg: &Message, from: &NodeId) -> RelayDecision;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelayDecision {
    /// Forward the message normally
    Forward,
    /// Drop the message (with reason for logging)
    Drop(String),
    /// Rate-limited — delay forwarding
    Delay(std::time::Duration),
}

/// Message store — cache messages for retrieval by offline subscribers.
/// Phase 1: in-memory hot cache with TTL. Later: D2 clique-DHT.
#[async_trait]
pub trait MessageStore: Send + Sync + 'static {
    /// Store a message
    async fn store(&self, msg: Message) -> Result<(), PubSubError>;

    /// Retrieve a specific message by ID
    async fn get(&self, id: &MessageId) -> Result<Option<Message>, PubSubError>;

    /// Retrieve messages for a topic since a given sequence number
    /// (used by subscribers catching up after being offline)
    async fn get_since(
        &self,
        topic: &TopicId,
        since_sequence_nr: u64,
        limit: usize,
    ) -> Result<Vec<Message>, PubSubError>;

    /// Evict expired messages (called periodically)
    async fn evict_expired(&self) -> Result<usize, PubSubError>;
}

// =============================================================================
// Chain Interface Traits
// =============================================================================

/// Read Cardano L1 state — stake snapshots, registries.
/// Phase 1: mock. Later: ogmios or cardano-node local socket.
#[async_trait]
pub trait ChainState: Send + Sync + 'static {
    /// Get the current set of registered PubSub relay nodes
    async fn get_registered_nodes(&self) -> Result<Vec<NodeInfo>, PubSubError>;

    /// Get topic configuration from the on-chain Topic Registry
    async fn get_topic_config(
        &self,
        topic: &TopicId,
    ) -> Result<Option<TopicConfig>, PubSubError>;

    /// Get all registered topics
    async fn get_all_topics(&self) -> Result<Vec<TopicConfig>, PubSubError>;

    /// Get the stake associated with a node (for future use in weighted selection)
    async fn get_node_stake(&self, node: &NodeId) -> Result<u64, PubSubError>;

    /// Return all KES public keys currently registered for any stake pool.
    /// Phase 1: returns a small hardcoded set from MockChainState.
    /// Production: queries the on-chain Pool Registry via Ogmios.
    async fn get_pool_kes_keys(&self) -> Result<Vec<Bytes>, PubSubError>;

    /// Return all signing keys registered for DRep credentials (CIP-1694).
    /// Phase 1: returns a small hardcoded set from MockChainState.
    async fn get_drep_keys(&self) -> Result<Vec<Bytes>, PubSubError>;

    /// Return the curated list of authority public keys authorised to publish
    /// emergency alerts.
    /// Phase 1: returns a small hardcoded set from MockChainState.
    async fn get_authority_keys(&self) -> Result<Vec<Bytes>, PubSubError>;
}

// =============================================================================
// Consumer API Traits
// =============================================================================

/// Subscription manager — tracks which topics local consumers are subscribed to.
#[async_trait]
pub trait SubscriptionManager: Send + Sync + 'static {
    /// Subscribe a local consumer to a topic
    async fn subscribe(&self, topic: &TopicId) -> Result<(), PubSubError>;

    /// Unsubscribe from a topic
    async fn unsubscribe(&self, topic: &TopicId) -> Result<(), PubSubError>;

    /// Get all locally subscribed topics
    async fn subscriptions(&self) -> HashSet<TopicId>;

    /// Deliver a message to local subscribers (called when dissemination layer receives a msg)
    async fn deliver(&self, msg: Message) -> Result<(), PubSubError>;
}

// =============================================================================
// Node Registry Trait
// =============================================================================

/// Node registry — tracks registered relay nodes.
/// Phase 1: in-memory mock. Later: on-chain Cardano registry via Plutus script.
///
/// Separate from ChainState: ChainState is read-only L1 observation;
/// NodeRegistry is the mutable local view a node maintains of its peers.
#[async_trait]
pub trait NodeRegistry: Send + Sync + 'static {
    /// Register this node (or refresh an existing registration).
    /// `commitment_epochs` is ignored in Phase 1; reserved for stake-weighted
    /// registration in the production on-chain contract.
    async fn register(
        &self,
        info: NodeInfo,
        commitment_epochs: u32,
    ) -> Result<(), PubSubError>;

    /// Remove a node from the registry.
    async fn deregister(&self, node_id: &NodeId) -> Result<(), PubSubError>;

    /// Return all currently registered relay nodes.
    async fn get_registered_nodes(&self) -> Result<Vec<NodeInfo>, PubSubError>;

    /// Look up a specific node by ID.
    async fn get_node(&self, node_id: &NodeId) -> Result<Option<NodeInfo>, PubSubError>;

    /// Check whether a node is currently registered.
    async fn is_registered(&self, node_id: &NodeId) -> Result<bool, PubSubError>;
}
