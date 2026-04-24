// =============================================================================
// HybridDisseminator — Harary graph + random links per topic
// =============================================================================
//
// Message dissemination within a topic uses two complementary overlay
// structures:
//
// 1. **Harary graph** — deterministic, fault-tolerant backbone.
//    For a given topic, all subscribed nodes are arranged in a cyclic order
//    (sorted by NodeId).  Each node links to the t/2 closest peers in each
//    direction, where t is the fault-tolerance parameter (default 6).  A
//    Harary graph with parameter t is t-connected, meaning it can tolerate up
//    to t-1 simultaneous node failures and still remain connected.
//
// 2. **Random links** — probabilistic fast paths.
//    Each node additionally maintains `fanout` random links per topic
//    (default 3), refreshed from the PeerSampler.  These dramatically reduce
//    the diameter of the overlay, giving near-logarithmic propagation latency.
//
// When a message arrives (via `on_receive`) or is published locally (via
// `disseminate`), the node:
//   - Checks its seen-set for deduplication.
//   - If new: marks it seen, delivers to local subscriptions, and forwards to
//     all Harary neighbors + random links.
//
// The seen-set is bounded (default 10 000 entries) and evicts oldest entries
// to prevent memory leaks in long-running nodes.
// =============================================================================

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use pubsub_types::error::PubSubError;
use pubsub_types::message::{Message, MessageId, TopicId};
use pubsub_types::node::NodeId;
use pubsub_types::traits::{
    Codec, Disseminator, PeerSampler, SubscriptionManager, TopicLinks, Transport,
};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Tuning parameters for the hybrid dissemination layer.
#[derive(Debug, Clone)]
pub struct DisseminationConfig {
    /// Harary fault-tolerance parameter.  Each node connects to t/2 peers in
    /// each direction on the cyclic ordering, giving a t-connected graph.
    pub fault_tolerance: usize,

    /// Number of random links per topic (on top of the Harary backbone).
    pub fanout: usize,

    /// Maximum entries in the seen-set before oldest entries are evicted.
    pub seen_set_capacity: usize,
}

impl Default for DisseminationConfig {
    fn default() -> Self {
        Self {
            fault_tolerance: 6,
            fanout: 3,
            seen_set_capacity: 10_000,
        }
    }
}

// ---------------------------------------------------------------------------
// Per-topic link state
// ---------------------------------------------------------------------------

/// The set of links maintained for a single topic.
#[derive(Debug, Clone, Default)]
struct TopicLinkState {
    /// Deterministic neighbors (based on cyclic NodeId ordering).
    neighbors: Vec<NodeId>,
    /// Random links from PeerSampler (probabilistic fast paths).
    random: Vec<NodeId>,
}

// ---------------------------------------------------------------------------
// Bounded seen-set
// ---------------------------------------------------------------------------

/// A set with bounded capacity that evicts the oldest entries (FIFO) once full.
struct BoundedSeenSet {
    capacity: usize,
    /// Insertion-ordered queue used to know which entry to evict next.
    order: VecDeque<MessageId>,
    /// Fast membership test.
    set: HashSet<MessageId>,
}

impl BoundedSeenSet {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            order: VecDeque::with_capacity(capacity),
            set: HashSet::with_capacity(capacity),
        }
    }

    /// Returns `true` if the message was already in the set.
    fn contains(&self, id: &MessageId) -> bool {
        self.set.contains(id)
    }

    /// Insert a message ID.  If the set is at capacity, the oldest entry is
    /// evicted first.
    fn insert(&mut self, id: MessageId) {
        if self.set.contains(&id) {
            return;
        }
        if self.order.len() >= self.capacity {
            if let Some(evicted) = self.order.pop_front() {
                self.set.remove(&evicted);
            }
        }
        self.set.insert(id.clone());
        self.order.push_back(id);
    }
}

// ---------------------------------------------------------------------------
// Inner mutable state
// ---------------------------------------------------------------------------

struct DisseminationState {
    /// Per-topic link tables.
    topic_links: HashMap<TopicId, TopicLinkState>,
    /// Deduplication set.
    seen: BoundedSeenSet,
}

// ---------------------------------------------------------------------------
// HybridDisseminator
// ---------------------------------------------------------------------------

/// Hybrid message dissemination service combining a Harary backbone with
/// random overlay links.
pub struct HybridDisseminator {
    local_id: NodeId,
    config: DisseminationConfig,
    state: Arc<RwLock<DisseminationState>>,

    /// Transport for sending messages to peers.
    transport: Arc<dyn Transport>,

    /// Codec for encoding messages before forwarding.
    codec: Arc<dyn Codec>,

    /// Peer sampler for refreshing random links.
    peer_sampler: Arc<dyn PeerSampler>,

    /// Subscription manager for delivering messages to local consumers.
    subscription_mgr: Arc<dyn SubscriptionManager>,
}

