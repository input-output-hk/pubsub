//! QUIC-based transport.
//!
//! Application messages travel on unidirectional QUIC streams (fire-and-forget).
//! Gossip exchanges (Cyclon shuffle, Vicinity T-Man), publish acks, and
//! subscribe streams travel on bidirectional QUIC streams routed by a leading
//! tag byte; see [`inbound`] for the dispatch table.
//!
//! Each node uses its Ed25519 signing key as its TLS identity. Outbound
//! connections verify that the peer's TLS cert public key matches the
//! expected NodeId, so identity is pinned by the handshake itself rather than
//! by a CA chain — see [`tls`].

mod frame;
mod inbound;
mod tls;

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use quinn::{Connection, Endpoint};
use tokio::sync::mpsc;
use tracing::{debug, info};

use pubsub_types::error::PubSubError;
use pubsub_types::node::{node_id_from_addr, NodeId, NodeInfo};
use pubsub_types::traits::{
    GossipTransport, InboundGossip, InboundPublish, InboundSubscribe, PublishTransport,
    SubscribeTransport, Transport, GOSSIP_CYCLON, GOSSIP_VICINITY, PUBLISH, SUBSCRIBE,
};

use frame::{read_framed, write_framed};
use tls::{generate_tls_config, peer_cert_node_id};

// ---------------------------------------------------------------------------
// Admission-control limits
// ---------------------------------------------------------------------------
//
// Two layered caps bound the work an unauthenticated peer can force on us.
// Both apply equally to inbound peer-to-peer relay traffic and inbound client
// (publisher / subscriber) traffic — the QUIC layer cannot tell them apart
// before the cert handshake completes.
//
// Tune downwards if dashboards show a quiet network; tune upwards only with
// evidence that a legitimate peer is being shed.

/// Maximum concurrent QUIC connections accepted from any single remote IP.
/// One is the legitimate steady state; 4 leaves headroom for retries and
/// short-lived overlap during reconnect.
pub(super) const MAX_CONN_PER_IP: usize = 4;

/// Maximum concurrent **bidirectional** streams a peer may have open against
/// us per connection. The protocol uses at most 4 in flight simultaneously
/// (Cyclon, Vicinity, Publish, Subscribe); 8 is generous slack.
pub(super) const MAX_CONCURRENT_BIDI_PER_CONN: u32 = 8;

/// Maximum concurrent **unidirectional** streams a peer may have open against
/// us per connection. Each carries one inter-node application message; we
/// allow more headroom than bidi because gossip-driven fan-in can be bursty.
pub(super) const MAX_CONCURRENT_UNI_PER_CONN: u32 = 64;

/// Owns the QUIC endpoint, the connection map, and the receiving side of every
/// inbound channel. Sending sides are owned by the spawned [`inbound::accept_loop`]
/// — when that task dies, the channels close and `recv` calls surface an error,
/// which is the desired signal that the transport is no longer operating.
pub struct QuicTransport {
    endpoint: Endpoint,
    connections: Arc<DashMap<NodeId, Connection>>,
    /// Peer addresses kept so stale connections can be re-established.
    peer_addrs: Arc<DashMap<NodeId, SocketAddr>>,
    incoming_rx: tokio::sync::Mutex<mpsc::Receiver<(NodeId, Vec<u8>)>>,
    cyclon_gossip_rx: tokio::sync::Mutex<mpsc::Receiver<InboundGossip>>,
    vicinity_gossip_rx: tokio::sync::Mutex<mpsc::Receiver<InboundGossip>>,
    subscribe_rx: tokio::sync::Mutex<mpsc::Receiver<InboundSubscribe>>,
    publish_rx: tokio::sync::Mutex<mpsc::Receiver<InboundPublish>>,
}

