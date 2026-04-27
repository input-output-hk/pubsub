// =============================================================================
// Cyclon — Gossip-based peer sampling service (with SecureCyclon extensions)
// =============================================================================
//
// Implements the Cyclon protocol (Voulgaris, Gavidia & van Steen, 2005) with
// the three eclipse-resistance extensions from Jesi, Montresor & Babaoglu
// ("SecureCyclon", 2007):
//
//   1. Signed PeerDescriptors — each node signs its own descriptor with its
//      Ed25519 key; recipients verify before inserting into their view.
//      Self-certifying: NodeId = Blake2b-256(public_key); no registry needed.
//   2. Bootstrap diversity — view is considered "warm" only after receiving
//      descriptors from ≥ min_seed_diversity distinct seed origins.
//   3. Rate-limited replacement — new-peer insertions are capped per
//      merge_received call (default: ≤ 50% of view_size) to slow eclipse.
//
// Each gossip cycle opens a QUIC **bidirectional** stream to the oldest peer,
// sends a shuffle buffer, reads the response from the same stream, and closes
// it.  Gossip traffic is therefore completely separate from the unidirectional
// application-message channel — they can never steal each other's messages.
// =============================================================================

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use pallas_crypto::key::ed25519::SecretKey;
use rand::prelude::SliceRandom;
use rand::rng;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use pubsub_types::error::PubSubError;
use pubsub_types::node::{NodeId, NodeInfo, PeerDescriptor};
use pubsub_types::traits::{GossipTransport, PeerSampler, Transport, GOSSIP_CYCLON};

#[derive(Debug, Clone)]
pub struct CyclonConfig {
    pub view_size: usize,
    pub shuffle_length: usize,
    /// Reject incoming descriptors whose signature is missing or invalid.
    pub verify_signatures: bool,
    /// Minimum number of distinct seed origins before the view is "warm".
    pub min_seed_diversity: usize,
    /// Maximum new peers inserted per merge_received call (0 = unlimited).
    /// Defaults to view_size / 2 to slow eclipse attacks.
    pub max_new_per_merge: usize,
}

impl Default for CyclonConfig {
    fn default() -> Self {
        Self {
            view_size: 20,
            shuffle_length: 10,
            verify_signatures: true,
            min_seed_diversity: 2,
            max_new_per_merge: 10, // 50% of default view_size
        }
    }
}

pub struct Cyclon {
    local_info: NodeInfo,
    view: Arc<RwLock<Vec<PeerDescriptor>>>,
    config: CyclonConfig,
    signing_key: SecretKey,
    /// Distinct NodeIds we have successfully bootstrapped from.
    seed_origins: Arc<RwLock<HashSet<NodeId>>>,
    /// Used only for `bootstrap()` — establishes initial QUIC connections.
    transport: Arc<dyn Transport>,
    /// Used for gossip exchanges — bidirectional QUIC streams, never shared
    /// with the application-message receive channel.
    gossip: Arc<dyn GossipTransport>,
}

impl Cyclon {
    pub fn new(
        local_info: NodeInfo,
        key_seed: [u8; 32],
        transport: Arc<dyn Transport>,
        gossip: Arc<dyn GossipTransport>,
        config: CyclonConfig,
    ) -> Self {
        info!(
            view_size = config.view_size,
            shuffle_length = config.shuffle_length,
            verify_signatures = config.verify_signatures,
            min_seed_diversity = config.min_seed_diversity,
            "Cyclon initialised"
        );
        Self {
            local_info,
            view: Arc::new(RwLock::new(Vec::with_capacity(config.view_size))),
            signing_key: SecretKey::from(key_seed),
            seed_origins: Arc::new(RwLock::new(HashSet::new())),
            config,
            transport,
            gossip,
        }
    }

    /// Returns `true` once the view has been seeded from ≥ `min_seed_diversity`
    /// distinct bootstrap origins (SecureCyclon extension 2).
    pub async fn is_warm(&self) -> bool {
        self.seed_origins.read().await.len() >= self.config.min_seed_diversity
    }

    fn self_descriptor(&self) -> PeerDescriptor {
        let msg = PeerDescriptor::signing_bytes(&self.local_info);
        let sig = self.signing_key.sign(&msg);
        PeerDescriptor {
            node_info: self.local_info.clone(),
            age: 0,
            signature: sig.as_ref().to_vec(),
        }
    }

