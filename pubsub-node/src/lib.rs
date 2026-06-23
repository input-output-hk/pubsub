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
//! - [`Message`], [`SignedMessage`], [`PlainMessage`], [`MessagePayload`] —
//!   the protocol-message hierarchy. [`Message`] is a `#[non_exhaustive]` enum;
//!   [`Message::Dissemination`] carries a [`SignedMessage`] (signed-over
//!   [`PlainMessage`] content plus a signature, with a [`TopicId`], a
//!   [`PublisherId`], and a [`MessagePayload`] body — currently only
//!   [`MessagePayload::Ping`]), and [`Message::Connection`] carries a
//!   [`ConnectionMessage`] (a signed [`PlainConnection`] — the carried emitter
//!   plus a [`ConnectionAction`]).
//! - [`UpstreamState`], [`ConnectionStrategy`], [`ConnectToAllCandidates`],
//!   [`ConnectionAcceptanceStrategy`], [`AcceptFromAllCandidates`] —
//!   the logical-connection vocabulary: a node's upstream connections carry an
//!   explicit state, an injected strategy selects which upstreams to dial on a
//!   setup event, and an injected acceptance strategy decides which inbound
//!   requests to accept as downstream. Read the topology via
//!   [`Node::upstream_connections`]/[`Node::downstream_connections`].
//! - [`crypto`] — the [`Signer`]/[`Verifier`] trait pair and the byte-newtype
//!   types they operate over ([`PublicKey`], [`PrivateKey`], [`Signature`],
//!   [`MessageHash`], [`Timestamp`]); [`crypto::mock`] holds the test crypto.
//! - [`Event`], [`EventQueue`] — the node's single event queue. Producers push
//!   [`Event`]s via a cloned [`EventQueue`]; the node drains them in one loop.
//! - [`ReceivedDelivery`] — one observed delivery returned by
//!   [`Node::received_messages`].
//! - [`NodeConfig`], [`PeerEntry`], [`load_node_config`] — TOML-driven
//!   configuration.
//! - [`ConfigError`], [`NetworkError`], [`NodeError`], [`PeerIdError`],
//!   [`TopicIdError`] — typed failure modes.

mod acceptance;
mod config;
mod connection;
pub mod crypto;
mod error;
mod event;
mod fanout;
mod message;
mod network;
mod node;
mod peer;
mod received;
mod state;
mod subscription_registry;
mod topic;
mod topic_registry;

pub use acceptance::{AcceptFromAllCandidates, ConnectionAcceptanceStrategy};
pub use config::{load_node_config, NodeConfig, PeerEntry};
pub use connection::{ConnectToAllCandidates, ConnectionStrategy, UpstreamState};
pub use crypto::mock::{derive_public, KeyPair, MockCryptoScheme, TestSigner, TestVerifier};
pub use crypto::{
    MessageHash, PrivateKey, PublicKey, Signature, Signer, Timestamp, Verifier, VerifyError,
};
pub use error::{ConfigError, NetworkError, NodeError};
pub use event::{Event, EventQueue};
pub use fanout::{FanoutStrategy, ForwardToAll};
pub use message::{
    ConnectionAction, ConnectionMessage, Message, MessagePayload, PlainConnection, PlainMessage,
    PublisherId, SignedMessage,
};
pub use network::{InMemoryNetwork, Network, NetworkHandle};
pub use node::Node;
pub use peer::{BasicPeerDescriptor, PeerDescriptor, PeerId, PeerIdError};
pub use received::{Origin, ReceivedDelivery};
pub use subscription_registry::{
    InMemorySubscriptionRegistry, MembershipEvent, MembershipWatch, SubscriptionRegistry,
    SubscriptionRegistryControl, SubscriptionRegistryError,
};
pub use topic::{TopicId, TopicIdError};
pub use topic_registry::{
    InMemoryTopicRegistry, TopicRegistry, TopicRegistryControl, TopicRegistryError,
    TopicRegistryEvent, TopicRegistryWatch,
};
