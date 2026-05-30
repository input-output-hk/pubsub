# Library API Contract — 002 Deltas

**Feature**: 002-topic-subscription-filtering
**Source of truth**: `src/lib.rs` re-exports + the per-module public surface
**Spec trace**: FR-001, FR-002, FR-003, FR-006, FR-007, FR-009, FR-013, FR-014, FR-015 (full matrix in `data-model.md` §7)

This contract documents **only what 002 adds or changes**. The 001 contract at `../001-minimal-node-scaffold/contracts/library-api.md` remains the canonical reference for everything else (`PeerId`, `PeerDescriptor`, `Network`, `NetworkHandle`, `InMemoryNetwork`, `ReceivedDelivery`, error types other than `ConfigError::InvalidTopic`). Items unchanged by 002 are not re-described here.

---

## Re-exports from `pubsub_node` — additions

```rust
// new in 002:
pub use topic::{TopicId, TopicIdError};
pub use message::MessagePayload;              // formerly `Message` enum; renamed
pub use node::{SubscribeOutcome, UnsubscribeOutcome};

// existing re-exports unchanged in surface; `Message` retains the name
// but its shape is now a struct (see "Message envelope" below).
pub use message::Message;
// Renamed in 002 (CHK017): `PeerListConfig` → `NodeConfig`,
// `load_peer_list` → `load_node_config`. `PeerEntry` keeps its name.
pub use config::{PeerEntry, NodeConfig, load_node_config};
pub use error::ConfigError;
```

## `TopicId`

| Item | Contract |
|------|----------|
| `pub fn from_str(s: &str) -> Result<TopicId, TopicIdError>` (via `FromStr`) | Accepts any non-empty UTF-8 string that contains no internal NUL byte. Returns `TopicIdError::Empty` or `TopicIdError::ContainsNul` otherwise. No additional character-class restrictions (no whitespace rule, no length cap). |
| `impl Display` | Prints the inner string verbatim — what appears in `tracing` field values via `%topic`. |
| `impl Hash + Eq + Clone` | Required by the `HashSet<TopicId>` subscription set and by FR-013's snapshot equality semantics. |
| `impl serde::Deserialize` via `#[serde(try_from = "String")]` | The TOML loader path uses this; validation failures become `ConfigError::InvalidTopic` (see below). |

`TopicId` is intentionally minimal — same shape and same rules as `PeerId`. Tightening the character class (namespacing, scoping) is a future decision deferred to the registry feature.

## `TopicIdError`

```rust
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TopicIdError {
    #[error("topic id must not be empty")]
    Empty,
    #[error("topic id must not contain a NUL byte")]
    ContainsNul,
}
```

Mirrors `PeerIdError`. Two closed variants; future failure modes (namespacing rules, registry validation) will be a separate `Result<TopicId, Error>` path layered on top, not new variants here.

## `Message` (envelope) — shape change from 001

```rust
pub struct Message {
    pub topic: TopicId,
    pub payload: MessagePayload,
}
```

The name `Message` is preserved; the **shape** changes from the 001 enum to this struct. Every call site that constructed `Message::Ping(N)` in 001 migrates to `Message { topic, payload: MessagePayload::Ping(N) }`.

| Item | Contract |
|------|----------|
| `topic: TopicId` | First-class field, public access. Required by FR-001. The constructor (literal or convenience helper) MUST supply a `TopicId`; there is no implicit default topic. |
| `payload: MessagePayload` | The previous Message-enum, renamed. See below. |
| `impl Clone + PartialEq + Eq + Debug` | Required by tests asserting on `ReceivedDelivery` contents (delivery contains a `Message`). |

A convenience constructor `Message::ping(topic, n) -> Message` MAY be provided for ergonomics — non-normative; tests are free to use it or build the literal explicitly.

## `MessagePayload`

```rust
#[non_exhaustive]
pub enum MessagePayload {
    Ping(u64),
}
```

Renamed from 001's `Message` enum (same variants, same `#[non_exhaustive]` discipline). Adding a new variant remains non-breaking for `match` arms with a wildcard.

## `SubscribeOutcome` and `UnsubscribeOutcome`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscribeOutcome {
    Added,
    AlreadyPresent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsubscribeOutcome {
    Removed,
    NotSubscribed,
}
```

| Item | Contract |
|------|----------|
| `Copy`-able | Both enums carry no payload; callers may freely match and log without cloning. |
| Closed (no `#[non_exhaustive]`) | Future failure modes wrap the existing enum in a `Result`; no new variants are added in-place. |
| Variant semantics | `Added` / `Removed` indicate the call mutated the subscription set; `AlreadyPresent` / `NotSubscribed` indicate the call was an idempotent no-op. The log level of the emitted tracing event is keyed to this (FR-014). |

## `Node` — additions and constructor change

### New constructor parameter

The 001 constructor `Node::new<N: Network>(self_id: PeerId, peer_list: PeerListConfig, network: Arc<N>) -> Result<Self, NodeError>` is extended with a fourth parameter and the type parameter is renamed per CHK017:

