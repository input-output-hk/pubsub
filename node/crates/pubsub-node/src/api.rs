use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, Sse};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt as _;
use tower_http::cors::CorsLayer;
use tracing::info;

use pubsub_types::message::{Message, TopicId};
use pubsub_types::node::{NodeId, NodeInfo};
use pubsub_types::traits::MessageStore;

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NodeEvent {
    PeerConnected {
        peer_id: String,
        peer_id_bech32: String,
        addr: String,
    },
    MessageReceived {
        from: String,
        topic: String,
        seq: u64,
        payload_preview: String,
    },
}

// ---------------------------------------------------------------------------
// Stored message (ring buffer per topic)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct StoredMessage {
    pub from: String,
    pub topic: String,
    pub seq: u64,
    pub payload_preview: String,
    pub received_at_ms: u64,
}

// ---------------------------------------------------------------------------
// Shared API state
// ---------------------------------------------------------------------------

/// Peer not refreshed in this many seconds is shown as stale (gray) in the dashboard.
const PEER_STALE_SECS: u64 = 15;
/// Peer not refreshed in this many seconds is removed from the topology entirely.
const PEER_REMOVE_SECS: u64 = 45;

pub struct ApiState {
    pub node_info: NodeInfo,
    pub started_at: Instant,
    /// "mainnet" | "preprod" | "preview"
    pub network: String,
    /// bech32 HRP for node identifiers ("psnode" or "psnode_test")
    pub bech32_hrp: String,
    /// peer_id_hex → (addr, last_seen). Peers not refreshed within PEER_REMOVE_SECS are pruned.
    pub connected_peers: Arc<DashMap<String, (String, Instant)>>,
    /// topic_hex → topic_name (all topics discovered from chain)
    pub topic_names: Arc<DashMap<String, String>>,
    /// topic_hex of topics this node is actively subscribed to (subset of topic_names)
    pub subscribed_topic_ids: Arc<DashMap<String, ()>>,
    /// Recent messages per topic (capped at 200 total)
    pub recent_messages: Arc<tokio::sync::RwLock<Vec<StoredMessage>>>,
    pub event_tx: broadcast::Sender<NodeEvent>,
    /// Live message broadcast — every validated, non-duplicate message lands here.
    /// Per-topic SSE subscribers filter by `topic_id` on the receive side.
    pub subscriber_tx: broadcast::Sender<Message>,
    /// Hot cache used to serve replay on subscribe.
    pub store: Arc<dyn MessageStore>,
}

impl ApiState {
    pub fn new(
        node_info: NodeInfo,
        network: String,
        bech32_hrp: String,
        subscriber_tx: broadcast::Sender<Message>,
        store: Arc<dyn MessageStore>,
    ) -> (Arc<Self>, broadcast::Sender<NodeEvent>) {
        let (tx, _) = broadcast::channel(1024);
        let state = Arc::new(Self {
            node_info,
            started_at: Instant::now(),
            network,
            bech32_hrp,
            connected_peers: Arc::new(DashMap::new()),
            topic_names: Arc::new(DashMap::new()),
            subscribed_topic_ids: Arc::new(DashMap::new()),
            recent_messages: Arc::new(tokio::sync::RwLock::new(Vec::new())),
            event_tx: tx.clone(),
            subscriber_tx,
            store,
        });
        (state, tx)
    }

    pub fn send_event(&self, evt: NodeEvent) {
        // ignore send errors (no active listeners is fine)
        let _ = self.event_tx.send(evt);
    }

    pub async fn record_message(&self, from: &NodeId, msg: &Message, topic_name: Option<&str>) {
        let topic_hex: String = msg.topic_id.0.iter().map(|b| format!("{b:02x}")).collect();
        if let Some(name) = topic_name {
            self.topic_names.insert(topic_hex.clone(), name.to_owned());
        }

        let stored = StoredMessage {
            from: from.0.iter().map(|b| format!("{b:02x}")).collect(),
            topic: topic_hex.clone(),
            seq: msg.sequence_nr,
            payload_preview: String::from_utf8_lossy(&msg.payload)
                .chars()
                .take(80)
                .collect(),
            received_at_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_millis() as u64,
        };

        let mut msgs = self.recent_messages.write().await;
        msgs.push(stored.clone());
        if msgs.len() > 200 {
            msgs.remove(0);
        }

        self.send_event(NodeEvent::MessageReceived {
            from: stored.from.clone(),
            topic: topic_hex,
            seq: msg.sequence_nr,
            payload_preview: stored.payload_preview,
        });
    }

