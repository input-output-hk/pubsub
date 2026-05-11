//! Deterministic in-memory simulator for the vanilla Cyclon protocol.
//!
//! N Cyclon nodes share an [`Arc<Network>`]. A [`MockTransport`] looks up
//! the destination node via the network and calls its
//! [`Cyclon::handle_inbound`](secure_cyclon::Cyclon::handle_inbound) directly,
//! so an integration test runs in one process with a single [`ManualClock`]
//! and a seeded RNG — no real time, no real sockets.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use async_trait::async_trait;
use rand::seq::SliceRandom;
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use tokio::sync::Mutex;

use secure_cyclon::{
    Cyclon, CyclonConfig, Descriptor, GossipRequest, GossipResponse, ManualClock, NodeId,
    Result as CyclonResult, SecureCyclonError, StaticSeeds, Transport,
};

/// Cyclon configured for the simulator. Concrete generics — no `dyn`.
pub type SimCyclon = Cyclon<MockTransport, StaticSeeds, ManualClock>;

/// Shared address book + drop policy used by every node's [`MockTransport`].
pub struct Network {
    routes: Mutex<HashMap<NodeId, Arc<Mutex<SimCyclon>>>>,
    dropped: StdMutex<HashSet<NodeId>>,
}

impl Network {
    pub fn new() -> Self {
        Self {
            routes: Mutex::new(HashMap::new()),
            dropped: StdMutex::new(HashSet::new()),
        }
    }

    async fn register(&self, id: NodeId, cyclon: Arc<Mutex<SimCyclon>>) {
        self.routes.lock().await.insert(id, cyclon);
    }

    /// Make exchanges *to* `id` hang until the cycle's exchange timeout
    /// fires. Models a peer that has gone away or refuses to respond.
    pub fn drop_peer(&self, id: NodeId) {
        self.dropped.lock().unwrap().insert(id);
    }

    fn is_dropped(&self, id: &NodeId) -> bool {
        self.dropped.lock().unwrap().contains(id)
    }
}

impl Default for Network {
    fn default() -> Self {
        Self::new()
    }
}

/// In-memory [`Transport`] implementation. Routes via [`Network`]; a peer
/// listed in `Network::drop_peer` produces a hang that the Cyclon exchange
/// timeout converts into a recovered cycle.
#[derive(Clone)]
pub struct MockTransport {
    network: Arc<Network>,
}

impl MockTransport {
    pub fn new(network: Arc<Network>) -> Self {
        Self { network }
    }
}

#[async_trait]
impl Transport for MockTransport {
    async fn exchange(
        &self,
        peer: &NodeId,
        _addr: SocketAddr,
        request: GossipRequest,
    ) -> CyclonResult<GossipResponse> {
        if self.network.is_dropped(peer) {
            std::future::pending::<()>().await;
            unreachable!();
        }
        let route = {
            let routes = self.network.routes.lock().await;
            routes.get(peer).cloned()
        };
        let route =
            route.ok_or_else(|| SecureCyclonError::Transport(format!("unknown peer {peer:?}")))?;
        let mut guard = route.lock().await;
        guard.handle_inbound(request).await
    }
}

pub struct SimBuilder {
    n: usize,
    view_len: usize,
    swap_len: usize,
    seed: u64,
    exchange_timeout_ms: u64,
    gossip_period_ms: u64,
    seeds_per_node: Option<usize>,
}

impl SimBuilder {
    pub fn new(n: usize) -> Self {
        Self {
            n,
            view_len: 10,
            swap_len: 3,
            seed: 1,
            exchange_timeout_ms: 50,
            gossip_period_ms: 10_000,
            seeds_per_node: None,
        }
    }

    pub fn view_len(mut self, v: usize) -> Self {
        self.view_len = v;
        self
    }

    pub fn swap_len(mut self, s: usize) -> Self {
        self.swap_len = s;
        self
    }

    pub fn seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    pub fn exchange_timeout_ms(mut self, ms: u64) -> Self {
        self.exchange_timeout_ms = ms;
        self
    }

    pub fn gossip_period_ms(mut self, ms: u64) -> Self {
        self.gossip_period_ms = ms;
        self
    }