impl HybridDisseminator {
    /// Create a new HybridDisseminator.
    pub fn new(
        local_id: NodeId,
        transport: Arc<dyn Transport>,
        codec: Arc<dyn Codec>,
        peer_sampler: Arc<dyn PeerSampler>,
        subscription_mgr: Arc<dyn SubscriptionManager>,
        config: DisseminationConfig,
    ) -> Self {
        info!(
            fault_tolerance = config.fault_tolerance,
            fanout = config.fanout,
            seen_cap = config.seen_set_capacity,
            "HybridDisseminator initialised"
        );
        Self {
            local_id,
            state: Arc::new(RwLock::new(DisseminationState {
                topic_links: HashMap::new(),
                seen: BoundedSeenSet::new(config.seen_set_capacity),
            })),
            config,
            transport,
            codec,
            peer_sampler,
            subscription_mgr,
        }
    }

    // ---- Harary graph construction -----------------------------------------

    /// Given an ordered list of NodeIds (sorted) for a topic, compute this
    /// node's Harary neighbors.
    ///
    /// Harary(t, n): arrange n nodes in a cycle.  Each node connects to the
    /// floor(t/2) nearest in each direction.  The resulting graph is
    /// t-connected.
    fn compute_neighbors(
        sorted_nodes: &[NodeId],
        local_id: &NodeId,
        fault_tolerance: usize,
    ) -> Vec<NodeId> {
        let n = sorted_nodes.len();
        if n <= 1 {
            return Vec::new();
        }

        // Find our index in the sorted list.
        let our_idx = match sorted_nodes.iter().position(|id| id == local_id) {
            Some(idx) => idx,
            None => return Vec::new(), // we are not in the topic
        };

        let half_t = fault_tolerance / 2;
        let mut neighbors = Vec::with_capacity(half_t * 2);

        // Connect to the `half_t` closest in each direction (wrapping).
        for offset in 1..=half_t {
            // Clockwise neighbor.
            let cw_idx = (our_idx + offset) % n;
            if sorted_nodes[cw_idx] != *local_id {
                neighbors.push(sorted_nodes[cw_idx].clone());
            }

            // Counter-clockwise neighbor.
            let ccw_idx = (our_idx + n - offset) % n;
            if sorted_nodes[ccw_idx] != *local_id
                && !neighbors.iter().any(|id| id == &sorted_nodes[ccw_idx])
            {
                neighbors.push(sorted_nodes[ccw_idx].clone());
            }
        }

        // For odd fault_tolerance, add one more CW link (Harary prescription).
        if fault_tolerance % 2 == 1 && n > fault_tolerance {
            let extra_idx = (our_idx + n / 2) % n;
            if sorted_nodes[extra_idx] != *local_id
                && !neighbors.iter().any(|id| id == &sorted_nodes[extra_idx])
            {
                neighbors.push(sorted_nodes[extra_idx].clone());
            }
        }

        neighbors
    }

    /// (Re-)compute Harary links for a topic given the current set of
    /// subscriber NodeIds.
    pub async fn rebuild_neighbors(
        &self,
        topic: &TopicId,
        mut subscriber_ids: Vec<NodeId>,
    ) {
        // Sort for deterministic cyclic ordering.
        subscriber_ids.sort_by(|a, b| a.0.cmp(&b.0));

        let neighbors = Self::compute_neighbors(
            &subscriber_ids,
            &self.local_id,
            self.config.fault_tolerance,
        );

        debug!(
            topic = ?topic,
            neighbor_count = neighbors.len(),
            "rebuilt neighbors"
        );

        let mut state = self.state.write().await;
        let entry = state
            .topic_links
            .entry(topic.clone())
            .or_insert_with(TopicLinkState::default);
        entry.neighbors = neighbors;
    }

    /// Refresh random links for a topic by sampling from the PeerSampler and
    /// filtering for peers subscribed to this topic.
    pub async fn refresh_random_links(&self, topic: &TopicId) {
        let sample = self.peer_sampler.sample(self.config.fanout * 3).await;

        let random_links: Vec<NodeId> = sample
            .into_iter()
            .filter(|pd| {
                pd.node_info.subscribed_topics.contains(topic)
                    && pd.node_info.node_id != self.local_id
            })
            .take(self.config.fanout)
            .map(|pd| pd.node_info.node_id)
            .collect();

        debug!(
            topic = ?topic,
            random_count = random_links.len(),
            "refreshed random links"
        );

        let mut state = self.state.write().await;
        let entry = state
            .topic_links
            .entry(topic.clone())
            .or_insert_with(TopicLinkState::default);
        entry.random = random_links;
    }

    // ---- forwarding --------------------------------------------------------