    /// Record that a peer is reachable. Called on actual handshake AND on every
    /// Cyclon cycle as a heartbeat — only emit a `PeerConnected` SSE event the
    /// first time we see a peer so the dashboard does not rerender on each cycle.
    pub fn record_peer_connected(&self, peer_id: &NodeId, addr: &str) {
        let id = peer_id.0.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let was_new = self
            .connected_peers
            .insert(id.clone(), (addr.to_owned(), Instant::now()))
            .is_none();
        if was_new {
            let peer_id_bech32 = encode_bech32(&self.bech32_hrp, &peer_id.0);
            self.send_event(NodeEvent::PeerConnected {
                peer_id: id,
                peer_id_bech32,
                addr: addr.to_owned(),
            });
        }
    }

    pub fn evict_stale_peers(&self) {
        self.connected_peers
            .retain(|_, (_, ts)| ts.elapsed().as_secs() < PEER_REMOVE_SECS);
    }

}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct StatusResponse {
    node_id: String,
    node_id_bech32: String,
    network: String,
    addr: String,
    uptime_secs: u64,
    peer_count: usize,
    topic_count: usize,
    message_count: usize,
}

#[derive(Serialize)]
struct PeerEntry {
    peer_id: String,
    peer_id_bech32: String,
    addr: String,
}

#[derive(Serialize)]
struct TopicEntry {
    topic_id: String,
    name: Option<String>,
    /// true if this node is actively relaying this topic
    subscribed: bool,
}

#[derive(Deserialize)]
struct MessagesQuery {
    #[serde(default)]
    topic: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    20
}

#[derive(Serialize)]
struct TopologyResponse {
    self_id: String,
    peers: Vec<TopologyPeer>,
}

#[derive(Serialize)]
struct TopologyPeer {
    peer_id: String,
    peer_id_bech32: String,
    addr: String,
    /// True when the peer hasn't been seen in the Cyclon view for PEER_STALE_SECS.
    stale: bool,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn handle_root() -> impl IntoResponse {
    axum::response::Html(include_str!("../../../dashboard/index.html"))
}

async fn handle_status(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    let msgs = state.recent_messages.read().await;
    let node_id: String = state.node_info.node_id.0.iter().map(|b| format!("{b:02x}")).collect();
    let node_id_bech32 = encode_bech32(&state.bech32_hrp, &state.node_info.node_id.0);
    Json(StatusResponse {
        node_id,
        node_id_bech32,
        network: state.network.clone(),
        addr: state.node_info.addr.to_string(),
        uptime_secs: state.started_at.elapsed().as_secs(),
        peer_count: state.connected_peers.iter()
            .filter(|e| e.value().1.elapsed().as_secs() < PEER_STALE_SECS)
            .count(),
        topic_count: state.subscribed_topic_ids.len(),
        message_count: msgs.len(),
    })
}

fn encode_bech32(hrp: &str, data: &[u8]) -> String {
    const CHARSET: &[u8] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
    const GEN: [u32; 5] = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3];

    fn polymod(v: &[u8]) -> u32 {
        let mut c: u32 = 1;
        for &d in v {
            let t = c >> 25;
            c = ((c & 0x1ffffff) << 5) ^ u32::from(d);
            for (i, &g) in GEN.iter().enumerate() {
                if (t >> i) & 1 != 0 { c ^= g; }
            }
        }
        c
    }

    // HRP expand
    let mut enc: Vec<u8> = hrp.bytes().map(|b| b >> 5).collect();
    enc.push(0);
    hrp.bytes().for_each(|b| enc.push(b & 31));

    // Convert data 8→5 bits
    let (mut acc, mut bits) = (0u32, 0u32);
    for &b in data {
        acc = (acc << 8) | u32::from(b);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            enc.push(((acc >> bits) & 31) as u8);
        }
    }
    if bits > 0 { enc.push(((acc << (5 - bits)) & 31) as u8); }

    // Checksum
    let payload_end = enc.len();
    for _ in 0..6 { enc.push(0); }
    let pm = polymod(&enc) ^ 1;
    for i in 0..6 { enc[payload_end + i] = ((pm >> (5 * (5 - i))) & 31) as u8; }

    // Encode
    let mut out = format!("{hrp}1");
    for &v in &enc[hrp.len() + 1..] {
        out.push(CHARSET[v as usize] as char);
    }
    out
}

async fn handle_peers(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    let peers: Vec<PeerEntry> = state
        .connected_peers
        .iter()
        .map(|e| {
            let peer_id = e.key().clone();
            let peer_id_bech32 = encode_bech32(&state.bech32_hrp, &hex_to_bytes(&peer_id));
            PeerEntry { peer_id, peer_id_bech32, addr: e.value().0.clone() }
        })
        .collect();
    Json(peers)
}

fn hex_to_bytes(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .chunks(2)
        .filter_map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap_or(""), 16).ok())
        .collect()
}

async fn handle_topics(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    let mut topics: Vec<TopicEntry> = state
        .topic_names
        .iter()
        .map(|e| {
            let id = e.key().clone();
            let subscribed = state.subscribed_topic_ids.contains_key(&id);
            TopicEntry { topic_id: id, name: Some(e.value().clone()), subscribed }
        })
        .collect();
    topics.sort_by(|a, b| a.name.cmp(&b.name));
    Json(topics)
}