    pub fn seeds_per_node(mut self, k: usize) -> Self {
        self.seeds_per_node = Some(k);
        self
    }

    pub async fn build(self) -> Simulator {
        let network = Arc::new(Network::new());
        let clock = ManualClock::new(1_000);
        let config = CyclonConfig {
            view_len: self.view_len,
            swap_len: self.swap_len,
            gossip_period_ms: self.gossip_period_ms,
            exchange_timeout_ms: self.exchange_timeout_ms,
        };

        let seeds_per_node = self
            .seeds_per_node
            .unwrap_or_else(|| self.view_len.min(self.n.saturating_sub(1)));

        let mut id_rng = ChaCha8Rng::seed_from_u64(self.seed);
        let mut node_ids = Vec::with_capacity(self.n);
        let mut addrs = Vec::with_capacity(self.n);
        let mut seed_descriptors = Vec::with_capacity(self.n);
        for i in 0..self.n {
            let mut id_bytes = [0u8; 32];
            id_rng.fill(&mut id_bytes);
            let id = NodeId::from_bytes(id_bytes);
            let addr: SocketAddr = format!("127.0.0.1:{}", 10_000 + i as u16).parse().unwrap();
            let desc = Descriptor::fresh(id, addr, 0);
            node_ids.push(id);
            addrs.push(addr);
            seed_descriptors.push(desc);
        }

        let mut rng = ChaCha8Rng::seed_from_u64(self.seed.wrapping_add(0xC0FFEE));
        let mut nodes = Vec::with_capacity(self.n);
        for i in 0..self.n {
            let mut others: Vec<usize> = (0..self.n).filter(|&j| j != i).collect();
            others.shuffle(&mut rng);
            let seeds: Vec<Descriptor> = others
                .iter()
                .take(seeds_per_node)
                .map(|&j| seed_descriptors[j].clone())
                .collect();
            let transport = MockTransport::new(Arc::clone(&network));
            let cyclon = SimCyclon::with_seed(
                config.clone(),
                transport,
                StaticSeeds::new(seeds),
                clock.clone(),
                node_ids[i],
                addrs[i],
                self.seed.wrapping_add((i as u64).wrapping_mul(1_234_567)),
            );
            let arc = Arc::new(Mutex::new(cyclon));
            network.register(node_ids[i], Arc::clone(&arc)).await;
            nodes.push(arc);
        }

        Simulator {
            nodes,
            node_ids,
            network,
            clock,
            config,
        }
    }
}

/// Deterministic harness running N Cyclon nodes in lockstep.
pub struct Simulator {
    nodes: Vec<Arc<Mutex<SimCyclon>>>,
    pub node_ids: Vec<NodeId>,
    pub network: Arc<Network>,
    pub clock: ManualClock,
    pub config: CyclonConfig,
}

impl Simulator {
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub async fn bootstrap_all(&self) -> CyclonResult<()> {
        for node in &self.nodes {
            let mut g = node.lock().await;
            g.bootstrap().await?;
        }
        Ok(())
    }

    /// Advance the clock by one gossip period and let every node initiate
    /// one gossip exchange in insertion order.
    pub async fn tick(&self) {
        self.clock.advance(self.config.gossip_period_ms);
        for node in &self.nodes {
            let mut g = node.lock().await;
            g.cycle().await;
        }
    }

    pub async fn ticks(&self, n: usize) {
        for _ in 0..n {
            self.tick().await;
        }
    }

    pub async fn view_size(&self, i: usize) -> usize {
        self.nodes[i].lock().await.view().len()
    }

    pub async fn view_ids(&self, i: usize) -> Vec<NodeId> {
        self.nodes[i].lock().await.view().node_ids()
    }

    /// In-degree per node: how many *other* views contain that node id.
    pub async fn indegree_distribution(&self) -> HashMap<NodeId, usize> {
        let mut counts: HashMap<NodeId, usize> = HashMap::new();
        for id in &self.node_ids {
            counts.insert(*id, 0);
        }
        for node in &self.nodes {
            let g = node.lock().await;
            for id in g.view().node_ids() {
                *counts.entry(id).or_insert(0) += 1;
            }
        }
        counts
    }
}
