// =============================================================================
// Vicinity — Gossip-based topic navigation on a circular ring
// =============================================================================
//
// Implements the Vicinity / T-Man protocol for topic-space routing.
//
// Topics are mapped to positions on a circular ring of size T (2^32 by default)
// using the first 4 bytes of the TopicId.  Each node that subscribes to a
// topic occupies that topic's position on the ring.
//
// To enable O(log T) lookup, each node maintains *finger links* at
// exponentially increasing distances in both directions:
//
//     distances = b^0, b^1, b^2, ... (b = 2 by default)
//
// Each finger slot holds the peer whose subscribed-topic position is closest
// to the target distance.
//
// T-Man active exchange (cycle):
//   Each Vicinity cycle picks a random peer from the Cyclon view, exchanges
//   the local descriptor set with it, and merges the response to improve
//   finger quality.  Both sides benefit from each exchange.
//
// Key operations:
//   - `find_topic_peers()`: follow finger links toward the target topic.
//   - `join_topic()` / `leave_topic()`: update the local subscription set
//     and immediately notify current finger-link neighbors.
//   - `cycle()`: T-Man active exchange with a randomly-selected peer.
//   - `serve_gossip()`: respond to inbound Vicinity exchanges and topic
//     update notifications from other nodes.
// =============================================================================

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use pubsub_types::error::PubSubError;
use pubsub_types::message::TopicId;
use pubsub_types::node::{NodeId, NodeInfo, PeerDescriptor};
use pubsub_types::traits::{GossipTransport, PeerSampler, TopicRouter, GOSSIP_VICINITY};

// ---------------------------------------------------------------------------
// Wire protocol
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
enum VicinityMessage {
    /// T-Man active exchange — both sides send their best descriptor sets.
    Exchange { descriptors: Vec<PeerDescriptor> },
    /// A peer's subscription set has changed (join or leave).
    /// Contains the peer's full updated `NodeInfo`.
    TopicUpdate { node_info: NodeInfo },
}

// ---------------------------------------------------------------------------
// Ring geometry helpers
// ---------------------------------------------------------------------------

/// Ring size — we use 2^32 so that the first 4 bytes of a TopicId map
/// directly to a position.
const RING_SIZE: u64 = 1u64 << 32;

/// Map a TopicId to a position on the ring by interpreting the first 4 bytes
/// as a big-endian u32.
fn topic_position(topic: &TopicId) -> u64 {
    u32::from_be_bytes([topic.0[0], topic.0[1], topic.0[2], topic.0[3]]) as u64
}

/// Clockwise distance from `a` to `b` on the ring.
fn ring_distance_cw(a: u64, b: u64) -> u64 {
    if b >= a { b - a } else { RING_SIZE - a + b }
}

/// Shortest (undirected) distance on the ring.
fn ring_distance(a: u64, b: u64) -> u64 {
    ring_distance_cw(a, b).min(ring_distance_cw(b, a))
}

// ---------------------------------------------------------------------------
// Finger table
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Finger {
    ideal_distance: u64,
    peer: Option<PeerDescriptor>,
}

/// Configuration for the Vicinity layer.
#[derive(Debug, Clone)]
pub struct VicinityConfig {
    pub finger_base: u64,
    pub max_fingers: usize,
    pub gossip_sample_size: usize,
}

impl Default for VicinityConfig {
    fn default() -> Self {
        Self {
            finger_base: 2,
            max_fingers: 32,
            gossip_sample_size: 10,
        }
    }
}

// ---------------------------------------------------------------------------
// Inner state (behind RwLock)
// ---------------------------------------------------------------------------

struct VicinityState {
    local_subscriptions: HashSet<TopicId>,
    cw_fingers: Vec<Finger>,
    ccw_fingers: Vec<Finger>,
    peer_topics: HashMap<Vec<u8>, (PeerDescriptor, HashSet<u64>)>,
}

// ---------------------------------------------------------------------------
// Vicinity struct
// ---------------------------------------------------------------------------

pub struct Vicinity {
    local_info: NodeInfo,
    config: VicinityConfig,
    state: Arc<RwLock<VicinityState>>,
    peer_sampler: Arc<dyn PeerSampler>,
    gossip: Arc<dyn GossipTransport>,
}