    /// Forward a message to all Harary neighbors and random links for its
    /// topic, excluding `exclude_peer` (the node we received it from, if any).
    async fn forward(
        &self,
        msg: &Message,
        exclude_peer: Option<&NodeId>,
    ) -> Result<(), PubSubError> {
        let state = self.state.read().await;
        let links = match state.topic_links.get(&msg.topic_id) {
            Some(l) => l,
            None => {
                debug!(topic = ?msg.topic_id, "no links for topic, skipping forward");
                return Ok(());
            }
        };

        // Encode once, send to many.
        let encoded = self.codec.encode(msg)?;

        let all_targets = links.neighbors.iter().chain(links.random.iter());
        for peer_id in all_targets {
            // Don't send back to whoever sent it to us.
            if let Some(excl) = exclude_peer {
                if peer_id == excl {
                    continue;
                }
            }
            // Don't send to ourselves.
            if peer_id == &self.local_id {
                continue;
            }
            if let Err(e) = self.transport.send(peer_id, &encoded).await {
                warn!(peer = ?peer_id, error = %e, "failed to forward message");
                // Non-fatal: keep forwarding to other peers.
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Disseminator for HybridDisseminator {
    /// Disseminate a locally-published message to the topic's overlay.
    ///
    /// Called by the local publisher API.  Marks the message as seen,
    /// delivers to local subscriptions, and forwards to all peers.
    async fn disseminate(&self, msg: &Message) -> Result<(), PubSubError> {
        let msg_id = msg.id();

        // Mark as seen so we don't process our own message again if it
        // comes back via a forwarding loop.
        {
            let mut state = self.state.write().await;
            if state.seen.contains(&msg_id) {
                debug!(msg_id = ?msg_id, "disseminate: already seen, dropping");
                return Ok(());
            }
            state.seen.insert(msg_id);
        }

        // Deliver to local consumers.
        if let Err(e) = self.subscription_mgr.deliver(msg.clone()).await {
            warn!(error = %e, "failed to deliver locally");
        }

        // Lazily build links if not yet initialised for this topic.
        let needs_init = self.state.read().await.topic_links
            .get(&msg.topic_id).map(|l| l.neighbors.is_empty() && l.random.is_empty())
            .unwrap_or(true);
        if needs_init {
            let view = self.peer_sampler.view().await;
            let subscriber_ids: Vec<NodeId> = std::iter::once(self.local_id.clone())
                .chain(
                    view.iter()
                        .filter(|pd| pd.node_info.subscribed_topics.contains(&msg.topic_id))
                        .map(|pd| pd.node_info.node_id.clone()),
                )
                .collect();
            self.rebuild_neighbors(&msg.topic_id, subscriber_ids).await;
            self.refresh_random_links(&msg.topic_id).await;
        }

        // Forward to all overlay peers (no exclusion — we are the origin).
        self.forward(msg, None).await?;

        info!(topic = ?msg.topic_id, seq = msg.sequence_nr, "message disseminated");
        Ok(())
    }

    /// Handle a message received from a remote peer.
    ///
    /// Deduplication check -> deliver locally -> forward to remaining peers.
    async fn on_receive(
        &self,
        from: &NodeId,
        msg: Message,
    ) -> Result<(), PubSubError> {
        let msg_id = msg.id();

        // --- Deduplication ---
        {
            let mut state = self.state.write().await;
            if state.seen.contains(&msg_id) {
                debug!(from = ?from, "on_receive: duplicate, dropping");
                return Ok(());
            }
            state.seen.insert(msg_id);
        }

        debug!(
            from = ?from,
            topic = ?msg.topic_id,
            seq = msg.sequence_nr,
            "on_receive: new message"
        );

        // --- Local delivery ---
        if let Err(e) = self.subscription_mgr.deliver(msg.clone()).await {
            warn!(error = %e, "failed to deliver locally on receive");
        }

        // Lazily build Harary + random links for this topic if none exist yet.
        let needs_init = self.state.read().await.topic_links
            .get(&msg.topic_id).map(|l| l.neighbors.is_empty() && l.random.is_empty())
            .unwrap_or(true);
        if needs_init {
            let view = self.peer_sampler.view().await;
            let subscriber_ids: Vec<NodeId> = std::iter::once(self.local_id.clone())
                .chain(
                    view.iter()
                        .filter(|pd| pd.node_info.subscribed_topics.contains(&msg.topic_id))
                        .map(|pd| pd.node_info.node_id.clone()),
                )
                .collect();
            self.rebuild_neighbors(&msg.topic_id, subscriber_ids).await;
            self.refresh_random_links(&msg.topic_id).await;
        }

        // --- Forward to peers (excluding sender) ---
        self.forward(&msg, Some(from)).await?;

        Ok(())
    }

    /// Return the current Harary + random link set for a topic.
    async fn topic_links(&self, topic: &TopicId) -> TopicLinks {
        let state = self.state.read().await;
        match state.topic_links.get(topic) {
            Some(links) => TopicLinks {
                neighbors: links.neighbors.clone(),
                random_links: links.random.clone(),
            },
            None => TopicLinks::default(),
        }
    }
}