impl QuicTransport {
    /// Bind a QUIC endpoint at `bind_addr` using the node's Ed25519 signing key
    /// as the TLS identity.  The cert public key doubles as the node's network
    /// identity so peers can derive the NodeId from the TLS handshake alone.
    pub async fn new(
        bind_addr: SocketAddr,
        signing_key_seed: &[u8; 32],
    ) -> Result<Self, PubSubError> {
        let (server_config, client_config) = generate_tls_config(signing_key_seed)
            .map_err(|e| PubSubError::Transport(format!("TLS setup failed: {e}")))?;

        let endpoint = Endpoint::server(server_config, bind_addr).map_err(|e| {
            PubSubError::Transport(format!("Failed to bind QUIC endpoint on {bind_addr}: {e}"))
        })?;

        info!(%bind_addr, "QUIC transport listening");

        let connections: Arc<DashMap<NodeId, Connection>> = Arc::new(DashMap::new());
        let (incoming_tx, incoming_rx) = mpsc::channel(4096);
        let (cyclon_gossip_tx, cyclon_gossip_rx) = mpsc::channel(1024);
        let (vicinity_gossip_tx, vicinity_gossip_rx) = mpsc::channel(1024);
        let (subscribe_tx, subscribe_rx) = mpsc::channel(256);
        let (publish_tx, publish_rx) = mpsc::channel(1024);

        let mut ep_clone = endpoint.clone();
        ep_clone.set_default_client_config(client_config);

        let ip_counts: Arc<DashMap<IpAddr, usize>> = Arc::new(DashMap::new());

        let transport = Self {
            endpoint: ep_clone,
            connections: connections.clone(),
            peer_addrs: Arc::new(DashMap::new()),
            incoming_rx: tokio::sync::Mutex::new(incoming_rx),
            cyclon_gossip_rx: tokio::sync::Mutex::new(cyclon_gossip_rx),
            vicinity_gossip_rx: tokio::sync::Mutex::new(vicinity_gossip_rx),
            subscribe_rx: tokio::sync::Mutex::new(subscribe_rx),
            publish_rx: tokio::sync::Mutex::new(publish_rx),
        };

        let accept_endpoint = transport.endpoint.clone();
        let channels = inbound::InboundChannels {
            app_tx: incoming_tx,
            cyclon_gossip_tx,
            vicinity_gossip_tx,
            subscribe_tx,
            publish_tx,
        };
        tokio::spawn(async move {
            inbound::accept_loop(accept_endpoint, connections, ip_counts, channels).await;
        });

        Ok(transport)
    }

    /// Connect to a bootstrap peer whose NodeId is not yet known.
    ///
    /// The real NodeId is derived from the peer's TLS certificate public key after
    /// the QUIC handshake.  Falls back to an address-based placeholder when the cert
    /// cannot be parsed (e.g. older node versions).
    pub async fn connect_bootstrap(&self, addr: SocketAddr) -> Result<NodeId, PubSubError> {
        let conn = self
            .endpoint
            .connect(addr, "pubsub-node")
            .map_err(|e| {
                PubSubError::Connection(format!("Bootstrap connect to {addr} failed: {e}"))
            })?
            .await
            .map_err(|e| {
                PubSubError::Connection(format!("Bootstrap handshake with {addr} failed: {e}"))
            })?;

        let node_id = peer_cert_node_id(&conn).unwrap_or_else(|| node_id_from_addr(addr));

        self.connections.insert(node_id.clone(), conn);
        self.peer_addrs.insert(node_id.clone(), addr);
        debug!(%addr, "Bootstrap connection established");
        Ok(node_id)
    }

    /// Connect to `peer` at `addr`, verifying that the TLS cert NodeId matches `peer`.
    async fn get_or_connect(
        &self,
        peer: &NodeId,
        addr: SocketAddr,
    ) -> Result<Connection, PubSubError> {
        if let Some(conn) = self.connections.get(peer) {
            return Ok(conn.clone());
        }
        let conn = self
            .endpoint
            .connect(addr, "pubsub-node")
            .map_err(|e| PubSubError::Connection(format!("Connect initiation failed: {e}")))?
            .await
            .map_err(|e| PubSubError::Connection(format!("QUIC handshake failed: {e}")))?;

        // Verify that the peer's TLS cert identity matches the NodeId we expect.
        // This guards against connecting to a rogue node that has claimed a false NodeId in gossip.
        if let Some(cert_id) = peer_cert_node_id(&conn) {
            if cert_id != *peer {
                conn.close(0u32.into(), b"node-id-mismatch");
                return Err(PubSubError::Connection(format!(
                    "NodeId mismatch connecting to {addr}: cert does not match expected peer"
                )));
            }
        }

        self.connections.insert(peer.clone(), conn.clone());
        self.peer_addrs.insert(peer.clone(), addr);
        debug!(%addr, "Established new QUIC connection");
        Ok(conn)
    }