impl Vicinity {
    pub fn new(
        local_info: NodeInfo,
        peer_sampler: Arc<dyn PeerSampler>,
        gossip: Arc<dyn GossipTransport>,
        config: VicinityConfig,
    ) -> Self {
        let mut cw_fingers = Vec::with_capacity(config.max_fingers);
        let mut ccw_fingers = Vec::with_capacity(config.max_fingers);
        let mut dist: u64 = 1;
        for _ in 0..config.max_fingers {
            if dist >= RING_SIZE / 2 {
                break;
            }
            cw_fingers.push(Finger { ideal_distance: dist, peer: None });
            ccw_fingers.push(Finger { ideal_distance: dist, peer: None });
            dist = dist.saturating_mul(config.finger_base);
        }

        info!(
            cw_fingers = cw_fingers.len(),
            ccw_fingers = ccw_fingers.len(),
            "Vicinity initialised"
        );

        Self {
            local_info,
            config,
            state: Arc::new(RwLock::new(VicinityState {
                local_subscriptions: HashSet::new(),
                cw_fingers,
                ccw_fingers,
                peer_topics: HashMap::new(),
            })),
            peer_sampler,
            gossip,
        }
    }

    // ---- helpers ------------------------------------------------------------

    fn local_position(subscriptions: &HashSet<TopicId>) -> Option<u64> {
        subscriptions.iter().map(topic_position).min()
    }

    fn is_better_finger(origin: u64, finger: &Finger, candidate_pos: u64, clockwise: bool) -> bool {
        let actual_dist = if clockwise {
            ring_distance_cw(origin, candidate_pos)
        } else {
            ring_distance_cw(candidate_pos, origin)
        };
        let deviation = actual_dist.abs_diff(finger.ideal_distance);

        match &finger.peer {
            None => true,
            Some(existing) => {
                let existing_pos = existing
                    .node_info
                    .subscribed_topics
                    .iter()
                    .map(topic_position)
                    .min()
                    .unwrap_or(0);
                let existing_dist = if clockwise {
                    ring_distance_cw(origin, existing_pos)
                } else {
                    ring_distance_cw(existing_pos, origin)
                };
                deviation < existing_dist.abs_diff(finger.ideal_distance)
            }
        }
    }

    fn try_improve_fingers(
        origin: u64,
        cw_fingers: &mut [Finger],
        ccw_fingers: &mut [Finger],
        candidates: &[PeerDescriptor],
    ) {
        for candidate in candidates {
            let cand_pos = match candidate.node_info.subscribed_topics.iter().map(topic_position).min() {
                Some(p) => p,
                None => continue,
            };
            for finger in cw_fingers.iter_mut() {
                if Self::is_better_finger(origin, finger, cand_pos, true) {
                    debug!(ideal = finger.ideal_distance, peer = ?candidate.node_info.node_id, "CW finger improved");
                    finger.peer = Some(candidate.clone());
                }
            }
            for finger in ccw_fingers.iter_mut() {
                if Self::is_better_finger(origin, finger, cand_pos, false) {
                    debug!(ideal = finger.ideal_distance, peer = ?candidate.node_info.node_id, "CCW finger improved");
                    finger.peer = Some(candidate.clone());
                }
            }
        }
    }