    fn pick_oldest(view: &[PeerDescriptor]) -> Option<(usize, PeerDescriptor)> {
        view.iter()
            .enumerate()
            .max_by_key(|(_, pd)| pd.age)
            .map(|(idx, pd)| (idx, pd.clone()))
    }

    fn build_shuffle_buffer(&self, view: &[PeerDescriptor]) -> Vec<PeerDescriptor> {
        let mut rng = rng();
        let mut buffer = vec![self.self_descriptor()];
        let take = self.config.shuffle_length.saturating_sub(1).min(view.len());
        let mut indices: Vec<usize> = (0..view.len()).collect();
        indices.shuffle(&mut rng);
        for &i in indices.iter().take(take) {
            buffer.push(view[i].clone());
        }
        buffer
    }

    fn merge_received(&self, view: &mut Vec<PeerDescriptor>, received: Vec<PeerDescriptor>) {
        let local_id = &self.local_info.node_id;
        let max_new = if self.config.max_new_per_merge == 0 {
            usize::MAX
        } else {
            self.config.max_new_per_merge
        };
        let mut new_count = 0usize;

        for incoming in received {
            if &incoming.node_info.node_id == local_id {
                continue;
            }

            // SecureCyclon extension 1: signature verification.
            if self.config.verify_signatures {
                match incoming.verify_signature() {
                    None => {
                        warn!(peer = %incoming.node_info.node_id, "dropping unsigned descriptor");
                        continue;
                    }
                    Some(false) => {
                        warn!(peer = %incoming.node_info.node_id, "dropping descriptor with invalid signature");
                        continue;
                    }
                    Some(true) => {}
                }
            }

            // Already in view — refresh if the incoming entry is younger.
            if let Some(existing) = view
                .iter_mut()
                .find(|pd| pd.node_info.node_id == incoming.node_info.node_id)
            {
                if incoming.age < existing.age {
                    *existing = incoming;
                }
                continue;
            }

            // SecureCyclon extension 3: rate-limit new peer insertions.
            if new_count >= max_new {
                continue;
            }

            if view.len() < self.config.view_size {
                view.push(incoming);
                new_count += 1;
            } else if let Some((oldest_idx, oldest)) = Self::pick_oldest(view) {
                if incoming.age < oldest.age {
                    view[oldest_idx] = incoming;
                    new_count += 1;
                }
            }
        }
    }

    fn increment_ages(view: &mut [PeerDescriptor]) {
        for pd in view.iter_mut() {
            pd.age = pd.age.saturating_add(1);
        }
    }

