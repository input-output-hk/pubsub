#![forbid(unsafe_code)]
//! `pubsub_node` — minimal scaffold for a decentralized pub/sub node.
//!
//! The crate exposes:
//!
//! - [`Node`] — a network participant that originates and observes messages.
//! - [`Network`], [`InMemoryNetwork`], [`NetworkHandle`] — the routing layer
//!   that connects nodes within a single process.
//! - [`PeerId`] — the identity type for addressing peers.
//! - [`TopicId`] — the topic carried on every [`Message`]; opaque newtype
//!   parallel to [`PeerId`].
//! - [`Message`], [`SignedMessage`], [`PlainMessage`], [`MessagePayload`] —
//!   the protocol-message hierarchy. [`Message`] is a `#[non_exhaustive]` enum;
//!   [`Message::Dissemination`] carries a [`SignedMessage`] (signed-over
//!   [`PlainMessage`] content plus a signature, with a [`TopicId`], a
//!   [`PublisherId`], and a [`MessagePayload`] body — currently only
//!   [`MessagePayload::Ping`]), and one connection variant per
//!   [`HandshakeKind`] ([`Message::RelayConnection`] /
//!   [`Message::PublisherConnection`] / [`Message::SymmetricConnection`])
//!   carries a [`ConnectionMessage`] (a signed [`PlainConnection`] — the
//!   carried emitter plus a [`ConnectionAction`]; the handshake kind is bound
//!   into the preimage).
//! - [`LinkKind`], [`LinkKey`], [`LinkState`],
//!   [`ConnectionStrategy`], [`Selection`],
//!   [`ConnectionAcceptanceStrategy`], [`UnifiedAcceptance`] —
//!   the logical-link vocabulary: a node holds per-`(topic, peer, kind)` links
//!   in two directions (upstream sources, downstream targets); injected
//!   selection strategies dial relay and publisher links on a dial event, and
//!   injected acceptance strategies decide which inbound requests to accept.
//!   Read the topology per class via [`Node::upstream_relays`] /
//!   [`Node::downstream_relays`] / [`Node::upstream_publishers`] /
//!   [`Node::downstream_publishers`].
//! - [`crypto`] — the [`Signer`]/[`Verifier`] trait pair and the byte-newtype
//!   types they operate over ([`PublicKey`], [`PrivateKey`], [`Signature`],
//!   [`MessageHash`], [`Timestamp`]); [`crypto::mock`] holds the test crypto.
//! - [`Event`], [`EventQueue`] — the node's single event queue. Producers push
//!   [`Event`]s via a cloned [`EventQueue`]; the node drains them in one loop.
//! - [`ReceivedDelivery`] — one observed delivery returned by
//!   [`Node::received_messages`].
//! - [`ConfigError`], [`NetworkError`], [`NodeError`], [`PeerIdError`],
//!   [`TopicIdError`] — typed failure modes.

mod connection_state;
pub mod crypto;
mod error;
mod event;
#[cfg(feature = "experiments")]
pub mod experiments;
mod message;
mod network;
mod node;
mod peer;
mod received;
mod state;
mod strategies;
mod subscription_registry;
mod topic;
mod topic_registry;

pub use connection_state::{LinkKey, LinkKind, LinkState};
pub use crypto::mock::{derive_public, KeyPair, MockCryptoScheme, TestSigner, TestVerifier};
pub use crypto::{
    MessageHash, PrivateKey, PublicKey, Signature, Signer, Timestamp, Verifier, VerifyError,
};
pub use error::{ConfigError, NetworkError, NodeError};
pub use event::{Event, EventQueue};
pub use message::{
    ConnectionAction, ConnectionMessage, HandshakeKind, Message, MessagePayload, PlainConnection,
    PlainMessage, PublisherId, SignedMessage,
};
pub use network::{InMemoryNetwork, Network, NetworkHandle};
pub use node::Node;
pub use peer::{PeerId, PeerIdError};
pub use received::{Origin, ReceivedDelivery};
pub use strategies::acceptance::{Admission, ConnectionAcceptanceStrategy, UnifiedAcceptance};
pub use strategies::config::{
    AcceptanceParams, NodeStrategies, SelectionParams, StrategyConfigError,
};
pub use strategies::connection::{ConnectionStrategy, Selection};
pub use strategies::edge::{is_valid_edge, is_valid_edge_publisher, is_valid_edge_sym};
pub use strategies::fanout::{
    FanoutStrategy, FanoutStrategyKind, ForwardToAll, ForwardToRelays, UnknownFanoutStrategy,
};
pub use strategies::view::NodeView;
pub use subscription_registry::{
    InMemorySubscriptionRegistry, MembershipEvent, MembershipSnapshot, MembershipWatch,
    SubscriptionRegistry, SubscriptionRegistryControl, SubscriptionRegistryError,
};
pub use topic::{TopicId, TopicIdError};
pub use topic_registry::{
    InMemoryTopicRegistry, TopicRegistry, TopicRegistryControl, TopicRegistryError,
    TopicRegistryEvent, TopicRegistryWatch, TopicSnapshot,
};