    /// Collect all unique peers across both finger tables.
    fn all_finger_peers(state: &VicinityState) -> Vec<PeerDescriptor> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for finger in state.cw_fingers.iter().chain(state.ccw_fingers.iter()) {
            if let Some(ref pd) = finger.peer {
                let key = pd.node_info.node_id.0.to_vec();
                if seen.insert(key) {
                    out.push(pd.clone());
                }
            }
        }
        out
    }

    /// Apply an incoming `TopicUpdate` to whatever finger entries reference that node.
    fn apply_topic_update(state: &mut VicinityState, updated: &NodeInfo) {
        let topics: HashSet<u64> = updated.subscribed_topics.iter().map(topic_position).collect();
        state.peer_topics.insert(
            updated.node_id.0.to_vec(),
            (PeerDescriptor { node_info: updated.clone(), age: 0 }, topics),
        );
        for finger in state.cw_fingers.iter_mut().chain(state.ccw_fingers.iter_mut()) {
            if let Some(ref mut pd) = finger.peer {
                if pd.node_info.node_id == updated.node_id {
                    pd.node_info = updated.clone();
                }
            }
        }
    }

    /// Respond to inbound Vicinity gossip (T-Man exchanges and topic updates).
    ///
    /// Spawn this in a dedicated task alongside the periodic `cycle()` loop.
    pub async fn serve_gossip(&self) {
        loop {
            match self.gossip.next_inbound_gossip(GOSSIP_VICINITY).await {
                Ok((from, request_bytes, resp_tx)) => {
                    let msg: VicinityMessage = match serde_json::from_slice(&request_bytes) {
                        Ok(m) => m,
                        Err(e) => {
                            warn!(?from, "Failed to decode vicinity message: {e}");
                            continue;
                        }
                    };

                    match msg {
                        VicinityMessage::Exchange { descriptors: received } => {
                            let response_descs = {
                                let mut state = self.state.write().await;
                                let origin = Self::local_position(&state.local_subscriptions);
                                if let Some(origin) = origin {
                                    let state_mut = &mut *state;
                                    Self::try_improve_fingers(
                                        origin,
                                        &mut state_mut.cw_fingers,
                                        &mut state_mut.ccw_fingers,
                                        &received,
                                    );
                                }
                                // Respond with our self-descriptor + all finger peers.
                                let mut descs = vec![PeerDescriptor {
                                    node_info: self.local_info.clone(),
                                    age: 0,
                                }];
                                descs.extend(Self::all_finger_peers(&state));
                                descs
                            };

                            let resp_msg = VicinityMessage::Exchange { descriptors: response_descs };
                            match serde_json::to_vec(&resp_msg) {
                                Ok(encoded) => { let _ = resp_tx.send(encoded); }
                                Err(e) => warn!("Failed to encode vicinity exchange response: {e}"),
                            }
                        }

                        VicinityMessage::TopicUpdate { node_info } => {
                            {
                                let mut state = self.state.write().await;
                                Self::apply_topic_update(&mut state, &node_info);
                            }
                            // ACK: empty exchange
                            let ack = VicinityMessage::Exchange { descriptors: vec![] };
                            let _ = resp_tx.send(serde_json::to_vec(&ack).unwrap_or_default());
                        }
                    }
                }
                Err(e) => {
                    warn!("Vicinity gossip serve error: {e}");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }

    /// Notify all current finger-link neighbors of a subscription change.
    async fn notify_neighbors(&self, updated_info: &NodeInfo) {
        let neighbor_info: Vec<(NodeId, std::net::SocketAddr)> = {
            let state = self.state.read().await;
            let mut seen = HashSet::new();
            let mut out = Vec::new();
            for finger in state.cw_fingers.iter().chain(state.ccw_fingers.iter()) {
                if let Some(ref pd) = finger.peer {
                    let key = pd.node_info.node_id.0.to_vec();
                    if seen.insert(key) {
                        out.push((pd.node_info.node_id.clone(), pd.node_info.addr));
                    }
                }
            }
            out
        };

        if neighbor_info.is_empty() {
            return;
        }

        let msg = VicinityMessage::TopicUpdate { node_info: updated_info.clone() };
        let encoded = match serde_json::to_vec(&msg) {
            Ok(b) => b,
            Err(e) => {
                warn!("Failed to encode TopicUpdate: {e}");
                return;
            }
        };

        for (neighbor_id, neighbor_addr) in neighbor_info {
            if let Err(e) = self
                .gossip
                .gossip_exchange(&neighbor_id, neighbor_addr, GOSSIP_VICINITY, encoded.clone())
                .await
            {
                debug!(error = %e, "Failed to notify neighbor of topic update");
            }
        }
    }
}

#[async_trait]
impl TopicRouter for Vicinity {
    async fn find_topic_peers(&self, topic: &TopicId, max_results: usize) -> Vec<PeerDescriptor> {
        let target_pos = topic_position(topic);
        let state = self.state.read().await;

        let mut results: Vec<PeerDescriptor> = Vec::new();
        let mut seen: HashSet<Vec<u8>> = HashSet::new();

        let all_fingers = state
            .cw_fingers
            .iter()
            .chain(state.ccw_fingers.iter())
            .filter_map(|f| f.peer.as_ref());

        let mut scored: Vec<(u64, &PeerDescriptor)> = Vec::new();
        for peer in all_fingers {
            let id_bytes = peer.node_info.node_id.0.to_vec();
            if !seen.insert(id_bytes) {
                continue;
            }
            let min_dist = peer
                .node_info
                .subscribed_topics
                .iter()
                .map(|t| ring_distance(topic_position(t), target_pos))
                .min()
                .unwrap_or(u64::MAX);

            if peer.node_info.subscribed_topics.iter().any(|t| t == topic) {
                results.push(peer.clone());
                if results.len() >= max_results {
                    return results;
                }
            } else {
                scored.push((min_dist, peer));
            }
        }

        scored.sort_by_key(|(dist, _)| *dist);
        for (dist, peer) in scored {
            if results.len() >= max_results {
                break;
            }
            debug!(distance = dist, peer = ?peer.node_info.node_id, "adding proximity result");
            results.push(peer.clone());
        }

        debug!(topic = ?topic, found = results.len(), "find_topic_peers complete");
        results
    }

    async fn join_topic(&self, topic: &TopicId) -> Result<(), PubSubError> {
        let updated_info = {
            let mut state = self.state.write().await;
            state.local_subscriptions.insert(topic.clone());
            info!(topic = ?topic, "joined topic");
            NodeInfo {
                node_id: self.local_info.node_id.clone(),
                addr: self.local_info.addr,
                public_key: self.local_info.public_key.clone(),
                subscribed_topics: state.local_subscriptions.iter().cloned().collect(),
            }
        };

        self.notify_neighbors(&updated_info).await;
        Ok(())
    }

    async fn leave_topic(&self, topic: &TopicId) -> Result<(), PubSubError> {
        let updated = {
            let mut state = self.state.write().await;
            let removed = state.local_subscriptions.remove(topic);
            if !removed {
                warn!(topic = ?topic, "leave_topic called but was not subscribed");
                return Ok(());
            }
            info!(topic = ?topic, "left topic");
            Some(NodeInfo {
                node_id: self.local_info.node_id.clone(),
                addr: self.local_info.addr,
                public_key: self.local_info.public_key.clone(),
                subscribed_topics: state.local_subscriptions.iter().cloned().collect(),
            })
        };

        if let Some(updated_info) = updated {
            self.notify_neighbors(&updated_info).await;
        }
        Ok(())
    }

    /// T-Man active exchange cycle.
    ///
    /// 1. Pick a random peer from the Cyclon sample.
    /// 2. Send our descriptor set (self + current finger peers).
    /// 3. Receive the peer's descriptor set.
    /// 4. Merge both sides' descriptors to improve finger quality.
    /// 5. Also passively improve using the full Cyclon sample.
    async fn cycle(&self) -> Result<(), PubSubError> {
        let sample = self.peer_sampler.sample(self.config.gossip_sample_size).await;
        if sample.is_empty() {
            debug!("vicinity cycle: peer sampler returned empty sample");
            return Ok(());
        }

        // T-Man: pick one random peer for the active exchange.
        // Drop rng before any await (ThreadRng is not Send).
        let target = {
            let mut rng = thread_rng();
            sample.choose(&mut rng).unwrap().clone()
        };

        // Build our descriptor set: self + all finger peers.
        let our_descriptors: Vec<PeerDescriptor> = {
            let state = self.state.read().await;
            let mut descs = vec![PeerDescriptor {
                node_info: self.local_info.clone(),
                age: 0,
            }];
            descs.extend(Self::all_finger_peers(&state));
            descs
        };

        let msg = VicinityMessage::Exchange { descriptors: our_descriptors };
        let encoded = serde_json::to_vec(&msg)
            .map_err(|e| PubSubError::Codec(format!("failed to encode vicinity exchange: {e}")))?;

        let response_bytes = self
            .gossip
            .gossip_exchange(&target.node_info.node_id, target.node_info.addr, GOSSIP_VICINITY, encoded)
            .await
            .map_err(|e| { debug!(error = %e, "vicinity exchange failed"); e })?;

        let response: VicinityMessage = serde_json::from_slice(&response_bytes)
            .map_err(|e| PubSubError::Codec(format!("failed to decode vicinity response: {e}")))?;

        if let VicinityMessage::Exchange { descriptors: received } = response {
            let mut state = self.state.write().await;
            let origin = match Self::local_position(&state.local_subscriptions) {
                Some(pos) => pos,
                None => {
                    debug!("vicinity cycle: no local subscriptions, skipping finger update");
                    return Ok(());
                }
            };

            // Cache sampled peers' topic sets.
            for peer in &sample {
                let topics: HashSet<u64> =
                    peer.node_info.subscribed_topics.iter().map(topic_position).collect();
                state.peer_topics.insert(peer.node_info.node_id.0.to_vec(), (peer.clone(), topics));
            }

            let state_mut = &mut *state;
            // Improve using peers received from the active exchange partner.
            Self::try_improve_fingers(origin, &mut state_mut.cw_fingers, &mut state_mut.ccw_fingers, &received);
            // Also improve passively using the full Cyclon sample.
            Self::try_improve_fingers(origin, &mut state_mut.cw_fingers, &mut state_mut.ccw_fingers, &sample);

            let occupied_cw = state.cw_fingers.iter().filter(|f| f.peer.is_some()).count();
            let occupied_ccw = state.ccw_fingers.iter().filter(|f| f.peer.is_some()).count();
            info!(occupied_cw, occupied_ccw, sampled = sample.len(), "vicinity cycle complete");
        }

        Ok(())
    }
}
