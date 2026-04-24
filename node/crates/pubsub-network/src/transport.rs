use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use dashmap::DashMap;
use quinn::{ClientConfig, Connection, Endpoint, ServerConfig, TransportConfig};
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use pubsub_types::error::PubSubError;
use pubsub_types::node::{node_id_from_addr, NodeId, NodeInfo};
use pubsub_types::traits::{GossipTransport, InboundGossip, Transport, GOSSIP_CYCLON, GOSSIP_VICINITY};

/// QUIC-based transport.
///
/// Application messages travel on unidirectional QUIC streams (fire-and-forget).
/// Gossip exchanges (Cyclon shuffle, Vicinity T-Man) travel on bidirectional QUIC
/// streams (request → response on the same stream, never touching the app channel).
///
/// Incoming gossip is routed by the leading tag byte to per-protocol channels so
/// Cyclon and Vicinity can each call `next_inbound_gossip` concurrently without
/// consuming each other's messages.
pub struct QuicTransport {
    endpoint: Endpoint,
    connections: Arc<DashMap<NodeId, Connection>>,
    /// Peer addresses kept so stale connections can be re-established.
    peer_addrs: Arc<DashMap<NodeId, SocketAddr>>,
    /// Inbound application messages (from unidirectional streams).
    incoming_rx: tokio::sync::Mutex<mpsc::Receiver<(NodeId, Vec<u8>)>>,
    #[allow(dead_code)]
    incoming_tx: mpsc::Sender<(NodeId, Vec<u8>)>,
    /// Inbound Cyclon gossip requests (tag = GOSSIP_CYCLON).
    cyclon_gossip_rx: tokio::sync::Mutex<mpsc::Receiver<InboundGossip>>,
    #[allow(dead_code)]
    cyclon_gossip_tx: mpsc::Sender<InboundGossip>,
    /// Inbound Vicinity gossip requests (tag = GOSSIP_VICINITY).
    vicinity_gossip_rx: tokio::sync::Mutex<mpsc::Receiver<InboundGossip>>,
    #[allow(dead_code)]
    vicinity_gossip_tx: mpsc::Sender<InboundGossip>,
}

impl QuicTransport {
    pub async fn new(bind_addr: SocketAddr) -> Result<Self, PubSubError> {
        let (server_config, client_config) =
            Self::generate_self_signed_config().map_err(|e| {
                PubSubError::Transport(format!("TLS setup failed: {e}"))
            })?;

        let endpoint = Endpoint::server(server_config, bind_addr).map_err(|e| {
            PubSubError::Transport(format!("Failed to bind QUIC endpoint on {bind_addr}: {e}"))
        })?;

        info!(%bind_addr, "QUIC transport listening");

        let connections: Arc<DashMap<NodeId, Connection>> = Arc::new(DashMap::new());
        let (incoming_tx, incoming_rx) = mpsc::channel(4096);
        let (cyclon_gossip_tx, cyclon_gossip_rx) = mpsc::channel(1024);
        let (vicinity_gossip_tx, vicinity_gossip_rx) = mpsc::channel(1024);

        let mut ep_clone = endpoint.clone();
        ep_clone.set_default_client_config(client_config);

        let transport = Self {
            endpoint: ep_clone,
            connections: connections.clone(),
            peer_addrs: Arc::new(DashMap::new()),
            incoming_rx: tokio::sync::Mutex::new(incoming_rx),
            incoming_tx: incoming_tx.clone(),
            cyclon_gossip_rx: tokio::sync::Mutex::new(cyclon_gossip_rx),
            cyclon_gossip_tx: cyclon_gossip_tx.clone(),
            vicinity_gossip_rx: tokio::sync::Mutex::new(vicinity_gossip_rx),
            vicinity_gossip_tx: vicinity_gossip_tx.clone(),
        };

        let accept_endpoint = transport.endpoint.clone();
        let accept_connections = connections.clone();
        let accept_app_tx = incoming_tx.clone();
        let accept_cyclon_tx = cyclon_gossip_tx.clone();
        let accept_vicinity_tx = vicinity_gossip_tx.clone();

        tokio::spawn(async move {
            Self::accept_loop(
                accept_endpoint,
                accept_connections,
                accept_app_tx,
                accept_cyclon_tx,
                accept_vicinity_tx,
            )
            .await;
        });

        Ok(transport)
    }

