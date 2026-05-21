#![forbid(unsafe_code)]
//! `pubsub_node` — minimal scaffold for a decentralized pub/sub node.
//!
//! See `specs/001-minimal-node-scaffold/` for the feature spec, plan,
//! contracts, and ADRs.

mod config;
mod error;
mod message;
mod network;
mod node;
mod peer;
mod received;

pub use config::{PeerEntry, PeerListConfig};
pub use error::{ConfigError, NetworkError, NodeError};
pub use message::Message;
pub use network::{InMemoryNetwork, Network, NetworkHandle};
pub use node::Node;
pub use peer::{BasicPeerDescriptor, PeerDescriptor, PeerId, PeerIdError};
pub use received::ReceivedDelivery;
