# Phase 1 — Data Model

**Feature**: 001-minimal-node-scaffold
**Date**: 2026-05-18

Concrete Rust types and their relationships, derived from `spec.md` Key Entities and the decisions in `research.md`. This document is the authoritative source for the public API surface that `contracts/library-api.md` then describes in interface form, and that integration tests in `tests/` assert against.

All type signatures below are illustrative — names and module paths are normative, but trait bounds may be widened during implementation as long as the FR-traceable behaviour is preserved.

---

## 1. Identifiers

### `PeerId`

```rust
// src/peer.rs
pub struct PeerId(String);
```

| Property | Value |
|---|---|
| Module | `pubsub_node::peer` |
| Visibility | `pub` |
| Derived traits | `Clone`, `Debug`, `Eq`, `PartialEq`, `Hash`, `serde::Deserialize`, `serde::Serialize` |
| Manual traits | `Display` (prints inner string verbatim), `FromStr` (validation: non-empty UTF-8, no internal NULs) |
| Invariants | Non-empty; the underlying `String` is never empty (`from_str` rejects `""`). UTF-8 by construction. |
| FR trace | FR-009 ("`id()` accessor that uniquely identifies a peer"); Clarifications S1-Q1 (UTF-8 string id, abstract surface) |

Notes:
- Uniqueness is enforced *per InMemoryNetwork instance* by the network's registration table (§4). The `PeerId` type itself is not globally unique.
- `Display` is what gets emitted into structured `tracing` fields when FR-010 demands the unregistered id be logged.

### `PeerDescriptor` (trait) and `BasicPeerDescriptor` (v1 impl)

```rust
// src/peer.rs
pub trait PeerDescriptor: Clone + Send + Sync + 'static {
    fn id(&self) -> &PeerId;
}

pub struct BasicPeerDescriptor {
    pub id: PeerId,
}

impl PeerDescriptor for BasicPeerDescriptor { … }
```

| Property | Value |
|---|---|
| Module | `pubsub_node::peer` |
| FR trace | FR-009 (abstract descriptor + `id()` accessor); Clarifications S1-Q1 (opaque type, v1 carries no other fields, future-extension hook) |

Notes:
- `Send + Sync + 'static` is required so the receive task and network can hold a descriptor across `.await` points.
- Future iterations introduce additional impls (e.g., `NetworkedPeerDescriptor { id, addr, public_key, … }`) without touching consumers.

---

## 2. Messages and envelopes

### `Message`

```rust
// src/message.rs
pub enum Message {
    Ping(u64),
}
```

| Property | Value |
|---|---|
| Module | `pubsub_node::message` |
| Derived traits | `Clone`, `Debug`, `Eq`, `PartialEq` |
| FR trace | FR-004 (Ping carries opaque numeric `N`, receiver can inspect); spec Edge Cases bullet on `N` value ranges (any value the chosen type accepts is valid) |

