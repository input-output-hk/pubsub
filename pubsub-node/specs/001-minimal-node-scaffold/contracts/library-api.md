# Library API Contract — `pubsub_node` crate

**Feature**: 001-minimal-node-scaffold
**Source of truth**: `src/lib.rs` re-exports
**Spec trace**: All FRs (full coverage matrix in `data-model.md` §9)

This contract is the stable surface that integration tests and the binary depend on. Internal types (`Envelope`, channel internals, the receive-task `JoinHandle`) are deliberately excluded.

---

## Re-exports from `pubsub_node`

```rust
pub use peer::{PeerId, PeerIdError, PeerDescriptor, BasicPeerDescriptor};
pub use message::Message;
pub use network::{Network, NetworkHandle, InMemoryNetwork};
pub use received::ReceivedDelivery;
pub use config::{PeerEntry, PeerListConfig, load_peer_list};
pub use node::Node;
pub use error::{ConfigError, NetworkError, NodeError};
```

---

## `PeerId`

| Item | Contract |
|------|----------|
| `pub fn from_str(s: &str) -> Result<PeerId, PeerIdError>` (via `FromStr`) | Accepts any non-empty UTF-8 string that contains no internal NUL bytes. Returns `PeerIdError::Empty` or `PeerIdError::ContainsNul` otherwise. |
| `impl Display` | Prints the inner string verbatim — this is what appears in `tracing` fields and in the CLI's error output. |
| `impl Hash + Eq + Clone` | Required by the `InMemoryNetwork` registry hashmap. |

`PeerId` is intentionally minimal: it does not parse host:port, it does not enforce a character class beyond the rules above. Tightening the class is a future decision (and an ADR).

## `PeerDescriptor` trait

```rust
pub trait PeerDescriptor: Clone + Send + Sync + 'static {
    fn id(&self) -> &PeerId;
}
```

Implementors MUST guarantee that the `PeerId` returned by `id()` is stable for the lifetime of the descriptor (no interior mutability that swaps it).

## `BasicPeerDescriptor`

```rust
pub struct BasicPeerDescriptor { pub id: PeerId }
```

The v1 concrete impl. Public field for ergonomic construction in tests; consumers should still address through `descriptor.id()` so future descriptor types can drop in.

## `Message`

```rust
pub enum Message { Ping(u64) }
```

Adding a new variant is a non-breaking change for consumers that `match` non-exhaustively; consumers that match exhaustively will need to add an arm. The crate marks `Message` with `#[non_exhaustive]` so external consumers are forced to plan for future variants.

## `Network` trait

```rust
pub trait Network: Send + Sync + 'static {
    async fn register(&self, id: PeerId) -> Result<NetworkHandle, NetworkError>;
}
```

`register` contract:
- On success: returns a `NetworkHandle` carrying `id` plus the receiver-side of an unbounded `mpsc` for incoming envelopes. The handle is how the registered peer both *sends* (via `handle.send(to, msg)`) and *receives* (the Node's recv task drains the handle's receiver — see `NetworkHandle` below).
- On `NetworkError::DuplicateRegistration(id)`: no mutation; the caller MAY retry with a different id.
- `register` MUST be safe to call concurrently from multiple async tasks.

The trait deliberately exposes no `send` method — sends are issued through the per-peer handle, whose sender identity is implicit. This matches future networked transports (per-connection handle; sender id derived from the registered peer, not asserted by callers) and avoids the v1 footgun of asking every send-site to pass its own id.

## `NetworkHandle`

```rust
pub struct NetworkHandle { /* private */ }

impl NetworkHandle {
    pub fn id(&self) -> &PeerId;
    pub async fn send(&self, to: &PeerId, message: Message) -> Result<(), NetworkError>;
    // crate-internal: receiver-side hook used by Node's recv task
}
```

