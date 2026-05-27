#![forbid(unsafe_code)]
//! `pubsub_node` — minimal scaffold for a decentralized pub/sub node.
//!
//! The crate exposes:
//!
//! - [`Node`] — a network participant that originates and observes messages.
//! - [`Network`], [`InMemoryNetwork`], [`NetworkHandle`] — the routing layer
//!   that connects nodes within a single process.
//! - [`PeerId`], [`PeerDescriptor`], [`BasicPeerDescriptor`] — identity types
//!   for addressing peers.
//! - [`Message`] — the message kinds nodes exchange (currently only
//!   [`Message::Ping`]).
//! - [`ReceivedDelivery`] — one observed delivery returned by
//!   [`Node::received_messages`].
//! - [`PeerListConfig`], [`PeerEntry`], [`load_peer_list`] — TOML-driven
//!   configuration.
//! - [`ConfigError`], [`NetworkError`], [`NodeError`], [`PeerIdError`] —
//!   typed failure modes.
//!
//! For the current iteration's specification, contracts, and design notes,
//! see `specs/001-minimal-node-scaffold/`.

mod config;
mod error;
mod message;
mod network;
mod node;
mod peer;
mod received;

pub use config::{load_peer_list, PeerEntry, PeerListConfig};
pub use error::{ConfigError, NetworkError, NodeError};
pub use message::Message;
pub use network::{InMemoryNetwork, Network, NetworkHandle};
pub use node::Node;
pub use peer::{BasicPeerDescriptor, PeerDescriptor, PeerId, PeerIdError};
pub use received::ReceivedDelivery;