async fn handle_messages(
    State(state): State<Arc<ApiState>>,
    Query(q): Query<MessagesQuery>,
) -> impl IntoResponse {
    let msgs = state.recent_messages.read().await;
    let filtered: Vec<StoredMessage> = msgs
        .iter()
        .rev()
        .filter(|m| q.topic.as_deref().map_or(true, |t| m.topic.starts_with(t)))
        .take(q.limit)
        .cloned()
        .collect();
    Json(filtered)
}

async fn handle_topology(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    let self_id: String = state
        .node_info
        .node_id
        .0
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let peers: Vec<TopologyPeer> = state
        .connected_peers
        .iter()
        .map(|e| {
            let peer_id = e.key().clone();
            let peer_id_bech32 = encode_bech32(&state.bech32_hrp, &hex_to_bytes(&peer_id));
            let (addr, ts) = e.value();
            let stale = ts.elapsed().as_secs() >= PEER_STALE_SECS;
            TopologyPeer { peer_id, peer_id_bech32, addr: addr.clone(), stale }
        })
        .collect();
    Json(TopologyResponse { self_id, peers })
}

/// SSE-friendly view of a single message — no signature/credential bytes; only
/// metadata + a UTF-8-lossy payload preview.
#[derive(Serialize)]
struct StreamMessage {
    from: String,
    topic: String,
    seq: u64,
    timestamp_ms: u64,
    payload: String,
}

fn message_to_stream(m: &Message) -> StreamMessage {
    StreamMessage {
        from: m.publisher_id.to_string(),
        topic: m.topic_id.0.iter().map(|b| format!("{b:02x}")).collect(),
        seq: m.sequence_nr,
        timestamp_ms: m.timestamp_ms,
        payload: String::from_utf8_lossy(&m.payload).into_owned(),
    }
}

fn parse_topic_id(hex: &str) -> Option<TopicId> {
    if hex.len() != 64 {
        return None;
    }
    let mut buf = [0u8; 32];
    for i in 0..32 {
        buf[i] = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(TopicId(buf))
}

#[derive(Deserialize)]
struct StreamQuery {
    #[serde(default)]
    since: u64,
    #[serde(default = "default_stream_limit")]
    limit: usize,
}

fn default_stream_limit() -> usize {
    1000
}

async fn handle_topic_stream(
    State(state): State<Arc<ApiState>>,
    AxumPath(topic_hex): AxumPath<String>,
    Query(q): Query<StreamQuery>,
) -> axum::response::Response {
    let topic_id = match parse_topic_id(&topic_hex) {
        Some(t) => t,
        None => return StatusCode::BAD_REQUEST.into_response(),
    };

    // Replay phase — snapshot from HotCache.
    let replay = state
        .store
        .get_since(&topic_id, q.since, q.limit)
        .await
        .unwrap_or_default();

    // Live phase — subscribe to broadcast and filter by topic on this side.
    let live_rx = state.subscriber_tx.subscribe();
    let topic_for_filter = topic_id.clone();
    let live = BroadcastStream::new(live_rx).filter_map(move |res| match res {
        Ok(m) if m.topic_id == topic_for_filter => Some(m),
        _ => None,
    });

    let replay_iter = replay.into_iter();
    let combined = tokio_stream::iter(replay_iter)
        .chain(live)
        .map(|m| {
            let body = serde_json::to_string(&message_to_stream(&m)).unwrap_or_default();
            Ok::<_, Infallible>(Event::default().data(body))
        });

    Sse::new(combined)
        .keep_alive(
            axum::response::sse::KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("ping"),
        )
        .into_response()
}

async fn handle_events(State(state): State<Arc<ApiState>>) -> Sse<impl tokio_stream::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = state.event_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|res| {
        res.ok().and_then(|evt| {
            serde_json::to_string(&evt).ok().map(|data| {
                Ok(Event::default().data(data))
            })
        })
    });
    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}

// ---------------------------------------------------------------------------
// Server startup
// ---------------------------------------------------------------------------

pub async fn start(state: Arc<ApiState>, addr: SocketAddr) {
    let app = Router::new()
        .route("/", get(handle_root))
        .route("/api/status", get(handle_status))
        .route("/api/peers", get(handle_peers))
        .route("/api/topics", get(handle_topics))
        .route("/api/messages", get(handle_messages))
        .route("/api/topology", get(handle_topology))
        .route("/api/topics/{topic_hex}/stream", get(handle_topic_stream))
        .route("/events", get(handle_events))
        .layer(CorsLayer::permissive())
        .with_state(state);

    info!(%addr, "HTTP API listening");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind HTTP port");
    axum::serve(listener, app)
        .await
        .expect("HTTP server failed");
}
