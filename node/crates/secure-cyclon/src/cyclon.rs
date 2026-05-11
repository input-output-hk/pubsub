use std::net::SocketAddr;
use std::time::Duration;

use rand::rngs::StdRng;
use rand::SeedableRng;
use tokio::time::timeout;
use tracing::{debug, warn};

use crate::bootstrap::BootstrapSource;
use crate::clock::Clock;
use crate::config::CyclonConfig;
use crate::descriptor::{Descriptor, NodeId};
use crate::error::{Result, SecureCyclonError};
use crate::protocol::{GossipRequest, GossipResponse};
use crate::transport::Transport;
use crate::view::View;

/// Cyclon peer-sampling node (paper §II.B + Fig. 1).
///
/// Holds a bounded peer view and periodically initiates push-pull gossip
/// exchanges to refresh it with random descriptors from the rest of the
/// network. Generic over the transport, bootstrap source, and clock so the
/// same algorithm runs against an in-memory simulator and a real network
/// without changes.
pub struct Cyclon<T: Transport, B: BootstrapSource, C: Clock> {
    config: CyclonConfig,
    transport: T,
    bootstrap: B,
    clock: C,
    self_id: NodeId,
    self_addr: SocketAddr,
    view: View,
    rng: StdRng,
}

impl<T: Transport, B: BootstrapSource, C: Clock> Cyclon<T, B, C> {
    pub fn new(
        config: CyclonConfig,
        transport: T,
        bootstrap: B,
        clock: C,
        self_id: NodeId,
        self_addr: SocketAddr,
    ) -> Self {
        Self::with_seed(
            config,
            transport,
            bootstrap,
            clock,
            self_id,
            self_addr,
            rand::random(),
        )
    }

    pub fn with_seed(
        config: CyclonConfig,
        transport: T,
        bootstrap: B,
        clock: C,
        self_id: NodeId,
        self_addr: SocketAddr,
        seed: u64,
    ) -> Self {
        let view = View::new(config.view_len, self_id);
        Self {
            config,
            transport,
            bootstrap,
            clock,
            self_id,
            self_addr,
            view,
            rng: StdRng::seed_from_u64(seed),
        }
    }

    pub fn self_id(&self) -> NodeId {
        self.self_id
    }

    pub fn self_addr(&self) -> SocketAddr {
        self.self_addr
    }

    pub fn view(&self) -> &View {
        &self.view
    }

    pub fn config(&self) -> &CyclonConfig {
        &self.config
    }

    /// Seeds the initial view from the configured [`BootstrapSource`].
    ///
    /// Idempotent: re-bootstrapping after the view has filled is harmless
    /// because [`View::insert`] dedupes and respects capacity.
    pub async fn bootstrap(&mut self) -> Result<()> {
        let seeds = self.bootstrap.seeds().await?;
        if seeds.is_empty() {
            return Err(SecureCyclonError::EmptyBootstrap);
        }
        for d in seeds {
            self.view.insert(d);
        }
        Ok(())
    }

