// =============================================================================
// Vicinity — Gossip-based topic navigation on a circular ring
// =============================================================================
//
// Implements the Vicinity / T-Man protocol for topic-space routing.
//
// Topics are mapped to positions on a circular ring of size 2^32 using the
// first 4 bytes of TopicId.  To find peers for topic T, each node maintains
// per-topic finger tables centred on topic_position(T), with slots at
// exponentially increasing distances in both directions:
//
//     distances = b^0, b^1, b^2, ...  (b = 2 by default)
//
// Per-topic tables (vs. a single global table) ensure that a node subscribing
// to topics spread across the ring discovers peers near each topic, not just
// near its lexicographically-smallest subscription.
//
// T-Man active exchange (cycle):
//   Each Vicinity cycle picks a random peer from the Cyclon view, exchanges
//   the local descriptor set, and merges the response to improve ALL per-topic
//   finger tables simultaneously.
//
// Key operations:
//   - `find_topic_peers()`: return known peers closest to the target topic.
//   - `get_topic_subscribers()`: return all peers known to subscribe to a topic
//     (used by the Harary graph builder in the dissemination layer).
//   - `join_topic()` / `leave_topic()`: create/remove per-topic finger table
//     and notify current finger neighbours.
//   - `cycle()`: T-Man active exchange.
//   - `serve_gossip()`: respond to inbound exchanges and topic-update notifications.
// =============================================================================

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rand::prelude::IndexedRandom;
use rand::rng;
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
    TopicUpdate { node_info: NodeInfo },
}

// ---------------------------------------------------------------------------
// Ring geometry helpers
// ---------------------------------------------------------------------------

const RING_SIZE: u64 = 1u64 << 32;

fn topic_position(topic: &TopicId) -> u64 {
    u32::from_be_bytes([topic.0[0], topic.0[1], topic.0[2], topic.0[3]]) as u64
}

fn ring_distance_cw(a: u64, b: u64) -> u64 {
    if b >= a { b - a } else { RING_SIZE - a + b }
}

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

fn make_finger_table(config: &VicinityConfig) -> (Vec<Finger>, Vec<Finger>) {
    let mut cw = Vec::with_capacity(config.max_fingers);
    let mut ccw = Vec::with_capacity(config.max_fingers);
    let mut dist: u64 = 1;
    for _ in 0..config.max_fingers {
        if dist >= RING_SIZE / 2 {
            break;
        }
        cw.push(Finger { ideal_distance: dist, peer: None });
        ccw.push(Finger { ideal_distance: dist, peer: None });
        dist = dist.saturating_mul(config.finger_base);
    }
    (cw, ccw)
}

// ---------------------------------------------------------------------------
// Inner state (behind RwLock)
// ---------------------------------------------------------------------------