```rust
pub async fn new<N: Network>(
    self_id: PeerId,
    config: NodeConfig,
    initial_subscriptions: HashSet<TopicId>,
    network: Arc<N>,
) -> Result<Self, NodeError>;
```

**Rename note (002 CHK017)**: 001's `peer_list: PeerListConfig` parameter is renamed to `config: NodeConfig` to reflect that the config now carries both peers and topics. The parameter rename is mechanical at all call sites.

**Rationale for parameter order**: the subscription set is the new "what does this Node consume?" input, parallel to `config` (the "what does this Node know about?" input). Both are parsed-at-edge inputs; placing them adjacent makes the layering visible. `network` stays last as in 001 (the shared substrate, dependency-injected).

Alternative considered: derive `initial_subscriptions` from `config.subscribed_topics` inside the constructor — rejected because it couples the Node's API to the TOML schema. The Node API takes a `HashSet<TopicId>` (the in-memory shape); the CLI/loader is what turns the TOML's `Vec<TopicId>` into a `HashSet`. Matches the FR-012 "parse at the edge" layering.

### New methods on `Node`

| Method | Contract |
|--------|----------|
| `pub fn subscribe(&self, topic: TopicId) -> SubscribeOutcome` | Synchronous; takes `&self`. Acquires the internal subscription-set lock, inserts `topic`, returns `Added` if newly inserted or `AlreadyPresent` otherwise. Emits an `info`-level structured `tracing` event on `Added`; emits a `debug`-level event on `AlreadyPresent` (FR-014). Linearizable with respect to `unsubscribe`, the receive-path filter, and `subscriptions()` (FR-015). MUST NOT perform any I/O, registry lookup, or persistence in this iteration (FR-006). |
| `pub fn unsubscribe(&self, topic: TopicId) -> UnsubscribeOutcome` | Symmetric: synchronous; `&self`; acquires the same lock; removes `topic` if present; returns `Removed` if it was present or `NotSubscribed` otherwise. Info event on `Removed`; debug event on `NotSubscribed`. Linearizable per FR-015. No I/O. |
| `pub fn subscriptions(&self) -> Vec<TopicId>` | Snapshot getter. Acquires the lock; clones the subscription set's contents into an owned `Vec<TopicId>`; releases the lock; returns the Vec. Entry order in the returned Vec is **unspecified** — the underlying state is a set; callers asserting against the result MUST treat it as a set (sort or compare as `HashSet`). The returned Vec is unaffected by subsequent mutations on the same Node, per FR-013. |

### Behavior unchanged

`Node::id(&self) -> &PeerId`, `Node::peers(&self) -> &[BasicPeerDescriptor]`, `Node::received_messages(&self) -> Vec<ReceivedDelivery>`, and `Node::send(&self, to: &PeerId, message: Message) -> Result<(), NodeError>` keep their 001 contracts. `Node::send` continues to accept any `Message` regardless of the Node's own subscription set (FR-008 decoupling — emission is independent of subscription).

### Receive-path filter (internal)

The recv_task (spawned by `Node::new`, internal) acquires the subscription-set lock for each inbound delivery, checks `subscriptions.contains(&envelope.message.topic)`, and either pushes the delivery to `received` (if the topic is subscribed) or emits an `info`-level `event = "topic_drop"` structured `tracing` event and discards the delivery (FR-004 + FR-011). The drop has NO observable side effect through any public Node API other than the `tracing` event itself — tests assert on `received_messages()`, never on log content.

## `NodeConfig` — renamed from `PeerListConfig`, new field

Renamed in 002 (CHK017) from 001's `PeerListConfig` to reflect the broader scope: the config now carries both peers and subscribed topics. The struct contents are the 001 layout plus the new `subscribed_topics` field.

```rust
#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct NodeConfig {
    #[serde(default)]
    pub peers: Vec<PeerEntry>,

    #[serde(default)]
    pub subscribed_topics: Vec<TopicId>,
}
```

| Item | Contract |
|------|----------|
| `subscribed_topics: Vec<TopicId>` | Public, owned. Defaults to empty when the TOML field is absent. Each entry has already passed `TopicId::from_str` (the loader applies it during deserialization or in a post-parse pass — see §7 below). |
| `#[serde(deny_unknown_fields)]` | Continues to apply at the top level; adding `subscribed_topics` does NOT relax this contract for any other field. |

## `load_node_config` — renamed from `load_peer_list`, extended behavior

```rust
pub fn load_node_config(path: &Path) -> Result<NodeConfig, ConfigError>;
```

Renamed in 002 (CHK017) alongside the `NodeConfig` type rename. The signature shape (one `&Path` argument; `Result<NodeConfig, ConfigError>` return) is unchanged from 001's `load_peer_list`; only the names change. Behavior is extended per the pipeline below.

Loader contract (additive to 001):