    /// Respond to inbound gossip requests indefinitely.
    ///
    /// Spawn this in a dedicated task alongside the periodic `cycle()` loop.
    /// For each inbound request: decode the peer's shuffle buffer, merge it
    /// into our view, build our own shuffle buffer, and reply.
    pub async fn serve_gossip(&self) {
        loop {
            match self.gossip.next_inbound_gossip(GOSSIP_CYCLON).await {
                Ok((from, request_bytes, resp_tx)) => {
                    let received: Vec<PeerDescriptor> =
                        match serde_json::from_slice(&request_bytes) {
                            Ok(v) => v,
                            Err(e) => {
                                warn!(from = %from, "Failed to decode inbound gossip request: {e}");
                                continue;
                            }
                        };

                    // Merge and build response under a single lock acquisition.
                    let response_buf = {
                        let mut view = self.view.write().await;
                        self.merge_received(&mut view, received);
                        self.build_shuffle_buffer(&view)
                    };

                    match serde_json::to_vec(&response_buf) {
                        Ok(encoded) => {
                            // Sending on the oneshot writes the response back
                            // through the bidirectional QUIC stream.
                            let _ = resp_tx.send(encoded);
                        }
                        Err(e) => warn!("Failed to encode gossip response: {e}"),
                    }
                }
                Err(e) => {
                    warn!("Gossip serve error: {e}");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }
}

#[async_trait]
impl PeerSampler for Cyclon {
    async fn sample(&self, count: usize) -> Vec<PeerDescriptor> {
        let view = self.view.read().await;
        let mut rng = rng();
        if count >= view.len() {
            debug!(requested = count, in_view = view.len(), "sample: returning full view");
            return view.clone();
        }
        let mut indices: Vec<usize> = (0..view.len()).collect();
        indices.shuffle(&mut rng);
        indices.into_iter().take(count).map(|i| view[i].clone()).collect()
    }

    async fn view(&self) -> Vec<PeerDescriptor> {
        self.view.read().await.clone()
    }

    /// One Cyclon gossip cycle.
    ///
    /// Sends a shuffle buffer to the oldest peer via a **bidirectional** QUIC
    /// stream and reads the response from that same stream — no shared channel,
    /// no risk of stealing application messages.
    async fn cycle(&self) -> Result<(), PubSubError> {
        let (target_peer, shuffle_out) = {
            let mut view = self.view.write().await;
            Self::increment_ages(&mut view);
            if view.is_empty() {
                debug!("cycle: view is empty, skipping");
                return Ok(());
            }
            let (oldest_idx, target) = Self::pick_oldest(&view).expect("non-empty");
            debug!(
                target = %target.node_info.node_id,
                target_age = target.age,
                "cycle: selected oldest peer"
            );
            let removed = view.remove(oldest_idx);
            let buffer = self.build_shuffle_buffer(&view);
            view.push(removed);
            (target, buffer)
        };

        let encoded = serde_json::to_vec(&shuffle_out)
            .map_err(|e| PubSubError::Codec(format!("failed to encode shuffle: {e}")))?;

        // Single bidirectional round-trip — response arrives on the same stream.
        let response_bytes = self
            .gossip
            .gossip_exchange(&target_peer.node_info.node_id, target_peer.node_info.addr, GOSSIP_CYCLON, encoded)
            .await?;

        let received: Vec<PeerDescriptor> = serde_json::from_slice(&response_bytes)
            .map_err(|e| PubSubError::Codec(format!("failed to decode shuffle response: {e}")))?;

        debug!(received_count = received.len(), "cycle: received shuffle response");

        {
            let mut view = self.view.write().await;
            view.retain(|pd| pd.node_info.node_id != target_peer.node_info.node_id);
            self.merge_received(&mut view, received);
        }

        let view_size = self.view.read().await.len();
        info!(view_size, "cycle complete");
        Ok(())
    }

    async fn bootstrap(&self, peers: Vec<NodeInfo>) -> Result<(), PubSubError> {
        let local_id = self.local_info.node_id.clone();
        let mut connected = 0usize;

        for info in peers {
            if info.node_id == local_id {
                continue;
            }
            {
                let view = self.view.read().await;
                if view.len() >= self.config.view_size {
                    break;
                }
                if view.iter().any(|pd| pd.node_info.node_id == info.node_id) {
                    continue;
                }
            }
            if let Err(e) = self.transport.connect(&info).await {
                warn!(addr = %info.addr, error = %e, "bootstrap: failed to connect, skipping");
                continue;
            }
            // SecureCyclon extension 2: track distinct seed origins.
            self.seed_origins.write().await.insert(info.node_id.clone());
            let mut view = self.view.write().await;
            view.push(PeerDescriptor::unsigned(info, 0));
            connected += 1;
        }

        let origins = self.seed_origins.read().await.len();
        let warm = origins >= self.config.min_seed_diversity;
        info!(bootstrapped = connected, seed_origins = origins, warm, "bootstrap complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pallas_crypto::key::ed25519::SecretKey;
    use pubsub_types::error::PubSubError;
    use pubsub_types::node::{node_id_from_key, NodeInfo, PeerDescriptor};
    use pubsub_types::traits::{GossipTransport, Transport};

    // ---- minimal stubs --------------------------------------------------------

    struct NoopTransport;
    struct NoopGossip;

    #[async_trait::async_trait]
    impl Transport for NoopTransport {
        async fn connect(&self, _: &NodeInfo) -> Result<(), PubSubError> { Ok(()) }
        async fn send(&self, _: &NodeId, _: &[u8]) -> Result<(), PubSubError> { Ok(()) }
        async fn recv(&self) -> Result<(NodeId, Vec<u8>), PubSubError> {
            std::future::pending().await
        }
        async fn disconnect(&self, _: &NodeId) -> Result<(), PubSubError> { Ok(()) }
        async fn connected_peers(&self) -> Vec<NodeId> { vec![] }
    }

    #[async_trait::async_trait]
    impl GossipTransport for NoopGossip {
        async fn gossip_exchange(
            &self, _: &NodeId, _: std::net::SocketAddr, _: u8, _: Vec<u8>,
        ) -> Result<Vec<u8>, PubSubError> {
            Ok(vec![])
        }
        async fn next_inbound_gossip(
            &self, _: u8,
        ) -> Result<pubsub_types::traits::InboundGossip, PubSubError> {
            std::future::pending().await
        }
    }

    // ---- helpers --------------------------------------------------------------

    fn make_key(seed: u8) -> SecretKey { SecretKey::from([seed; 32]) }

    fn make_node_info(key: &SecretKey) -> NodeInfo {
        let pk = key.public_key();
        let pk_bytes = pk.as_ref().to_vec();
        NodeInfo {
            node_id: node_id_from_key(&pk_bytes),
            addr: "127.0.0.1:9000".parse().unwrap(),
            public_key: pk_bytes,
            subscribed_topics: vec![],
        }
    }

    fn signed_descriptor(key: &SecretKey, info: NodeInfo) -> PeerDescriptor {
        let msg = PeerDescriptor::signing_bytes(&info);
        let sig = key.sign(&msg);
        PeerDescriptor { node_info: info, age: 0, signature: sig.as_ref().to_vec() }
    }

    fn make_cyclon(key_seed: u8, cfg: CyclonConfig) -> Cyclon {
        let key = make_key(key_seed);
        let info = make_node_info(&key);
        Cyclon {
            local_info: info,
            view: Arc::new(RwLock::new(Vec::new())),
            signing_key: make_key(key_seed),
            seed_origins: Arc::new(RwLock::new(HashSet::new())),
            transport: Arc::new(NoopTransport),
            gossip: Arc::new(NoopGossip),
            config: cfg,
        }
    }

    // ---- tests ----------------------------------------------------------------

    #[test]
    fn self_descriptor_signature_is_valid() {
        let cyclon = make_cyclon(0x01, CyclonConfig::default());
        let desc = cyclon.self_descriptor();
        assert_eq!(desc.verify_signature(), Some(true));
    }

    #[test]
    fn merge_drops_unsigned_when_verify_enabled() {
        let cyclon = make_cyclon(0x01, CyclonConfig { verify_signatures: true, ..CyclonConfig::default() });
        let unsigned = PeerDescriptor::unsigned(make_node_info(&make_key(0x02)), 0);
        let mut view = vec![];
        cyclon.merge_received(&mut view, vec![unsigned]);
        assert!(view.is_empty(), "unsigned descriptor should be rejected");
    }

    #[test]
    fn merge_accepts_signed_descriptor() {
        let cyclon = make_cyclon(0x01, CyclonConfig { verify_signatures: true, ..CyclonConfig::default() });
        let peer_key = make_key(0x02);
        let signed = signed_descriptor(&peer_key, make_node_info(&peer_key));
        let mut view = vec![];
        cyclon.merge_received(&mut view, vec![signed]);
        assert_eq!(view.len(), 1, "valid signed descriptor should be accepted");
    }

    #[test]
    fn merge_drops_wrong_signature() {
        let cyclon = make_cyclon(0x01, CyclonConfig { verify_signatures: true, ..CyclonConfig::default() });
        let peer_key = make_key(0x02);
        let peer_info = make_node_info(&peer_key);
        let tampered = signed_descriptor(&make_key(0x99), peer_info);
        let mut view = vec![];
        cyclon.merge_received(&mut view, vec![tampered]);
        assert!(view.is_empty(), "descriptor signed by wrong key should be rejected");
    }

    #[test]
    fn merge_rate_limits_new_insertions() {
        let cfg = CyclonConfig { verify_signatures: false, max_new_per_merge: 2, ..CyclonConfig::default() };
        let cyclon = make_cyclon(0x01, cfg);
        let peers: Vec<PeerDescriptor> = (2u8..8)
            .map(|s| PeerDescriptor::unsigned(make_node_info(&make_key(s)), 0))
            .collect();
        let mut view = vec![];
        cyclon.merge_received(&mut view, peers);
        assert_eq!(view.len(), 2, "only max_new_per_merge peers should be inserted");
    }

    #[test]
    fn merge_accepts_unsigned_when_verify_disabled() {
        let cfg = CyclonConfig { verify_signatures: false, ..CyclonConfig::default() };
        let cyclon = make_cyclon(0x01, cfg);
        let peers: Vec<PeerDescriptor> = (2u8..5)
            .map(|s| PeerDescriptor::unsigned(make_node_info(&make_key(s)), 0))
            .collect();
        let mut view = vec![];
        cyclon.merge_received(&mut view, peers);
        assert_eq!(view.len(), 3, "unsigned peers accepted when verification disabled");
    }
}