    fn generate_self_signed_config()
    -> Result<(ServerConfig, ClientConfig), Box<dyn std::error::Error>> {
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])?;
        let cert_der = CertificateDer::from(cert.cert);
        let key_der = PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der());

        // Keep connections alive with PING frames; remove the default 30 s idle
        // timeout so a quiet node doesn't lose all its connections.
        let mut tc = TransportConfig::default();
        tc.keep_alive_interval(Some(Duration::from_secs(15)));
        tc.max_idle_timeout(None);
        let tc = Arc::new(tc);

        let mut server_config = ServerConfig::with_single_cert(
            vec![cert_der.clone()],
            key_der.into(),
        )?;
        server_config.transport_config(tc.clone());

        let crypto = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
            .with_no_client_auth();

        let mut client_config = ClientConfig::new(Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(crypto)?,
        ));
        client_config.transport_config(tc);

        Ok((server_config, client_config))
    }

    async fn accept_loop(
        endpoint: Endpoint,
        connections: Arc<DashMap<NodeId, Connection>>,
        app_tx: mpsc::Sender<(NodeId, Vec<u8>)>,
        cyclon_gossip_tx: mpsc::Sender<InboundGossip>,
        vicinity_gossip_tx: mpsc::Sender<InboundGossip>,
    ) {
        while let Some(incoming) = endpoint.accept().await {
            let connections = connections.clone();
            let app_tx = app_tx.clone();
            let cyclon_tx = cyclon_gossip_tx.clone();
            let vicinity_tx = vicinity_gossip_tx.clone();

            tokio::spawn(async move {
                match incoming.await {
                    Ok(conn) => {
                        debug!(remote = %conn.remote_address(), "Accepted incoming QUIC connection");
                        Self::handle_connection(conn, connections, app_tx, cyclon_tx, vicinity_tx).await;
                    }
                    Err(e) => warn!("Failed to accept incoming connection: {e}"),
                }
            });
        }
        info!("QUIC accept loop terminated");
    }

    /// Drive a single accepted connection.
    ///
    /// Unidirectional streams  → application message channel.
    /// Bidirectional streams   → gossip channel routed by tag byte:
    ///   tag 0x01 (GOSSIP_CYCLON)   → cyclon channel
    ///   tag 0x02 (GOSSIP_VICINITY) → vicinity channel
    async fn handle_connection(
        conn: Connection,
        _connections: Arc<DashMap<NodeId, Connection>>,
        app_tx: mpsc::Sender<(NodeId, Vec<u8>)>,
        cyclon_gossip_tx: mpsc::Sender<InboundGossip>,
        vicinity_gossip_tx: mpsc::Sender<InboundGossip>,
    ) {
        let sender_id = node_id_from_addr(conn.remote_address());

        loop {
            tokio::select! {
                // --- application message (one-way delivery) ---
                uni = conn.accept_uni() => {
                    match uni {
                        Ok(mut recv) => {
                            let tx = app_tx.clone();
                            let id = sender_id.clone();
                            tokio::spawn(async move {
                                match Self::read_framed(&mut recv).await {
                                    Ok(data) => {
                                        if let Err(e) = tx.send((id, data)).await {
                                            error!("App channel send failed: {e}");
                                        }
                                    }
                                    Err(e) => warn!("Failed to read app message: {e}"),
                                }
                            });
                        }
                        Err(e) => { debug!("Uni stream closed: {e}"); break; }
                    }
                }

                // --- gossip request/response (bidirectional, routed by tag) ---
                bi = conn.accept_bi() => {
                    match bi {
                        Ok((mut send, mut recv)) => {
                            let cyclon_tx = cyclon_gossip_tx.clone();
                            let vicinity_tx = vicinity_gossip_tx.clone();
                            let id = sender_id.clone();
                            tokio::spawn(async move {
                                match Self::read_framed(&mut recv).await {
                                    Ok(tagged_request) => {
                                        if tagged_request.is_empty() {
                                            warn!(?id, "Empty gossip request");
                                            return;
                                        }
                                        let tag = tagged_request[0];
                                        let payload = tagged_request[1..].to_vec();

                                        let tx = match tag {
                                            GOSSIP_CYCLON => cyclon_tx,
                                            GOSSIP_VICINITY => vicinity_tx,
                                            _ => {
                                                warn!(tag, "Unknown gossip tag, dropping");
                                                return;
                                            }
                                        };

                                        let (resp_tx, resp_rx) =
                                            tokio::sync::oneshot::channel::<Vec<u8>>();
                                        if tx.send((id, payload, resp_tx)).await.is_err() {
                                            return;
                                        }
                                        match resp_rx.await {
                                            Ok(response) => {
                                                if let Err(e) =
                                                    Self::write_framed(&mut send, &response).await
                                                {
                                                    warn!("Failed to write gossip response: {e}");
                                                }
                                            }
                                            Err(_) => {} // handler dropped sender, no reply
                                        }
                                    }
                                    Err(e) => warn!("Failed to read gossip request: {e}"),
                                }
                            });
                        }
                        Err(e) => { debug!("Bi stream closed: {e}"); break; }
                    }
                }
            }
        }
    }

    /// Read a length-prefixed frame.  Format: [4-byte BE length][payload]
    async fn read_framed(stream: &mut quinn::RecvStream) -> Result<Vec<u8>, PubSubError> {
        let mut len_buf = [0u8; 4];
        stream
            .read_exact(&mut len_buf)
            .await
            .map_err(|e| PubSubError::Transport(format!("Failed to read frame length: {e}")))?;

        let len = u32::from_be_bytes(len_buf) as usize;
        if len > 16 * 1024 * 1024 {
            return Err(PubSubError::Transport(format!("Frame too large: {len} bytes")));
        }

        let mut payload = vec![0u8; len];
        stream
            .read_exact(&mut payload)
            .await
            .map_err(|e| PubSubError::Transport(format!("Failed to read frame payload: {e}")))?;

        Ok(payload)
    }

    /// Write a length-prefixed frame and finish the send stream.
    async fn write_framed(
        stream: &mut quinn::SendStream,
        data: &[u8],
    ) -> Result<(), PubSubError> {
        let len = (data.len() as u32).to_be_bytes();
        stream
            .write_all(&len)
            .await
            .map_err(|e| PubSubError::Transport(format!("Failed to write frame length: {e}")))?;
        stream
            .write_all(data)
            .await
            .map_err(|e| PubSubError::Transport(format!("Failed to write frame payload: {e}")))?;
        stream
            .finish()
            .map_err(|e| PubSubError::Transport(format!("Failed to finish stream: {e}")))?;
        Ok(())
    }

    async fn get_or_connect(&self, peer: &NodeId, addr: SocketAddr) -> Result<Connection, PubSubError> {
        if let Some(conn) = self.connections.get(peer) {
            return Ok(conn.clone());
        }
        let conn = self
            .endpoint
            .connect(addr, "localhost")
            .map_err(|e| PubSubError::Connection(format!("Connect initiation failed: {e}")))?
            .await
            .map_err(|e| PubSubError::Connection(format!("QUIC handshake failed: {e}")))?;

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
        Self::write_framed(&mut stream, data).await
    }
}