1. Read the file at `path`. I/O failure → `ConfigError::Io { path, source }` (unchanged).
2. Parse contents as TOML via a shadow `RawNodeConfig { peers: …, subscribed_topics: Vec<String> }`. Structural failure → `ConfigError::Parse { path, source }` (unchanged).
3. Validate each `peers` entry's id via `PeerId::from_str` → `ConfigError::InvalidPeer(message)` (unchanged).
4. **NEW**: validate each `subscribed_topics` entry via `TopicId::from_str` → `ConfigError::InvalidTopic(message)`. The message has the same shape as `InvalidPeer`: `"{path}: {topic_id_error}"`.

Steps 3 and 4 are fail-fast: the loader returns the first `Err` encountered when iterating either field. Multi-error reporting is out of scope for v1 (matches 001 precedent).

## `ConfigError` — new variant

```rust
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    // existing variants from 001…
    Io { path: PathBuf, source: std::io::Error },
    Parse { path: PathBuf, source: toml::de::Error },
    InvalidPeer(String),

    // new in 002:
    #[error("config invalid topic entry: {0}")]
    InvalidTopic(String),
}
```

Format string mirrors `InvalidPeer`. The variant is the **only** new error introduced by 002; no new `NetworkError` or `NodeError` variants are added (no new failure modes at the network or node API boundaries).

## `tracing` events emitted by 002

All events use the existing 001 logging facility (`tracing` crate). Target string `pubsub_node::node` matches 001 (`src/node.rs:51`). Field shape is documented here as the operator-facing contract:

| Event marker | Level | Trigger | Target | Fields |
|--------------|-------|---------|--------|--------|
| `topic_drop` | info | Receive task observes a delivery whose topic is not in the subscription set (FR-004 + FR-011) | `pubsub_node::node` | `event`, `self_id`, `from`, `topic` |
| `topic_subscribed` | info | `subscribe(T)` returns `SubscribeOutcome::Added` (FR-014) | `pubsub_node::node` | `event`, `self_id`, `topic` |
| `topic_unsubscribed` | info | `unsubscribe(T)` returns `UnsubscribeOutcome::Removed` (FR-014) | `pubsub_node::node` | `event`, `self_id`, `topic` |
| `topic_subscribe_noop` | debug | `subscribe(T)` returns `SubscribeOutcome::AlreadyPresent` (FR-014) | `pubsub_node::node` | `event`, `self_id`, `topic`, `reason="already_present"` |
| `topic_unsubscribe_noop` | debug | `unsubscribe(T)` returns `UnsubscribeOutcome::NotSubscribed` (FR-014) | `pubsub_node::node` | `event`, `self_id`, `topic`, `reason="not_subscribed"` |
| `topic_config_duplicate` | warn | TOML loader detected a duplicate entry in `subscribed_topics` (FR-010, one event per duplicated topic per load call) | `pubsub_node::config` | `event`, `topic`, `config_path` (no `self_id` — emitted before Node exists; CLI invocation supplies the operator's context) |

Field formats:

- `event`: a literal string, the event marker from the table above. Operators grep on this.
- `self_id`: the emitting Node's own `PeerId` via `Display` (`%self_id` in the macro). Absent on `topic_config_duplicate` (emitted by the loader before any Node exists).
- `from`: the sender's `PeerId` (drop event only) via `Display`.
- `topic`: the message's `TopicId` (drop event), the operated topic (mutation events), or the duplicated topic (config-duplicate event) via `Display`.
- `reason`: a literal short string distinguishing the no-op cause (no-op events only).
- `config_path`: the loader's config file path via `Path::display()` (config-duplicate event only). Disambiguates per-process when an operator runs multiple binaries.

No payload (`MessagePayload::Ping(n)`'s `n` value, etc.) is included in any of the events. Operator-facing strings carry no FR identifiers.

At 001's default `--log-level info`, the info events (`topic_drop`, `topic_subscribed`, `topic_unsubscribed`) and the warn event (`topic_config_duplicate`) are operator-visible without explicit configuration; the two debug events (`topic_subscribe_noop`, `topic_unsubscribe_noop`) are invisible at the default and require `--log-level debug`.

## What 002 does NOT add to the API

Pinned here for clarity (avoids future drift):

- No new method on `Node` for "publishing on a topic" — emission stays through `Node::send`, with the topic carried on the Message (FR-007).
- No method on `Node` for "is this Node subscribed to T?" — callers may use `node.subscriptions().contains(&t)` if they need a boolean predicate. A `Node::is_subscribed(&self, &TopicId) -> bool` helper MAY be added later if it proves ergonomically warranted, but it is **not** part of the 002 contract.
- No new method on `Network`, `NetworkHandle`, or `InMemoryNetwork` — the substrate is unchanged (FR-005).
- No new CLI flag — `subscribed_topics` rides inside the existing config file; the CLI surface from 001 is unchanged.
- No new `NodeError` or `NetworkError` variant — no new failure modes at those API boundaries.
- No `Subscriptions` opaque type — the snapshot is a raw `Vec<TopicId>`. A future iteration may wrap this in an opaque type if richer metadata is needed (recorded in `research.md` §9).
