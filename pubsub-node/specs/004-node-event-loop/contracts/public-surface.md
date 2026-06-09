# Contract: Public Surface (004 — Node Event-Loop Refactor)

**Date**: 2026-06-09 | **Plan**: [../plan.md](../plan.md)

The library's contract for this feature is **stability**: the refactor adds **no new public
items** and changes **no existing public signature or observable semantics**. This file
records (a) the public surface that must be byte-identical in `lib.rs` re-export terms, and
(b) the crate-internal items the feature adds, so the post-implementation analyze pass can
verify both directions (constitution: spec fidelity is verified against code).

## A. Public surface — unchanged (verified against `src/lib.rs` on `main`)

The `pub use` list in `src/lib.rs` MUST be identical before and after this feature:

```rust
pub use config::{load_node_config, NodeConfig, PeerEntry};
pub use crypto::mock::{derive_public, KeyPair, MockCryptoScheme, TestSigner, TestVerifier};
pub use crypto::{
    MessageHash, PrivateKey, PublicKey, Signature, Signer, Timestamp, Verifier, VerifyError,
};
pub use error::{ConfigError, NetworkError, NodeError};
pub use event::{Event, EventQueue};
pub use message::{Message, MessagePayload, PlainMessage, PublisherId, SignedMessage};
pub use network::{InMemoryNetwork, Network, NetworkHandle};
pub use node::{Node, SubscribeOutcome, UnsubscribeOutcome};
pub use peer::{BasicPeerDescriptor, PeerDescriptor, PeerId, PeerIdError};
pub use received::ReceivedDelivery;
pub use topic::{TopicId, TopicIdError};
```

(`pub mod crypto` likewise unchanged.)

`Node`'s public methods keep their exact signatures and observable semantics:

| Method | Signature (unchanged) | Semantics (unchanged) |
|---|---|---|
| `Node::new` | `async fn new<N: Network>(PeerId, NodeConfig, HashSet<TopicId>, Arc<N>, Arc<dyn Verifier>) -> Result<Self, NodeError>` | registers, spawns loop + network producer, ready on return |
| `send` | `async fn send(&self, &PeerId, Message) -> Result<(), NodeError>` | fire-and-forget to unregistered ids (warn log), no sender-side subscription coupling |
| `id` / `peers` | `fn id(&self) -> &PeerId` / `fn peers(&self) -> &[BasicPeerDescriptor]` | identical |
| `events` | `fn events(&self) -> EventQueue` | cloneable push handle; pushes after shutdown silently dropped |
| `spawn_producer` | `fn spawn_producer<F, Fut>(&mut self, F)` (same bounds) | node-owned, aborted on drop |
| `received_messages` | `fn received_messages(&self) -> Vec<ReceivedDelivery>` | **sync**, stable clone, receive order |
| `subscriptions` | `fn subscriptions(&self) -> Vec<TopicId>` | **sync**, stable clone, unspecified order |
| `subscribe` | `fn subscribe(&self, TopicId) -> SubscribeOutcome` | **sync**; `Added` / `AlreadyPresent` |
| `unsubscribe` | `fn unsubscribe(&self, TopicId) -> UnsubscribeOutcome` | **sync**; `Removed` / `NotSubscribed` |
| `Drop` | — | aborts event loop and all producers |

Operator-visible log events (`message_dropped` + `cause`, `topic_subscribed`,
`topic_unsubscribed`, noop variants, `recv`, send-drop warns) keep their event names and
fields. They are operator UX, not a contract surface tests may assert on; listed only so the
refactor does not silently rename them.

## B. Crate-internal additions — MUST NOT appear in the public API

New module `src/state.rs`, **not** re-exported from `lib.rs`:

| Item | Visibility | Check |
|---|---|---|
| `struct NodeState` | `pub(crate)` | no `pub use` in `lib.rs`; not reachable from `tests/` |
| `enum Effect` (uninhabited, `#[non_exhaustive]`) | `pub(crate)` | same |
| `fn apply` | `pub(crate)` | same |
| `handle_message_received` (and future handlers) | private to `state.rs` | same |
| `mod state` declaration in `lib.rs` | `mod state;` (no `pub`) | grep `lib.rs` |

## C. Verification procedure (post-implementation analyze pass)

1. `git diff main -- src/lib.rs` shows only the `mod state;` line added — no `pub use`
   change.
2. `grep -n "pub " src/state.rs` shows only `pub(crate)` items.
3. Existing integration tests under `tests/` compile and pass unmodified (they exercise only
   surface A).
4. An attempt to reference `pubsub_node::NodeState` from `tests/` fails to compile (spot
   check; not a committed test).
