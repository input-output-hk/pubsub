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
//! - [`TopicId`] — the topic carried on every [`Message`]; opaque newtype
//!   parallel to [`PeerId`].
//! - [`Message`], [`MessagePayload`] — message envelope and body kinds. The
//!   envelope carries a [`TopicId`] and a payload; currently only
//!   [`MessagePayload::Ping`] is defined.
//! - [`ReceivedDelivery`] — one observed delivery returned by
//!   [`Node::received_messages`].
//! - [`SubscribeOutcome`], [`UnsubscribeOutcome`] — return values for the
//!   runtime subscription mutators on [`Node`].
//! - [`NodeConfig`], [`PeerEntry`], [`load_node_config`] — TOML-driven
//!   configuration.
//! - [`ConfigError`], [`NetworkError`], [`NodeError`], [`PeerIdError`],
//!   [`TopicIdError`] — typed failure modes.

mod config;
pub mod crypto;
mod error;
mod message;
mod network;
mod node;
mod peer;
mod received;
mod topic;

pub use config::{load_node_config, NodeConfig, PeerEntry};
pub use crypto::mock::{derive_public, KeyPair, MockCryptoScheme, TestSigner, TestVerifier};
pub use crypto::{
    MessageHash, PrivateKey, PublicKey, Signature, Signer, Timestamp, Verifier, VerifyError,
};
pub use error::{ConfigError, NetworkError, NodeError};
pub use message::{Message, MessagePayload};
pub use network::{InMemoryNetwork, Network, NetworkHandle};
pub use node::{Node, SubscribeOutcome, UnsubscribeOutcome};
pub use peer::{BasicPeerDescriptor, PeerDescriptor, PeerId, PeerIdError};
pub use received::ReceivedDelivery;
pub use topic::{TopicId, TopicIdError};