Notes:
- `u64` is chosen as the concrete numeric type. The Edge Cases bullet defers semantics of `N`; `u64` accepts the full range of values the bullet contemplates ("unusually large, zero" — negative is moot because `u64` is unsigned, consistent with the bullet's "any value the chosen numeric type accepts").
- `Eq` enables the await-on-delivery helper to assert payload equality.

### `Envelope` (internal)

```rust
// src/network.rs (private)
pub(crate) struct Envelope {
    pub from: PeerId,
    pub message: Message,
}
```

Not part of the public API surface. Used internally as the queue element shipped over the `mpsc` channel from network to node.

---

## 3. Configuration

### `PeerListConfig`

```rust
// src/config.rs
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PeerListConfig {
    #[serde(default)]
    pub peers: Vec<PeerEntry>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PeerEntry {
    pub id: PeerId,
}
```

TOML representation (v1 minimal schema — contracts/peer-list.toml.md is the schema spec):

```toml
[[peers]]
id = "node-a"

[[peers]]
id = "node-b"
```

| Property | Value |
|---|---|
| Module | `pubsub_node::config` |
| Loader entry point | `pub fn load_peer_list(path: &Path) -> Result<PeerListConfig, ConfigError>` |
| Invariants | `peers` may be empty (per spec Edge Cases bullet 1: empty peer set is a valid start state); `PeerEntry::id`'s `PeerId` constructor enforces non-empty. |
| FR trace | FR-001 (TOML config, peer descriptors); FR-012 (loader yields parsed value; Node constructor consumes the parsed value, not a path); Clarifications S1-Q2 (TOML), S2-Q1 (parse-at-edge layering) |

Notes on the schema design:
- The peer is wrapped under an array-of-tables (`[[peers]]`) rather than a top-level array, so future fields (`addr`, `pubkey`) can extend each entry without bumping a version field.
- No top-level `node_id` / `[self]` table — node identity is supplied at the CLI / constructor boundary (Clarifications S2-Q1).
- "Malformed config" (US3 AS-2) covers both syntactically invalid TOML and structurally invalid (missing `id`, empty `id`) cases. Both surface as `ConfigError`.

---

## 4. Network abstraction

### `Network` (trait)

```rust
// src/network.rs
pub trait Network: Send + Sync + 'static {
    async fn register(
        &self,
        id: PeerId,
    ) -> Result<NetworkHandle, NetworkError>;
}
```

| Property | Value |
|---|---|
| Module | `pubsub_node::network` |
| FR trace | FR-002 (network abstraction; nodes register and exchange messages via registered id) |

The `Network` trait exposes only `register` — sends are issued through the returned `NetworkHandle`, whose sender identity is implicit. This matches the shape future networked transports will have (per-connection handle; sender id derived from the registered/authenticated peer, not asserted by every caller) and removes the v1 footgun of asking the caller to pass its own id on every send.

### `NetworkHandle`

`NetworkHandle` is the per-node attach token returned by `register`. The Node owns it for its lifetime.

```rust
pub struct NetworkHandle {
    self_id: PeerId,
    // Outbound: cloneable sender into the network's dispatch fabric.
    // Routes Envelopes addressed to other peers into their mailboxes.
    // Internally wraps an Arc into the InMemoryNetwork registry (§4.2).
    tx: NetworkSender,
    // Inbound: single-consumer mailbox drain. Moved into the recv task
    // during Node::new via take_receiver(); subsequent NetworkHandle
    // methods do not touch rx.
    rx: tokio::sync::mpsc::UnboundedReceiver<Envelope>,
}

impl NetworkHandle {
    pub fn id(&self) -> &PeerId;
    pub async fn send(&self, to: &PeerId, message: Message)
        -> Result<(), NetworkError>;
    pub(crate) fn take_receiver(&mut self)
        -> tokio::sync::mpsc::UnboundedReceiver<Envelope>;
}

// Cloneable send-half. Held by NetworkHandle and by anything else
// that dispatches into the network. Crate-internal — NOT re-exported
// from lib.rs (the public surface is `NetworkHandle::send`).
#[derive(Clone)]
pub(crate) struct NetworkSender { /* Arc into the InMemoryNetwork registry */ }
```

The handle is structured as an actor-handle (Ryhl pattern; full rationale, alternatives, and source citations in `research.md` §12, ADR slot 0007). `Node::new` calls `handle.take_receiver()` once during construction to move `rx` into the spawned recv task; subsequent `&self` uses of the handle (all `send` calls) touch only `tx` and `self_id`.

| Property | Value |
|---|---|
| Module | `pubsub_node::network` |
| Sender identity | Implicit — the handle was issued by `register(self_id)` and carries that `PeerId` for its lifetime. Callers do NOT pass `from`. |
| Ownership | `NetworkHandle` is NOT `Clone` (the receiver-end of `mpsc::unbounded_channel` is single-consumer). The Node owns the handle and drives recv via a spawned task; `Node::send` forwards through it. |
| FR trace | FR-005 (one-to-one `send`); FR-006 (the handle supplies the logical peer identity into the recorded delivery's `from` field at enqueue time); FR-010 (unregistered-id drop + log); FR-011 (async API); FR-013 (`send().await` resolves on enqueue, not on observable delivery) |

`NetworkHandle::send` contract (FR-005, FR-006, FR-010, FR-013):
- If `to` is currently registered: enqueue an `Envelope { from: self.id().clone(), message }` onto the recipient's mailbox, emit a `tracing::debug!` for `send.accepted`, and return `Ok(())`. The `from` field is supplied by the handle from its own `self_id`, satisfying FR-006's logical-peer-identity requirement (the recorded value is what `PeerDescriptor::id()` returns for the originating peer).
- If `to` is not currently registered: drop the message, emit `tracing::warn!(target = "pubsub_node::network", peer_id = %to, "send dropped: unregistered peer id")`, and return `Ok(())` (the sender does **not** observe a synchronous error per FR-010).
- The future returned by `send` MUST resolve once the in-network operation above completes; it MUST NOT wait for the recipient to drain the mailbox (FR-013).

### `InMemoryNetwork` (v1 concrete impl)

```rust
// src/network.rs
pub struct InMemoryNetwork {
    registry: tokio::sync::RwLock<HashMap<PeerId, tokio::sync::mpsc::UnboundedSender<Envelope>>>,
}

impl InMemoryNetwork {
    pub fn new() -> Self { … }
}

impl Network for InMemoryNetwork { … }
```

| Property | Value |
|---|---|
| Shape | Hashmap of `PeerId -> UnboundedSender<Envelope>`, behind an async `RwLock` |
| Sharing | `Arc<InMemoryNetwork>` is the idiomatic way for multiple nodes to share one network; the trait bounds enable this |
| Failure modes | Only `NetworkError::DuplicateRegistration` from `register` (registration succeeds at most once per id within a single network instance). `NetworkHandle::send` — whose dispatch is backed by this InMemoryNetwork's registry — never returns `Err` for handles issued by this impl; unregistered-id addressing drops + logs per FR-010, never produces a synchronous error. |
| Spec note | The "hashmap of peers to message boxes" wording in the spec's input description is realised verbatim here |

---

## 5. Node

```rust
// src/node.rs
pub struct Node {
    // Owned post-take_receiver: carries self_id and the cloneable send-side.
    // rx has already been moved into recv_task at construction time.
    handle: NetworkHandle,
    peers: Vec<BasicPeerDescriptor>,
    received: Arc<Mutex<Vec<ReceivedDelivery>>>,
    recv_task: tokio::task::JoinHandle<()>,
}

impl Node {
    pub async fn new(
        self_id: PeerId,
        peer_list: PeerListConfig,
        network: Arc<dyn Network>,
    ) -> Result<Self, NodeError> { … }

    pub async fn send(&self, to: &PeerId, message: Message)
        -> Result<(), NodeError>;

    pub fn id(&self) -> &PeerId;

    pub fn peers(&self) -> &[BasicPeerDescriptor];

    pub fn received_messages(&self) -> Vec<ReceivedDelivery>;
}

impl Drop for Node {
    fn drop(&mut self) { self.recv_task.abort(); }
}
```

| Property | Value |
|---|---|
| Module | `pubsub_node::node` |
| Construction | `Node::new` registers on the network, spawns the receive task, and returns the fully-attached Node (Research §6 + §8) |
| Peer set semantics | Static for lifetime (FR-008); no mutation API exposed |
| Send routing | `Node::send` forwards to `self.handle.send(to, message).await`. The sender id is implicit in the handle (FR-006). The Node does **not** check whether `to` is in its peer set — that is the operator's responsibility per Configuration trust (spec Assumptions) and trust-on-arrival (FR-003). Empty-peer-set semantics (Edge Cases bullet 1, US1 AS-3) are: the *caller* simply has no peer to address to. |
| `received_messages()` | Returns a snapshot clone of the record (acquire mutex, clone vector, release). FR-006: normative observability surface. |
| FR trace | FR-002, FR-003, FR-004, FR-005, FR-006, FR-008, FR-011, FR-012, FR-013 |

### Send-side state transitions

```text
        ┌──────────────────────┐
        │  Node::send(to, msg) │
        └──────────┬───────────┘
                   │ await
                   ▼
        ┌──────────────────────┐
        │ handle.send(to, msg) │
        │ (from = self_id      │
        │  supplied by handle) │
        └──┬───────────────┬───┘
           │               │
   to registered?         to not registered?
           │               │
           ▼               ▼
  enqueue Envelope    drop + warn-log
  Ok(())              Ok(())
```

### Receive-side state transitions

```text
        ┌─────────────────────────┐
        │ recv_task spawn'd in    │
        │ Node::new (Research §6) │
        │ owning rx, taken out of │
        │ NetworkHandle via       │
        │ take_receiver()         │
        └────────────┬────────────┘
                     │ loop
                     ▼
        ┌─────────────────────────┐
        │ rx.recv().await         │
        └────────────┬────────────┘
                     │ Some(Envelope)
                     ▼
        ┌─────────────────────────┐
        │ received.lock();        │
        │ push ReceivedDelivery;  │
        │ tracing::debug!         │
        └─────────────────────────┘
                     │
                     └─► loop
        On `None` (channel closed because Drop ran): task exits.
```

---

## 6. ReceivedDelivery

```rust
// src/received.rs
pub struct ReceivedDelivery {
    pub from: PeerId,
    pub message: Message,
}
```

| Property | Value |
|---|---|
| Module | `pubsub_node::received` |
| Derived traits | `Clone`, `Debug`, `Eq`, `PartialEq` |
| FR trace | FR-006 ("carrying at least the sender's id and the message payload"); US1 AS-1 ("attributed to A") |

Notes:
- No timestamp field at v1: the spec doesn't require ordering claims beyond per-sender FIFO (Research §9), and adding a timestamp would force a clock-injection decision the scaffold doesn't need yet.
- Ordering: `Vec<ReceivedDelivery>` preserves per-sender FIFO via the underlying `mpsc` (Research §9). The vector also captures cross-sender interleaving in receive order, but tests MUST NOT assert specific cross-sender ordering.

---

## 7. Errors

```rust
// src/error.rs

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file {path:?}: {source}")]
    Io { path: PathBuf, source: std::io::Error },

    #[error("failed to parse TOML config {path:?}: {source}")]
    Parse { path: PathBuf, source: toml::de::Error },

    #[error("invalid peer entry: {0}")]
    InvalidPeer(String),
}

#[derive(Debug, thiserror::Error)]
pub enum NetworkError {
    #[error("peer id {0} is already registered on this network")]
    DuplicateRegistration(PeerId),
}

#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    #[error(transparent)]
    Network(#[from] NetworkError),
}
```

| FR trace | US3 AS-2 (clear, actionable error for malformed config); FR-009 (id uniqueness — duplicate registration is the only network-level failure mode in v1) |

---

## 8. Type / module dependency graph

```text
peer.rs ─────────► PeerId, PeerIdError, PeerDescriptor, BasicPeerDescriptor
   ▲
   │ used by
   ├──── message.rs ──► Message
   ├──── network.rs ──► Envelope (private), Network trait, InMemoryNetwork,
   │                    NetworkHandle, NetworkSender (crate-private)
   ├──── received.rs ─► ReceivedDelivery
   ├──── config.rs ──► PeerEntry, PeerListConfig, load_peer_list
   ├──── error.rs ───► ConfigError, NetworkError, NodeError
   └──── node.rs ───► Node
                       ▲ (Node also uses received.rs, config.rs, network.rs, error.rs)
                       │ used by
                       └── main.rs (binary): parses CLI, loads config, constructs Node, runs

lib.rs re-exports the public surface for consumers.
```

`lib.rs` re-exports:

```rust
pub use peer::{PeerId, PeerIdError, PeerDescriptor, BasicPeerDescriptor};
pub use message::Message;
pub use network::{Network, NetworkHandle, InMemoryNetwork};
pub use received::ReceivedDelivery;
pub use config::{PeerEntry, PeerListConfig, load_peer_list};
pub use node::Node;
pub use error::{ConfigError, NetworkError, NodeError};
```

Error-location policy: `src/error.rs` centralises cross-module errors (`ConfigError`, `NetworkError`, `NodeError`); `src/peer.rs` co-locates `PeerIdError` with `PeerId` itself (parse error next to the parsed type, matching `std::num::ParseIntError` next to integer types). Callers always reach errors via the flat top-level namespace (`pubsub_node::ConfigError`, etc.) — same shape as every other re-exported type.

---

## 9. Cross-reference matrix

| FR | Realised by |
|----|-------------|
| FR-001 | `config::load_peer_list` + `PeerListConfig` (§3) |
| FR-002 | `Network` trait + `InMemoryNetwork` (§4) |
| FR-003 | No admission check on receive path; recv task simply records every envelope (§5 receive-side diagram) |
| FR-004 | `Message::Ping(u64)`; `Node::send` is fire-and-forget; recv task records every Ping (§2, §6) |
| FR-005 | `NetworkHandle::send(to, message)` is one-to-one by signature; sender id is implicit in the handle (§4) |
| FR-006 | `Node::received_messages()` returns a snapshot `Vec<ReceivedDelivery>` whose `from` is the logical peer identity, supplied by the handle's `self_id` at enqueue (§4, §5, §6) |
| FR-007 | No crypto modules; no `signing` / `hashing` deps in `Cargo.toml` |
| FR-008 | `Node` exposes no peer-set mutation API; `peers` field is `Vec<…>` set once at construction (§5) |
| FR-009 | `PeerDescriptor::id() -> &PeerId`; `InMemoryNetwork` enforces uniqueness via the registry hashmap (§1, §4) |
| FR-010 | `NetworkHandle::send` drop-and-warn branch (§4 `send` contract) |
| FR-011 | `NetworkHandle::send`, `Node::new`, `Node::send` all `async fn` (§4, §5) |
| FR-012 | `Node::new(self_id, peer_list, network)` takes parsed `PeerListConfig`, not a path; CLI does parsing (§3, §5) |
| FR-013 | `NetworkHandle::send` returns after enqueue; `recv_task` updates `received` later; tests use `await_delivery` helper (§5, §10 below) |

---

## 10. Test-harness types (under `tests/common/`)

These types are NOT part of the public library surface but are defined here so the contracts and tasks reference a single canonical shape.

```rust
// tests/common/mod.rs
pub struct TwoNodeFixture {
    pub network: Arc<InMemoryNetwork>,
    pub a: Node,
    pub b: Node,
}

pub async fn two_node_fixture() -> TwoNodeFixture { … }

pub async fn await_delivery(
    node: &Node,
    expected_sender: &PeerId,
    expected_message: &Message,
    timeout: Duration,
) -> Result<(), AwaitError>;

#[derive(Debug, thiserror::Error)]
pub enum AwaitError {
    #[error("timed out after {0:?} waiting for delivery")]
    Timeout(Duration),
}
```

| FR trace | FR-013 (delivery-observation primitive required for assertions); SC-004 (helpers documented for contributor reproduction) |