    /// Resolve a live connection for `peer`, reconnecting once if stale.
    async fn live_connection(&self, peer: &NodeId) -> Result<Connection, PubSubError> {
        let addr = self.peer_addrs.get(peer).map(|a| *a).ok_or_else(|| {
            PubSubError::PeerNotFound(format!("No address for peer {:?}", peer.0))
        })?;
        if let Some(conn) = self.connections.get(peer) {
            return Ok(conn.clone());
        }
        self.get_or_connect(peer, addr).await
    }

    async fn do_send(conn: &Connection, data: &[u8]) -> Result<(), PubSubError> {
        let mut stream = conn
            .open_uni()
            .await
            .map_err(|e| PubSubError::Transport(format!("Failed to open uni stream: {e}")))?;
        write_framed(&mut stream, data).await
    }
}

// ---------------------------------------------------------------------------
// Transport impl (application messages, unidirectional)
// ---------------------------------------------------------------------------

#[async_trait]
impl Transport for QuicTransport {
    async fn send(&self, peer: &NodeId, data: &[u8]) -> Result<(), PubSubError> {
        let conn = self.live_connection(peer).await?;
        match Self::do_send(&conn, data).await {
            Ok(()) => {
                debug!("Sent {} bytes to peer", data.len());
                Ok(())
            }
            Err(e) => {
                // Stale connection — evict and reconnect once.
                self.connections.remove(peer);
                let addr = self.peer_addrs.get(peer).map(|a| *a).ok_or_else(|| {
                    PubSubError::PeerNotFound(format!("No address for peer {:?}", peer.0))
                })?;
                debug!(%addr, "Reconnecting after send failure: {e}");
                let new_conn = self.get_or_connect(peer, addr).await?;
                Self::do_send(&new_conn, data).await?;
                debug!("Sent {} bytes to peer (after reconnect)", data.len());
                Ok(())
            }
        }
    }

    async fn recv(&self) -> Result<(NodeId, Vec<u8>), PubSubError> {
        let mut rx = self.incoming_rx.lock().await;
        rx.recv()
            .await
            .ok_or_else(|| PubSubError::Transport("Incoming channel closed".to_string()))
    }

    async fn connect(&self, info: &NodeInfo) -> Result<(), PubSubError> {
        let conn = self.get_or_connect(&info.node_id, info.addr).await?;
        self.connections.insert(info.node_id.clone(), conn);
        self.peer_addrs.insert(info.node_id.clone(), info.addr);
        info!(addr = %info.addr, "Connected to peer");
        Ok(())
    }

    async fn disconnect(&self, peer: &NodeId) -> Result<(), PubSubError> {
        if let Some((_, conn)) = self.connections.remove(peer) {
            conn.close(0u32.into(), b"disconnect");
            debug!("Disconnected from peer");
        }
        Ok(())
    }

    async fn connected_peers(&self) -> Vec<NodeId> {
        self.connections.iter().map(|e| e.key().clone()).collect()
    }
}

// ---------------------------------------------------------------------------
// GossipTransport impl (bidirectional streams, tag-routed)
// ---------------------------------------------------------------------------

#[async_trait]
impl GossipTransport for QuicTransport {
    /// Open a bidirectional stream to `peer` (connecting to `addr` if needed),
    /// prepend `tag`, write the request, and read back the response.
    async fn gossip_exchange(
        &self,
        peer: &NodeId,
        addr: SocketAddr,
        tag: u8,
        request: Vec<u8>,
    ) -> Result<Vec<u8>, PubSubError> {
        let conn = self.get_or_connect(peer, addr).await?;

        let (mut send, mut recv) = conn
            .open_bi()
            .await
            .map_err(|e| PubSubError::Transport(format!("Failed to open bi stream: {e}")))?;

        let mut tagged = Vec::with_capacity(1 + request.len());
        tagged.push(tag);
        tagged.extend_from_slice(&request);
        write_framed(&mut send, &tagged).await?;

        // Read response (no tag on responses — they're returned on the same stream).
        read_framed(&mut recv).await
    }