    /// Initiates one gossip exchange.
    ///
    /// Picks the oldest peer in the view as the partner, swaps `swap_len`
    /// descriptors with them, and merges the response. If the exchange
    /// fails or times out, the shipped descriptors are re-inserted so view
    /// slots are not permanently lost; the unresponsive partner is dropped.
    pub async fn cycle(&mut self) {
        if self.view.is_empty() {
            debug!("cycle skipped: view empty");
            return;
        }

        let now = self.clock.now_ms();

        let partner = self
            .view
            .take_oldest()
            .expect("view non-empty by check above");
        let partner_id = partner.node;
        let partner_addr = partner.addr;

        let mut peers_to_send = self.view.take_random_excluding(
            self.config.swap_len.saturating_sub(1),
            &partner_id,
            &mut self.rng,
        );

        let self_descriptor = Descriptor::fresh(self.self_id, self.self_addr, now);
        peers_to_send.push(self_descriptor.clone());

        // Snapshot so the shipped descriptors can be re-inserted if the
        // exchange does not complete.
        let shipped = peers_to_send.clone();

        let request = GossipRequest {
            sender: self_descriptor,
            peers: peers_to_send,
        };

        let exchange = self.transport.exchange(&partner_id, partner_addr, request);
        let outcome = timeout(
            Duration::from_millis(self.config.exchange_timeout_ms),
            exchange,
        )
        .await;

        let response = match outcome {
            Ok(Ok(resp)) => resp,
            Ok(Err(e)) => {
                warn!(?partner_id, error=%e, "gossip exchange failed; recovering shipped");
                self.reinsert_shipped(shipped);
                return;
            }
            Err(_) => {
                warn!(?partner_id, "gossip exchange timed out; recovering shipped");
                self.reinsert_shipped(shipped);
                return;
            }
        };

        if response.peers.len() > self.config.swap_len {
            warn!(
                received = response.peers.len(),
                expected_max = self.config.swap_len,
                ?partner_id,
                "oversized response peer list; dropping payload, recovering shipped"
            );
            self.reinsert_shipped(shipped);
            return;
        }

        for d in response.peers {
            self.view.insert(d);
        }
        // Fill remaining empty slots with shipped descriptors (paper §V.A.1).
        // `View::insert` excludes self and dedupes, so an entry already
        // delivered by the response is a no-op here.
        for d in shipped {
            self.view.insert(d);
        }
    }

    /// Handles an inbound gossip request and produces the response.
    ///
    /// Rejects requests carrying more than `swap_len` peers to bound how
    /// many descriptors a single sender can inject per exchange.
    pub async fn handle_inbound(&mut self, request: GossipRequest) -> Result<GossipResponse> {
        if request.peers.len() > self.config.swap_len {
            warn!(
                received = request.peers.len(),
                expected_max = self.config.swap_len,
                sender = ?request.sender.node,
                "oversized request peer list; dropping payload"
            );
            return Err(SecureCyclonError::InvalidMessage(format!(
                "request peers.len() = {} > swap_len = {}",
                request.peers.len(),
                self.config.swap_len
            )));
        }

        let initiator_id = request.sender.node;

        let peers_to_send =
            self.view
                .take_random_excluding(self.config.swap_len, &initiator_id, &mut self.rng);
        let shipped = peers_to_send.clone();

        self.view.insert(request.sender);
        for d in request.peers {
            self.view.insert(d);
        }
        for d in shipped {
            self.view.insert(d);
        }

        Ok(GossipResponse {
            sender_id: self.self_id,
            peers: peers_to_send,
        })
    }