// ---------------------------------------------------------------------------
// Transport impl (application messages, unidirectional)
// ---------------------------------------------------------------------------

#[async_trait]
impl Transport for QuicTransport {
    async fn send(&self, peer: &NodeId, data: &[u8]) -> Result<(), PubSubError> {
        let conn = match self.live_connection(peer).await {
            Ok(c) => c,
            Err(e) => return Err(e),
        };
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

        // Prepend the protocol tag byte so the receiver can route the request.
        let mut tagged = Vec::with_capacity(1 + request.len());
        tagged.push(tag);
        tagged.extend_from_slice(&request);

        let len = (tagged.len() as u32).to_be_bytes();
        send.write_all(&len)
            .await
            .map_err(|e| PubSubError::Transport(format!("gossip write len: {e}")))?;
        send.write_all(&tagged)
            .await
            .map_err(|e| PubSubError::Transport(format!("gossip write payload: {e}")))?;
        send.finish()
            .map_err(|e| PubSubError::Transport(format!("gossip finish: {e}")))?;

        // Read response (no tag on responses — they're returned on the same stream).
        Self::read_framed(&mut recv).await
    }

    async fn next_inbound_gossip(&self, tag: u8) -> Result<InboundGossip, PubSubError> {
        match tag {
            GOSSIP_CYCLON => {
                let mut rx = self.cyclon_gossip_rx.lock().await;
                rx.recv()
                    .await
                    .ok_or_else(|| PubSubError::Transport("Cyclon gossip channel closed".to_string()))
            }
            GOSSIP_VICINITY => {
                let mut rx = self.vicinity_gossip_rx.lock().await;
                rx.recv()
                    .await
                    .ok_or_else(|| PubSubError::Transport("Vicinity gossip channel closed".to_string()))
            }
            _ => Err(PubSubError::Transport(format!("Unknown gossip tag: {tag}"))),
        }
    }
}

// ---------------------------------------------------------------------------
// TLS: skip verification — testnet / development only
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct SkipServerVerification;

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::ED25519,
        ]
    }
}