Design pattern: `NetworkHandle` is an actor-handle in the style of [Alice Ryhl's "Actors with Tokio"](https://ryhl.io/blog/actors-with-tokio/), as productionised in [Lighthouse's `NetworkSenders`](https://github.com/sigp/lighthouse/blob/stable/beacon_node/network/src/service.rs) and [Substrate's `sc_network::NetworkService`](https://paritytech.github.io/polkadot-sdk/master/sc_network/index.html). Internally it bundles a cloneable send-half into the network's dispatch fabric and a single-consumer receiver for the per-peer mailbox; externally only `id()` and `send()` are exposed. The receive path surfaces to clients exclusively via `Node::received_messages()` (FR-006, snapshot semantics). The handle itself is **not** `Clone` — single-consumer recv discipline. Full rationale and alternatives considered: `research.md` §12 (ADR slot 0007).

`send` contract (the FR-013 contract):
- Resolves once the network has accepted the message for delivery (enqueued onto the recipient's mailbox in the InMemory impl).
- Returns `Ok(())` even when `to` is unregistered — the message is dropped and a `WARN` `tracing` event is emitted with the unregistered id as a structured field (FR-010).
- Does NOT block on the recipient consuming the message. Tests asserting observability MUST use `await_delivery` (see test-harness contract below).
- Sender attribution: the recipient's record will show `from = self.id()` — the handle supplies this value automatically. This is FR-006's logical-peer-identity requirement (the recorded `from` is the value `PeerDescriptor::id()` returns for the originating peer) realised at delivery time in v1. Callers neither pass `from` nor can override it. This matches future networked transports where the per-connection handle holds the sender identity, not the caller.

## `InMemoryNetwork`

```rust
pub struct InMemoryNetwork { /* private */ }
impl InMemoryNetwork {
    pub fn new() -> Self;
}
impl Network for InMemoryNetwork { … }
```

Sharing pattern: `let net = Arc::new(InMemoryNetwork::new()); Node::new(id, cfg, net.clone()).await?`.

## `Node`

```rust
pub struct Node { /* private */ }

impl Node {
    pub async fn new(
        self_id: PeerId,
        peer_list: PeerListConfig,
        network: Arc<dyn Network>,
    ) -> Result<Node, NodeError>;

    pub async fn send(&self, to: &PeerId, message: Message) -> Result<(), NodeError>;
    pub fn id(&self) -> &PeerId;
    pub fn peers(&self) -> &[BasicPeerDescriptor];
    pub fn received_messages(&self) -> Vec<ReceivedDelivery>;
}
```

Construction:
- `new` registers on the network and spawns the receive task **before returning**. Once the future resolves, the Node is fully ready to send and receive.
- A `Drop` impl aborts the receive task. Tests that need explicit shutdown should drop the `Node` and `.await` an explicit channel-close confirmation if needed (a small helper may live in `tests/common/`).

`send` semantics:
- Forwards to `self.handle.send(to, message).await` — `handle` (returned by `Network::register`) carries the sender id implicitly. The Node never asserts its own id at send time.
- A Node with an empty `peers` slice can still call `send` — the spec's empty-peer-set Edge Case is enforced by the *caller* having no peer to address to. The Node itself does not gate sends by peer-set membership; the network does the routing.

`received_messages()`:
- Returns a `Vec<ReceivedDelivery>` snapshot in receive order (cf. `data-model.md` §6 and Research §9 for ordering semantics).
- This is the **normative observability surface** (FR-006). Tests assert against it, not against logs.
- Returned values are clones — internal state is preserved.

## `PeerListConfig` and `load_peer_list`

```rust
pub fn load_peer_list(path: &Path) -> Result<PeerListConfig, ConfigError>;
```

| Behaviour | Detail |
|-----------|--------|
| Reads `path` | Returns `ConfigError::Io { path, source }` on read failure. |
| Parses TOML | Returns `ConfigError::Parse { path, source }` on syntactic failure (US3 AS-2). |
| Validates entries | Each `PeerEntry` must have a non-empty `id`; returns `ConfigError::InvalidPeer(reason)` otherwise. |
| Idempotent | Pure function; no global state. Safe to call multiple times in tests. |

## Errors

All error types implement `std::error::Error` (via `thiserror`) with sources preserved. The CLI walks the source chain to render the error report described in `cli.md`.

---

## Test-harness contract (under `tests/common/`)

```rust
pub struct TwoNodeFixture {
    pub network: Arc<InMemoryNetwork>,
    pub a: Node,
    pub b: Node,
}

pub async fn two_node_fixture() -> TwoNodeFixture;

pub async fn await_delivery(
    node: &Node,
    expected_sender: &PeerId,
    expected_message: &Message,
    timeout: Duration,
) -> Result<(), AwaitError>;

pub enum AwaitError { Timeout(Duration) }
```

`await_delivery` contract:
- Polls `node.received_messages()` on a fixed short interval (1 ms) until a delivery matching both `expected_sender` and `expected_message` appears.
- Returns `Ok(())` immediately on match.
- Returns `Err(AwaitError::Timeout(timeout))` if the budget is exhausted with no match.
- `timeout` is mandatory; the helper provides no implicit wall-clock default (Engineering Standards "Reproducible tests").
- Recommended default in integration tests: 1 second (orders of magnitude headroom for an in-memory hashmap network).

---

## Versioning of this contract

The library is pre-1.0; breaking changes are expected as the project moves past the scaffold. Specifically:
- Adding `Message` variants is non-breaking (`#[non_exhaustive]`).
- Replacing `PeerListConfig`'s TOML schema is a structural change → ADR + version bump.
- Replacing the `NetworkHandle::send(to, message)` shape — or moving sends back onto the `Network` trait — is structural → ADR + version bump.
- Adding new methods to `Node` or `Network` is non-breaking when defaulted; trait method additions without defaults are breaking.