    fn reinsert_shipped(&mut self, shipped: Vec<Descriptor>) {
        for d in shipped {
            self.view.insert(d);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;

    use super::*;
    use crate::bootstrap::StaticSeeds;
    use crate::clock::ManualClock;

    fn addr(port: u16) -> SocketAddr {
        format!("127.0.0.1:{port}").parse().unwrap()
    }

    fn node_id(byte: u8) -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = byte;
        NodeId::from_bytes(bytes)
    }

    fn descriptor(seed: u8, port: u16, created_at: u64) -> Descriptor {
        Descriptor::fresh(node_id(seed), addr(port), created_at)
    }

    struct FailTransport;

    #[async_trait]
    impl Transport for FailTransport {
        async fn exchange(
            &self,
            _: &NodeId,
            _: SocketAddr,
            _: GossipRequest,
        ) -> Result<GossipResponse> {
            Err(SecureCyclonError::Transport("forced failure".to_string()))
        }
    }

    struct HangTransport;

    #[async_trait]
    impl Transport for HangTransport {
        async fn exchange(
            &self,
            _: &NodeId,
            _: SocketAddr,
            _: GossipRequest,
        ) -> Result<GossipResponse> {
            std::future::pending().await
        }
    }

    struct RecordingTransport {
        log: Arc<Mutex<Vec<GossipRequest>>>,
        canned: GossipResponse,
    }

    impl RecordingTransport {
        fn new(canned: GossipResponse) -> (Self, Arc<Mutex<Vec<GossipRequest>>>) {
            let log = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    log: Arc::clone(&log),
                    canned,
                },
                log,
            )
        }
    }

    #[async_trait]
    impl Transport for RecordingTransport {
        async fn exchange(
            &self,
            _: &NodeId,
            _: SocketAddr,
            request: GossipRequest,
        ) -> Result<GossipResponse> {
            self.log.lock().unwrap().push(request);
            Ok(self.canned.clone())
        }
    }

    fn make_cyclon<T: Transport>(
        transport: T,
        clock: ManualClock,
        seeds: Vec<Descriptor>,
    ) -> Cyclon<T, StaticSeeds, ManualClock> {
        let cfg = CyclonConfig {
            view_len: 5,
            swap_len: 3,
            exchange_timeout_ms: 50,
            ..CyclonConfig::default()
        };
        Cyclon::with_seed(
            cfg,
            transport,
            StaticSeeds::new(seeds),
            clock,
            node_id(0),
            addr(8000),
            1,
        )
    }

    #[tokio::test]
    async fn bootstrap_seeds_initial_view() {
        let seeds = vec![descriptor(1, 9001, 10), descriptor(2, 9002, 20)];
        let mut cyclon = make_cyclon(FailTransport, ManualClock::new(100), seeds);
        cyclon.bootstrap().await.unwrap();
        assert_eq!(cyclon.view().len(), 2);
    }

    #[tokio::test]
    async fn bootstrap_returns_error_on_empty_seeds() {
        let mut cyclon = make_cyclon(FailTransport, ManualClock::new(0), vec![]);
        let err = cyclon.bootstrap().await.unwrap_err();
        assert!(matches!(err, SecureCyclonError::EmptyBootstrap));
    }

    #[tokio::test(start_paused = true)]
    async fn cycle_with_failed_exchange_recovers_shipped() {
        let seeds = vec![
            descriptor(1, 9001, 10),
            descriptor(2, 9002, 20),
            descriptor(3, 9003, 30),
            descriptor(4, 9004, 40),
        ];
        let mut cyclon = make_cyclon(FailTransport, ManualClock::new(100), seeds);
        cyclon.bootstrap().await.unwrap();
        assert_eq!(cyclon.view().len(), 4);

        cyclon.cycle().await;

        // The oldest peer (chosen as partner) is dropped on failure; the
        // other shipped peers are recovered.
        assert_eq!(cyclon.view().len(), 3);
        assert!(!cyclon.view().node_ids().contains(&node_id(1)));
    }

    #[tokio::test(start_paused = true)]
    async fn cycle_with_timeout_recovers_shipped() {
        let seeds = vec![
            descriptor(1, 9001, 10),
            descriptor(2, 9002, 20),
            descriptor(3, 9003, 30),
        ];
        let mut cyclon = make_cyclon(HangTransport, ManualClock::new(100), seeds);
        cyclon.bootstrap().await.unwrap();
        let before = cyclon.view().len();
        cyclon.cycle().await;
        assert_eq!(cyclon.view().len(), before - 1);
    }

    #[tokio::test]
    async fn cycle_rejects_oversize_response() {
        let oversized = GossipResponse {
            sender_id: node_id(9),
            peers: (10..20)
                .map(|i| descriptor(i, 9000 + i as u16, 100))
                .collect(),
        };
        let (transport, _log) = RecordingTransport::new(oversized);
        let seeds = vec![
            descriptor(1, 9001, 10),
            descriptor(2, 9002, 20),
            descriptor(3, 9003, 30),
            descriptor(4, 9004, 40),
        ];
        let mut cyclon = make_cyclon(transport, ManualClock::new(100), seeds);
        cyclon.bootstrap().await.unwrap();
        cyclon.cycle().await;
        for id_byte in 10..20u8 {
            assert!(!cyclon.view().contains(&node_id(id_byte)));
        }
    }

    #[tokio::test]
    async fn handle_inbound_rejects_oversize_request() {
        let mut cyclon = make_cyclon(FailTransport, ManualClock::new(100), vec![]);
        let oversized_peers: Vec<_> = (10..20)
            .map(|i| descriptor(i, 9000 + i as u16, 50))
            .collect();
        let sender = descriptor(7, 9007, 60);
        let req = GossipRequest {
            sender,
            peers: oversized_peers,
        };
        let err = cyclon.handle_inbound(req).await.unwrap_err();
        assert!(matches!(err, SecureCyclonError::InvalidMessage(_)));
        assert!(cyclon.view().is_empty());
    }

    #[tokio::test]
    async fn cycle_drops_self_when_present_in_response() {
        // A malicious gossip partner echoes the initiator's own descriptor
        // back in the response. The initiator must not accept itself into
        // its own view.
        let self_id = node_id(0);
        let canned = GossipResponse {
            sender_id: node_id(9),
            peers: vec![
                descriptor(0, 8000, 500), // forged: initiator's own id
                descriptor(99, 9099, 500),
            ],
        };
        let (transport, _log) = RecordingTransport::new(canned);
        let seeds = vec![
            descriptor(1, 9001, 10),
            descriptor(2, 9002, 20),
            descriptor(3, 9003, 30),
        ];
        let mut cyclon = make_cyclon(transport, ManualClock::new(100), seeds);
        cyclon.bootstrap().await.unwrap();
        cyclon.cycle().await;
        assert!(!cyclon.view().contains(&self_id));
    }

    #[tokio::test]
    async fn cycle_dedupes_duplicate_entries_in_response() {
        // A malicious gossip partner ships several descriptors for the same
        // peer id. After the cycle the view must contain at most one entry
        // for that peer.
        let duplicate = descriptor(42, 9042, 500);
        let canned = GossipResponse {
            sender_id: node_id(9),
            peers: vec![duplicate.clone(), duplicate.clone(), duplicate.clone()],
        };
        let (transport, _log) = RecordingTransport::new(canned);
        let seeds = vec![descriptor(1, 9001, 10), descriptor(2, 9002, 20)];
        let mut cyclon = make_cyclon(transport, ManualClock::new(100), seeds);
        cyclon.bootstrap().await.unwrap();
        cyclon.cycle().await;
        let count = cyclon
            .view()
            .node_ids()
            .iter()
            .filter(|id| **id == node_id(42))
            .count();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn handle_inbound_drops_self_in_request_peers() {
        // A malicious initiator includes the responder's own descriptor in
        // the request peer list. The responder must not pick up itself.
        let self_id = node_id(0);
        let mut cyclon = make_cyclon(FailTransport, ManualClock::new(100), vec![]);
        let req = GossipRequest {
            sender: descriptor(7, 9007, 60),
            peers: vec![descriptor(0, 8000, 60)], // forged: responder's own id
        };
        let _ = cyclon.handle_inbound(req).await.unwrap();
        assert!(!cyclon.view().contains(&self_id));
    }

    #[tokio::test]
    async fn handle_inbound_dedupes_duplicate_entries_in_request_peers() {
        // A malicious initiator ships several descriptors for the same peer
        // id in one request. The responder's view ends up with at most one.
        let mut cyclon = make_cyclon(FailTransport, ManualClock::new(100), vec![]);
        let duplicate = descriptor(42, 9042, 60);
        let req = GossipRequest {
            sender: descriptor(7, 9007, 60),
            peers: vec![duplicate.clone(), duplicate.clone()],
        };
        let _ = cyclon.handle_inbound(req).await.unwrap();
        let count = cyclon
            .view()
            .node_ids()
            .iter()
            .filter(|id| **id == node_id(42))
            .count();
        assert_eq!(count, 1);
    }

    #[tokio::test(start_paused = true)]
    async fn self_descriptor_uses_current_clock_each_cycle() {
        let canned = GossipResponse {
            sender_id: node_id(9),
            peers: Vec::new(),
        };
        let (transport, log) = RecordingTransport::new(canned);
        let clock = ManualClock::new(1_000);
        let seeds = vec![descriptor(1, 9001, 10), descriptor(2, 9002, 20)];
        let mut cyclon = make_cyclon(transport, clock.clone(), seeds);
        cyclon.bootstrap().await.unwrap();

        cyclon.cycle().await;
        clock.advance(5_000);
        cyclon.bootstrap().await.unwrap();
        clock.advance(5_000);
        cyclon.cycle().await;

        let log = log.lock().unwrap();
        assert!(log.len() >= 2);
        let ts: Vec<u64> = log.iter().map(|r| r.sender.created_at).collect();
        assert!(
            ts[0] < ts[ts.len() - 1],
            "timestamps not increasing: {ts:?}"
        );
    }
}
