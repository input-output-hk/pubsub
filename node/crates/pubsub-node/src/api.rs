use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{Query, State};
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

use pubsub_types::message::Message;
use pubsub_types::node::{NodeId, NodeInfo};

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NodeEvent {
    PeerConnected {
        peer_id: String,
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

pub struct ApiState {
    pub node_info: NodeInfo,
    pub started_at: Instant,
    /// peer_id_hex → addr
    pub connected_peers: Arc<DashMap<String, String>>,
    /// topic_hex → topic_name, seeded at startup from subscriptions
    pub topic_names: Arc<DashMap<String, String>>,
    /// Recent messages per topic (capped at 200 total)
    pub recent_messages: Arc<tokio::sync::RwLock<Vec<StoredMessage>>>,
    pub event_tx: broadcast::Sender<NodeEvent>,
}

impl ApiState {
    pub fn new(node_info: NodeInfo) -> (Arc<Self>, broadcast::Sender<NodeEvent>) {
        let (tx, _) = broadcast::channel(1024);
        let state = Arc::new(Self {
            node_info,
            started_at: Instant::now(),
            connected_peers: Arc::new(DashMap::new()),
            topic_names: Arc::new(DashMap::new()),
            recent_messages: Arc::new(tokio::sync::RwLock::new(Vec::new())),
            event_tx: tx.clone(),
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

    pub fn record_peer_connected(&self, peer_id: &NodeId, addr: &str) {
        let id = peer_id.0.iter().map(|b| format!("{b:02x}")).collect::<String>();
        self.connected_peers.insert(id.clone(), addr.to_owned());
        self.send_event(NodeEvent::PeerConnected {
            peer_id: id,
            addr: addr.to_owned(),
        });
    }

}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct StatusResponse {
    node_id: String,
    addr: String,
    uptime_secs: u64,
    peer_count: usize,
    topic_count: usize,
    message_count: usize,
}

#[derive(Serialize)]
struct PeerEntry {
    peer_id: String,
    addr: String,
}

#[derive(Serialize)]
struct TopicEntry {
    topic_id: String,
    name: Option<String>,
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
    addr: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn handle_root() -> impl IntoResponse {
    axum::response::Html(include_str!("../../../dashboard/index.html"))
}

async fn handle_status(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    let msgs = state.recent_messages.read().await;
    Json(StatusResponse {
        node_id: state.node_info.node_id.0.iter().map(|b| format!("{b:02x}")).collect(),
        addr: state.node_info.addr.to_string(),
        uptime_secs: state.started_at.elapsed().as_secs(),
        peer_count: state.connected_peers.len(),
        topic_count: state.node_info.subscribed_topics.len(),
        message_count: msgs.len(),
    })
}

async fn handle_peers(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    let peers: Vec<PeerEntry> = state
        .connected_peers
        .iter()
        .map(|e| PeerEntry {
            peer_id: e.key().clone(),
            addr: e.value().clone(),
        })
        .collect();
    Json(peers)
}

async fn handle_topics(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    let topics: Vec<TopicEntry> = state
        .node_info
        .subscribed_topics
        .iter()
        .map(|t| {
            let id: String = t.0.iter().map(|b| format!("{b:02x}")).collect();
            let name = state.topic_names.get(&id).map(|v| v.clone());
            TopicEntry { topic_id: id, name }
        })
        .collect();
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
        .map(|e| TopologyPeer {
            peer_id: e.key().clone(),
            addr: e.value().clone(),
        })
        .collect();
    Json(TopologyResponse { self_id, peers })
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
