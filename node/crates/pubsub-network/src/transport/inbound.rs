//! Inbound side of the QUIC transport.
//!
//! The accept loop pulls connections off the endpoint and dispatches each one
//! to `handle_connection`, which routes streams by type:
//!
//! * **Unidirectional** → application messages (one-way delivery).
//! * **Bidirectional**  → tag-routed:
//!   * `GOSSIP_CYCLON` / `GOSSIP_VICINITY` → one-shot request/response gossip.
//!   * `PUBLISH`                          → one-shot request/ack.
//!   * `SUBSCRIBE`                        → streaming response until the handler
//!     drops the channel sender.

use std::sync::Arc;

use dashmap::DashMap;
use quinn::{Connection, Endpoint};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use pubsub_types::node::{node_id_from_addr, NodeId};
use pubsub_types::traits::{
    InboundGossip, InboundPublish, InboundSubscribe, GOSSIP_CYCLON, GOSSIP_VICINITY, PUBLISH,
    SUBSCRIBE,
};

use super::frame::{read_framed, write_framed, write_framed_no_finish};
use super::tls::peer_cert_node_id;

pub(super) async fn accept_loop(
    endpoint: Endpoint,
    connections: Arc<DashMap<NodeId, Connection>>,
    app_tx: mpsc::Sender<(NodeId, Vec<u8>)>,
    cyclon_gossip_tx: mpsc::Sender<InboundGossip>,
    vicinity_gossip_tx: mpsc::Sender<InboundGossip>,
    subscribe_tx: mpsc::Sender<InboundSubscribe>,
    publish_tx: mpsc::Sender<InboundPublish>,
) {
    while let Some(incoming) = endpoint.accept().await {
        let connections = connections.clone();
        let app_tx = app_tx.clone();
        let cyclon_tx = cyclon_gossip_tx.clone();
        let vicinity_tx = vicinity_gossip_tx.clone();
        let subscribe_tx = subscribe_tx.clone();
        let publish_tx = publish_tx.clone();

        tokio::spawn(async move {
            match incoming.await {
                Ok(conn) => {
                    debug!(remote = %conn.remote_address(), "Accepted incoming QUIC connection");
                    handle_connection(
                        conn,
                        connections,
                        app_tx,
                        cyclon_tx,
                        vicinity_tx,
                        subscribe_tx,
                        publish_tx,
                    )
                    .await;
                }
                Err(e) => warn!("Failed to accept incoming connection: {e}"),
            }
        });
    }
    info!("QUIC accept loop terminated");
}

async fn handle_connection(
    conn: Connection,
    _connections: Arc<DashMap<NodeId, Connection>>,
    app_tx: mpsc::Sender<(NodeId, Vec<u8>)>,
    cyclon_gossip_tx: mpsc::Sender<InboundGossip>,
    vicinity_gossip_tx: mpsc::Sender<InboundGossip>,
    subscribe_tx: mpsc::Sender<InboundSubscribe>,
    publish_tx: mpsc::Sender<InboundPublish>,
) {
    // Try cert-based NodeId; without mutual TLS the server sees no client cert.
    let sender_id =
        peer_cert_node_id(&conn).unwrap_or_else(|| node_id_from_addr(conn.remote_address()));

    loop {
        tokio::select! {
            uni = conn.accept_uni() => {
                match uni {
                    Ok(mut recv) => {
                        let tx = app_tx.clone();
                        let id = sender_id.clone();
                        tokio::spawn(async move {
                            match read_framed(&mut recv).await {
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

            bi = conn.accept_bi() => {
                match bi {
                    Ok((mut send, mut recv)) => {
                        let cyclon_tx = cyclon_gossip_tx.clone();
                        let vicinity_tx = vicinity_gossip_tx.clone();
                        let subscribe_tx = subscribe_tx.clone();
                        let publish_tx = publish_tx.clone();
                        let id = sender_id.clone();
                        tokio::spawn(async move {
                            let tagged_request = match read_framed(&mut recv).await {
                                Ok(r) => r,
                                Err(e) => {
                                    warn!("Failed to read bi-stream request: {e}");
                                    return;
                                }
                            };
                            if tagged_request.is_empty() {
                                warn!(?id, "Empty bi-stream request");
                                return;
                            }
                            let tag = tagged_request[0];
                            let payload = tagged_request[1..].to_vec();

                            match tag {
                                GOSSIP_CYCLON | GOSSIP_VICINITY => {
                                    let tx = if tag == GOSSIP_CYCLON {
                                        cyclon_tx
                                    } else {
                                        vicinity_tx
                                    };
                                    let (resp_tx, resp_rx) =
                                        tokio::sync::oneshot::channel::<Vec<u8>>();
                                    if tx.send((id, payload, resp_tx)).await.is_err() {
                                        return;
                                    }
                                    if let Ok(response) = resp_rx.await {
                                        if let Err(e) = write_framed(&mut send, &response).await {
                                            warn!("Failed to write gossip response: {e}");
                                        }
                                    }
                                }
                                PUBLISH => {
                                    let (resp_tx, resp_rx) =
                                        tokio::sync::oneshot::channel::<Vec<u8>>();
                                    if publish_tx.send((id, payload, resp_tx)).await.is_err() {
                                        return;
                                    }
                                    if let Ok(response) = resp_rx.await {
                                        if let Err(e) = write_framed(&mut send, &response).await {
                                            warn!("Failed to write publish ack: {e}");
                                        }
                                    }
                                }
                                SUBSCRIBE => {
                                    // Streaming response: handler writes any number of frames
                                    // until it drops the mpsc sender, then we finish() the stream.
                                    let (frame_tx, mut frame_rx) =
                                        mpsc::channel::<Vec<u8>>(64);
                                    if subscribe_tx.send((id, payload, frame_tx)).await.is_err() {
                                        return;
                                    }
                                    while let Some(frame) = frame_rx.recv().await {
                                        if let Err(e) =
                                            write_framed_no_finish(&mut send, &frame).await
                                        {
                                            warn!("Failed to write subscribe frame: {e}");
                                            break;
                                        }
                                    }
                                    let _ = send.finish();
                                }
                                _ => {
                                    warn!(tag, "Unknown bi-stream tag, dropping");
                                }
                            }
                        });
                    }
                    Err(e) => { debug!("Bi stream closed: {e}"); break; }
                }
            }
        }
    }
}
