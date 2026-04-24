// =============================================================================
// Cyclon — Gossip-based peer sampling service
// =============================================================================
//
// Implements the Cyclon protocol (Voulgaris, Gavidia & van Steen, 2005).
//
// Each gossip cycle opens a QUIC **bidirectional** stream to the oldest peer,
// sends a shuffle buffer, reads the response from the same stream, and closes
// it.  Gossip traffic is therefore completely separate from the unidirectional
// application-message channel — they can never steal each other's messages.
//
// Eclipse-resistance (future work — needed before production):
//   The original Jesi–Montresor–Babaoglu "SecureCyclon" extensions add:
//   1. Signed PeerDescriptors: each node signs its own descriptor with its
//      Ed25519 key so recipients can verify before inserting into their view.
//      Requires the on-chain Node Registry to be live (public key distribution).
//   2. Bootstrap diversity: require entries from ≥ 2 distinct seed nodes
//      before the view is considered warm.
//   3. Rate-limited peer replacement: cap new-peer insertions to ≤ 50% of
//      view_size per cycle to slow eclipse attacks.
//   These are deferred until the on-chain registry provides key infrastructure.
// =============================================================================

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rand::prelude::SliceRandom;
use rand::rng;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use pubsub_types::error::PubSubError;
use pubsub_types::node::{NodeInfo, PeerDescriptor};
use pubsub_types::traits::{GossipTransport, PeerSampler, Transport, GOSSIP_CYCLON};

#[derive(Debug, Clone)]
pub struct CyclonConfig {
    pub view_size: usize,
    pub shuffle_length: usize,
}

impl Default for CyclonConfig {
    fn default() -> Self {
        Self {
            view_size: 20,
            shuffle_length: 10,
        }
    }
}

pub struct Cyclon {
    local_info: NodeInfo,
    view: Arc<RwLock<Vec<PeerDescriptor>>>,
    config: CyclonConfig,
    /// Used only for `bootstrap()` — establishes initial QUIC connections.
    transport: Arc<dyn Transport>,
    /// Used for gossip exchanges — bidirectional QUIC streams, never shared
    /// with the application-message receive channel.
    gossip: Arc<dyn GossipTransport>,
}

impl Cyclon {
    pub fn new(
        local_info: NodeInfo,
        transport: Arc<dyn Transport>,
        gossip: Arc<dyn GossipTransport>,
        config: CyclonConfig,
    ) -> Self {
        info!(
            view_size = config.view_size,
            shuffle_length = config.shuffle_length,
            "Cyclon initialised"
        );
        Self {
            local_info,
            view: Arc::new(RwLock::new(Vec::with_capacity(config.view_size))),
            config,
            transport,
            gossip,
        }
    }

    fn self_descriptor(&self) -> PeerDescriptor {
        PeerDescriptor {
            node_info: self.local_info.clone(),
            age: 0,
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
        for incoming in received {
            if &incoming.node_info.node_id == local_id {
                continue;
            }
            if let Some(existing) = view
                .iter_mut()
                .find(|pd| pd.node_info.node_id == incoming.node_info.node_id)
            {
                if incoming.age < existing.age {
                    *existing = incoming;
                }
                continue;
            }
            if view.len() < self.config.view_size {
                view.push(incoming);
            } else if let Some((oldest_idx, oldest)) = Self::pick_oldest(view) {
                if incoming.age < oldest.age {
                    view[oldest_idx] = incoming;
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
            let mut view = self.view.write().await;
            view.push(PeerDescriptor { node_info: info, age: 0 });
            connected += 1;
        }

        info!(bootstrapped = connected, "bootstrap complete");
        Ok(())
    }
}