struct VicinityState {
    local_subscriptions: HashSet<TopicId>,
    /// Per-topic finger tables: each topic T has fingers centred on topic_position(T).
    topic_fingers: HashMap<TopicId, (Vec<Finger>, Vec<Finger>)>,
    /// Known peers and their subscribed topic positions, updated on every exchange.
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
        info!("Vicinity initialised (per-topic finger tables)");
        Self {
            local_info,
            config,
            state: Arc::new(RwLock::new(VicinityState {
                local_subscriptions: HashSet::new(),
                topic_fingers: HashMap::new(),
                peer_topics: HashMap::new(),
            })),
            peer_sampler,
            gossip,
        }
    }

    // ---- helpers ------------------------------------------------------------

    /// Is `candidate` a better fit for `finger` than its current occupant?
    /// `origin` is the centre of the finger table (= topic_position for some T).
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

    /// Try to fill finger slots using `candidates`, relative to `origin`.
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
                    finger.peer = Some(candidate.clone());
                }
            }
            for finger in ccw_fingers.iter_mut() {
                if Self::is_better_finger(origin, finger, cand_pos, false) {
                    finger.peer = Some(candidate.clone());
                }
            }
        }
    }

    /// Improve all per-topic finger tables using `candidates`.
    fn improve_all_topic_fingers(state: &mut VicinityState, candidates: &[PeerDescriptor]) {
        for (topic, (cw, ccw)) in state.topic_fingers.iter_mut() {
            let origin = topic_position(topic);
            Self::try_improve_fingers(origin, cw, ccw, candidates);
        }
    }

    /// Collect all unique peers across all per-topic finger tables.
    fn all_finger_peers(state: &VicinityState) -> Vec<PeerDescriptor> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for (cw, ccw) in state.topic_fingers.values() {
            for finger in cw.iter().chain(ccw.iter()) {
                if let Some(ref pd) = finger.peer {
                    let key = pd.node_info.node_id.0.to_vec();
                    if seen.insert(key) {
                        out.push(pd.clone());
                    }
                }
            }
        }
        out
    }

    /// Apply an incoming `TopicUpdate`: refresh `peer_topics` and improve any
    /// matching per-topic finger tables.
    fn apply_topic_update(state: &mut VicinityState, updated: &NodeInfo) {
        let positions: HashSet<u64> = updated.subscribed_topics.iter().map(topic_position).collect();
        let pd = PeerDescriptor { node_info: updated.clone(), age: 0 };
        state.peer_topics.insert(updated.node_id.0.to_vec(), (pd.clone(), positions));
        // Update any finger slot that already holds this peer.
        for (cw, ccw) in state.topic_fingers.values_mut() {
            for finger in cw.iter_mut().chain(ccw.iter_mut()) {
                if let Some(ref mut fp) = finger.peer {
                    if fp.node_info.node_id == updated.node_id {
                        fp.node_info = updated.clone();
                    }
                }
            }
        }
        // Try to improve finger tables for topics this peer subscribes to.
        Self::improve_all_topic_fingers(state, std::slice::from_ref(&pd));
    }

    /// Respond to inbound Vicinity gossip (T-Man exchanges and topic updates).
    pub async fn serve_gossip(&self) {
        loop {
            match self.gossip.next_inbound_gossip(GOSSIP_VICINITY).await {
                Ok((_from, request_bytes, resp_tx)) => {
                    let msg: VicinityMessage = match serde_json::from_slice(&request_bytes) {
                        Ok(m) => m,
                        Err(e) => {
                            warn!("Failed to decode vicinity message: {e}");
                            continue;
                        }
                    };

                    match msg {
                        VicinityMessage::Exchange { descriptors: received } => {
                            let response_descs = {
                                let mut state = self.state.write().await;
                                // Cache received peers.
                                for peer in &received {
                                    let positions: HashSet<u64> = peer.node_info.subscribed_topics
                                        .iter().map(topic_position).collect();
                                    state.peer_topics.insert(
                                        peer.node_info.node_id.0.to_vec(),
                                        (peer.clone(), positions),
                                    );
                                }
                                Self::improve_all_topic_fingers(&mut state, &received);
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

    /// Notify all current finger-link neighbours of a subscription change.
    async fn notify_neighbors(&self, updated_info: &NodeInfo) {
        let neighbor_info: Vec<(NodeId, std::net::SocketAddr)> = {
            let state = self.state.read().await;
            let mut seen = HashSet::new();
            let mut out = Vec::new();
            for pd in Self::all_finger_peers(&state) {
                let key = pd.node_info.node_id.0.to_vec();
                if seen.insert(key) {
                    out.push((pd.node_info.node_id, pd.node_info.addr));
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
            Err(e) => { warn!("Failed to encode TopicUpdate: {e}"); return; }
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

        let Some((cw, ccw)) = state.topic_fingers.get(topic) else {
            return Vec::new();
        };

        let mut seen: HashSet<Vec<u8>> = HashSet::new();
        let mut direct: Vec<PeerDescriptor> = Vec::new();
        let mut proximity: Vec<(u64, PeerDescriptor)> = Vec::new();

        for finger in cw.iter().chain(ccw.iter()) {
            let Some(ref pd) = finger.peer else { continue };
            let key = pd.node_info.node_id.0.to_vec();
            if !seen.insert(key) {
                continue;
            }
            if pd.node_info.subscribed_topics.iter().any(|t| t == topic) {
                direct.push(pd.clone());
            } else {
                let dist = pd.node_info.subscribed_topics.iter()
                    .map(|t| ring_distance(topic_position(t), target_pos))
                    .min()
                    .unwrap_or(u64::MAX);
                proximity.push((dist, pd.clone()));
            }
        }

        proximity.sort_by_key(|(d, _)| *d);
        let mut results = direct;
        for (_, pd) in proximity {
            if results.len() >= max_results {
                break;
            }
            results.push(pd);
        }
        results.truncate(max_results);

        debug!(topic = %topic, found = results.len(), "find_topic_peers complete");
        results
    }

    async fn get_topic_subscribers(&self, topic: &TopicId) -> Vec<NodeId> {
        let target_pos = topic_position(topic);
        let state = self.state.read().await;
        let mut ids: Vec<NodeId> = state.peer_topics.values()
            .filter(|(_, positions)| positions.contains(&target_pos))
            .map(|(pd, _)| pd.node_info.node_id.clone())
            .collect();
        if state.local_subscriptions.contains(topic) {
            ids.push(self.local_info.node_id.clone());
        }
        ids
    }

    async fn join_topic(&self, topic: &TopicId) -> Result<(), PubSubError> {
        let updated_info = {
            let mut state = self.state.write().await;
            state.local_subscriptions.insert(topic.clone());
            state.topic_fingers
                .entry(topic.clone())
                .or_insert_with(|| make_finger_table(&self.config));
            // Seed the new table immediately from known peers.
            let all_peers: Vec<PeerDescriptor> = state.peer_topics.values()
                .map(|(pd, _)| pd.clone())
                .collect();
            let origin = topic_position(topic);
            let (cw, ccw) = state.topic_fingers.get_mut(topic).unwrap();
            Self::try_improve_fingers(origin, cw, ccw, &all_peers);
            info!(topic = %topic, "joined topic");
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
            if !state.local_subscriptions.remove(topic) {
                warn!(topic = %topic, "leave_topic called but was not subscribed");
                return Ok(());
            }
            state.topic_fingers.remove(topic);
            info!(topic = %topic, "left topic");
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
    /// Exchanges our descriptor set with one randomly-chosen Cyclon peer, then
    /// merges received + sampled descriptors into ALL per-topic finger tables.
    /// One exchange per cycle is sufficient because the received candidates are
    /// evaluated against every topic's origin independently.
    async fn cycle(&self) -> Result<(), PubSubError> {
        let sample = self.peer_sampler.sample(self.config.gossip_sample_size).await;
        if sample.is_empty() {
            debug!("vicinity cycle: empty sample");
            return Ok(());
        }

        let target = {
            let mut rng = rng();
            sample.choose(&mut rng).unwrap().clone()
        };

        let our_descriptors: Vec<PeerDescriptor> = {
            let state = self.state.read().await;
            let mut descs = vec![PeerDescriptor { node_info: self.local_info.clone(), age: 0 }];
            descs.extend(Self::all_finger_peers(&state));
            descs
        };

        let msg = VicinityMessage::Exchange { descriptors: our_descriptors };
        let encoded = serde_json::to_vec(&msg)
            .map_err(|e| PubSubError::Codec(format!("vicinity encode: {e}")))?;

        let response_bytes = self
            .gossip
            .gossip_exchange(&target.node_info.node_id, target.node_info.addr, GOSSIP_VICINITY, encoded)
            .await
            .map_err(|e| { debug!(error = %e, "vicinity exchange failed"); e })?;

        let response: VicinityMessage = serde_json::from_slice(&response_bytes)
            .map_err(|e| PubSubError::Codec(format!("vicinity decode: {e}")))?;

        if let VicinityMessage::Exchange { descriptors: received } = response {
            let mut state = self.state.write().await;
            // Update peer_topics cache.
            for peer in sample.iter().chain(received.iter()) {
                let positions: HashSet<u64> =
                    peer.node_info.subscribed_topics.iter().map(topic_position).collect();
                state.peer_topics.insert(peer.node_info.node_id.0.to_vec(), (peer.clone(), positions));
            }
            // Merge all candidates into every per-topic finger table.
            let all_candidates: Vec<PeerDescriptor> = sample.iter().chain(received.iter()).cloned().collect();
            Self::improve_all_topic_fingers(&mut state, &all_candidates);

            let total_filled: usize = state.topic_fingers.values()
                .map(|(cw, ccw)| cw.iter().chain(ccw.iter()).filter(|f| f.peer.is_some()).count())
                .sum();
            info!(
                topics = state.topic_fingers.len(),
                filled_slots = total_filled,
                "vicinity cycle complete"
            );
        }

        Ok(())
    }
}