    async fn next_inbound_gossip(&self, tag: u8) -> Result<InboundGossip, PubSubError> {
        match tag {
            GOSSIP_CYCLON => {
                let mut rx = self.cyclon_gossip_rx.lock().await;
                rx.recv().await.ok_or_else(|| {
                    PubSubError::Transport("Cyclon gossip channel closed".to_string())
                })
            }
            GOSSIP_VICINITY => {
                let mut rx = self.vicinity_gossip_rx.lock().await;
                rx.recv().await.ok_or_else(|| {
                    PubSubError::Transport("Vicinity gossip channel closed".to_string())
                })
            }
            _ => Err(PubSubError::Transport(format!("Unknown gossip tag: {tag}"))),
        }
    }
}

// ---------------------------------------------------------------------------
// SubscribeTransport impl (long-lived bidirectional stream, tag = SUBSCRIBE)
// ---------------------------------------------------------------------------

#[async_trait]
impl SubscribeTransport for QuicTransport {
    async fn subscribe_stream(
        &self,
        peer: &NodeId,
        addr: SocketAddr,
        control_frame: Vec<u8>,
    ) -> Result<mpsc::Receiver<Vec<u8>>, PubSubError> {
        let conn = self.get_or_connect(peer, addr).await?;

        let (mut send, mut recv) = conn.open_bi().await.map_err(|e| {
            PubSubError::Transport(format!("Failed to open subscribe bi stream: {e}"))
        })?;

        let mut tagged = Vec::with_capacity(1 + control_frame.len());
        tagged.push(SUBSCRIBE);
        tagged.extend_from_slice(&control_frame);
        write_framed(&mut send, &tagged).await?;

        // Pump framed responses into a channel until the peer finishes the stream.
        let (tx, rx) = mpsc::channel::<Vec<u8>>(64);
        tokio::spawn(async move {
            loop {
                match read_framed(&mut recv).await {
                    Ok(frame) => {
                        if tx.send(frame).await.is_err() {
                            return; // subscriber dropped the receiver
                        }
                    }
                    Err(e) => {
                        debug!("Subscribe stream ended: {e}");
                        return;
                    }
                }
            }
        });
        Ok(rx)
    }

    async fn next_inbound_subscribe(&self) -> Result<InboundSubscribe, PubSubError> {
        let mut rx = self.subscribe_rx.lock().await;
        rx.recv()
            .await
            .ok_or_else(|| PubSubError::Transport("Subscribe channel closed".to_string()))
    }
}

// ---------------------------------------------------------------------------
// PublishTransport impl (one-shot bi stream, tag = PUBLISH)
// ---------------------------------------------------------------------------

#[async_trait]
impl PublishTransport for QuicTransport {
    async fn publish_exchange(
        &self,
        peer: &NodeId,
        addr: SocketAddr,
        request: Vec<u8>,
    ) -> Result<Vec<u8>, PubSubError> {
        let conn = self.get_or_connect(peer, addr).await?;

        let (mut send, mut recv) = conn
            .open_bi()
            .await
            .map_err(|e| PubSubError::Transport(format!("Failed to open publish bi stream: {e}")))?;

        let mut tagged = Vec::with_capacity(1 + request.len());
        tagged.push(PUBLISH);
        tagged.extend_from_slice(&request);
        write_framed(&mut send, &tagged).await?;

        read_framed(&mut recv).await
    }

    async fn next_inbound_publish(&self) -> Result<InboundPublish, PubSubError> {
        let mut rx = self.publish_rx.lock().await;
        rx.recv()
            .await
            .ok_or_else(|| PubSubError::Transport("Publish channel closed".to_string()))
    }
}
