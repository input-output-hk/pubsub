# Data Model: Topics + Topic-Subscription Filtering

**Feature**: 002-topic-subscription-filtering

**Created**: 2026-05-30

**Purpose**: enumerate the entities introduced or extended by 002, with fields, validation rules, state-transition semantics, relationships, and an FR cross-reference. Entities unchanged by 002 (PeerId, PeerDescriptor, BasicPeerDescriptor, Network, NetworkHandle, NodeError, NetworkError, ReceivedDelivery's containing-Vec semantics) are not duplicated here — the canonical reference is `../001-minimal-node-scaffold/data-model.md`.

---

## 1. `TopicId` — new entity

**Source**: `src/topic.rs` (new file, parallel to `src/peer.rs`).

**Definition**: opaque newtype wrapper around an owned UTF-8 `String`.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize)]
#[serde(try_from = "String")]
pub struct TopicId(String);
```

**Fields**: a single private `String`. No additional fields at this stage.

**Validation rules** (FR-002):

- The string MUST be non-empty.
- The string MUST NOT contain any internal NUL byte (`'\0'`).
- No additional character-class restrictions (no whitespace rule, no length cap, no namespacing structure). Resolved by `/speckit-clarify` Q2 round 1.

**Construction surface**:

- `TopicId::from_str(s) -> Result<TopicId, TopicIdError>` — the canonical construction path. `TopicIdError` is the parallel of `PeerIdError` (see 001's `src/peer.rs`).
- `impl FromStr for TopicId` — same body as `from_str`.
- `impl TryFrom<String> for TopicId` — used by `serde` via the `#[serde(try_from = "String")]` attribute so the same validation pipeline applies during TOML deserialization.

**Accessors**:

- `pub fn as_str(&self) -> &str` — mirrors `PeerId::as_str` (`src/peer.rs:33`).
- `impl Display for TopicId` — emits the inner string verbatim; used by `tracing` field formatters via `%topic`.

**Error type**:

```rust
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TopicIdError {
    #[error("topic id must not be empty")]
    Empty,
    #[error("topic id must not contain a NUL byte")]
    ContainsNul,
}
```

Mirrors `PeerIdError` exactly. Two variants; no future variants planned for v1.

**FR traces**: FR-001, FR-002, FR-010.

## 2. `Message` (envelope) — replaces 001's enum-as-Message shape

**Source**: `src/message.rs` (extended).

**Definition**: a struct wrapping a `TopicId` with a payload variant. Reuses the public type name `Message`; the previous enum is renamed to `MessagePayload`. Resolved in `research.md §1`.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub topic: TopicId,
    pub payload: MessagePayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessagePayload {
    Ping(u64),
    // future variants land here without disturbing the envelope
}
```

**Fields**:

- `topic: TopicId` — the first-class topic dimension required by FR-001.
- `payload: MessagePayload` — the existing variant enum, renamed (was `Message` in 001; `Message::Ping(N)` becomes `MessagePayload::Ping(N)`).

**Construction**: callers explicitly build `Message { topic: topic_id, payload: MessagePayload::Ping(n) }`. A convenience constructor (`Message::ping(topic, n)`) MAY be added in `src/message.rs` as a small ergonomic affordance — non-normative.

**Migration note**: every existing 001 call site that constructs `Message::Ping(N)` is updated mechanically. Test files (`tests/two_node_ping.rs`, `tests/n_node_graph.rs`) and the common test helpers (`tests/common/mod.rs`) are the only call sites in 001. The rename is a single early task in `/speckit-tasks`, completed before any 002-specific test is added so the green-checkpoint invariant holds.

**FR traces**: FR-001 (`topic` field is normative), FR-007 (send signature unchanged; whoever constructs the Message picks the topic), FR-008 (the topic on a Message does not need to match the sender's subscription set), FR-009 (forwarded copies are receipts).

## 3. Subscription Set — new Node-internal state

**Source**: a private field on `Node` in `src/node.rs`.

**Definition**: a thread-safe, mutable `HashSet<TopicId>` accessed through interior mutability.

```rust
pub struct Node {
    // existing fields preserved from 001…
    handle: NetworkHandle,
    peers: Vec<BasicPeerDescriptor>,
    received: Arc<Mutex<Vec<ReceivedDelivery>>>,
    recv_task: JoinHandle<()>,

    // new in 002:
    subscriptions: Arc<Mutex<HashSet<TopicId>>>,
}
```

**Initial value**: at Node construction, the constructor receives an in-memory `HashSet<TopicId>` (the parsed initial set, supplied by the loader / CLI). The Node clones that into the `Arc<Mutex<…>>`; the recv_task is spawned with an additional `Arc::clone` so it shares the same lock-protected state.

**Mutation API** (FR-006):

- `fn subscribe(&self, topic: TopicId) -> SubscribeOutcome` — acquire lock; `HashSet::insert(topic)`; if it returned `true` (the value was newly added), emit FR-014 info log and return `SubscribeOutcome::Added`; if it returned `false`, emit FR-014 debug log and return `SubscribeOutcome::AlreadyPresent`.
- `fn unsubscribe(&self, topic: TopicId) -> UnsubscribeOutcome` — acquire lock; `HashSet::remove(&topic)`; symmetric outcome handling and logging.

**Read API** (FR-013):

- `fn subscriptions(&self) -> Vec<TopicId>` — acquire lock; clone the HashSet's contents into a `Vec<TopicId>`; release lock; return. Entry order in the returned Vec is unspecified.

**Receive-path read** (FR-004): the recv_task acquires the same lock per inbound delivery and runs `HashSet::contains(&envelope.message.topic)`. The lock is held only during the membership check. If the contains returns true, the message is appended to `received` (taking that lock as well — the order is `subscriptions` first, then `received`, to maintain a stable lock-acquisition order project-wide). If false, the recv_task emits the FR-011 info log and skips the push.

**Lock-acquisition order**: throughout the codebase, when both `subscriptions` and `received` must be held, acquire `subscriptions` first, `received` second. This is the convention to prevent deadlock; only the recv_task ever holds both (and only briefly). External callers of `subscribe`/`unsubscribe` never touch `received`; external callers of `received_messages()` never touch `subscriptions`. No deadlock risk in practice; the convention is documented for the discipline.

**State-transition semantics**:

```text
subscribe(T) on a Node with subscriptions = S:
  if T in S: emit debug log; return AlreadyPresent; S unchanged.
  if T not in S: S becomes S ∪ {T}; emit info log; return Added.

unsubscribe(T) on a Node with subscriptions = S:
  if T not in S: emit debug log; return NotSubscribed; S unchanged.
  if T in S: S becomes S \ {T}; emit info log; return Removed.

recv-path observation on a Node with subscriptions = S, inbound message m:
  if m.topic in S: push delivery to received; ack to recv_task continues.
  if m.topic not in S: emit info log (topic_drop); do not push.
```

All three are atomic with respect to each other under FR-015's linearizability contract: the lock serializes execution; any subsequent operation after a mutator returns sees the post-mutation state.

**Snapshot semantics** (FR-013): `subscriptions()`'s returned Vec is unaffected by subsequent mutations. Implementation: the Vec is constructed by cloning the HashSet's contents under the lock, then released; the caller owns the data; subsequent mutations modify the live HashSet but the returned Vec is decoupled.

**FR traces**: FR-003, FR-004, FR-006, FR-013, FR-015.

## 4. `SubscribeOutcome` / `UnsubscribeOutcome` — new enums

**Source**: `src/node.rs` (alongside the Node type, since they are the return types of Node methods). May migrate to a separate `src/outcomes.rs` if a third Outcome arrives in a future feature — not anticipated at v1.

**Definitions**:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscribeOutcome {
    /// The topic was not previously in the subscription set; the call added it.
    Added,
    /// The topic was already in the subscription set; the call is an idempotent no-op.
    AlreadyPresent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsubscribeOutcome {
    /// The topic was in the subscription set; the call removed it.
    Removed,
    /// The topic was not in the subscription set; the call is an idempotent no-op.
    NotSubscribed,
}
```

**Trait derivations**:

- `Debug`: required for test assertions (`assert_eq!`).
- `Clone, Copy`: zero-cost since variants carry no data; lets callers log + match without cloning.
- `PartialEq, Eq`: required for assertions and for the operator's log-line emission decision (Added → info; AlreadyPresent → debug — see §6 below).

**No `#[non_exhaustive]`**: explicitly closed. Future failure modes (e.g., registry-unknown) wrap these enums in a `Result`; they do not add new variants. Recorded in `research.md` §5 and §8.

**FR traces**: FR-006, FR-014.

## 5. `PeerListConfig` extension — new field

**Source**: `src/config.rs` (extended).

**Updated definition**:

```rust
#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct PeerListConfig {
    #[serde(default)]
    pub peers: Vec<PeerEntry>,

    /// Topics the local node subscribes to at construction.
    /// Empty or absent yields an empty initial subscription set.
    /// Each entry is validated via `TopicId::from_str`; a failure
    /// surfaces as `ConfigError::InvalidTopic`.
    #[serde(default)]
    pub subscribed_topics: Vec<TopicId>,
}
```

**Field semantics**:

- `subscribed_topics: Vec<TopicId>` — owned, parsed, deduped-by-the-loader (or by `HashSet` construction at Node construction time; either is acceptable since the on-disk shape is a `Vec` and the in-memory shape downstream is a `HashSet`).
- `#[serde(default)]` ensures absent fields yield empty Vec, matching 001's pattern for `peers`.
- `#[serde(deny_unknown_fields)]` continues to apply at the top level. The new field does not relax that contract.

**Loader behavior** (mirrors 001's `load_peer_list`):

```text
load_peer_list(path):
  1. Read file at path → ConfigError::Io on failure.
  2. Parse TOML via the shadow type RawPeerListConfig (string fields, no
     TopicId::FromStr applied yet) → ConfigError::Parse on syntactic failure.
  3. For each `peers` entry: PeerId::from_str → ConfigError::InvalidPeer on
     rule violation.
  4. For each `subscribed_topics` entry: TopicId::from_str →
     ConfigError::InvalidTopic on rule violation.
  5. Return PeerListConfig with both fields populated.
```

Order of validation in step 3 vs step 4 is implementation-internal; per 001 precedent, the iterator collects on first `Err` (fail-fast). Either field's first invalid entry surfaces immediately; subsequent entries are not validated in the same call. This matches the user's mental model from 001 and avoids confusing multi-error reporting at this iteration.

**FR traces**: FR-010, FR-012.

## 6. `ConfigError::InvalidTopic` — new error variant

**Source**: `src/error.rs` (extended).

**Updated definition** (additions):

```rust
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    // existing variants from 001…
    #[error("config io error: {path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },

    #[error("config parse error: {path}: {source}")]
    Parse { path: PathBuf, source: toml::de::Error },

    #[error("config invalid peer entry: {0}")]
    InvalidPeer(String),

    // new in 002:
    #[error("config invalid topic entry: {0}")]
    InvalidTopic(String),
}
```

**Format string**: deliberately mirrors `InvalidPeer` — same `Display` shape, same `String` field carrying `"{path}: {error}"` (constructed by the loader). Operator-facing string discipline applies: no FR-reference in the message text.

**FR traces**: FR-010.

## 7. Cross-reference matrix (FR → entity/file)

| FR | Entity / file touched | Notes |
|----|------------------------|-------|
| FR-001 | §2 Message envelope; `src/message.rs` | topic is a struct field on `Message` |
| FR-002 | §1 TopicId; `src/topic.rs` | newtype + FromStr + validation rules |
| FR-003 | §3 Subscription Set; `src/node.rs` | HashSet under Arc<Mutex<…>> |
| FR-004 | §3 recv-path read; `src/node.rs` recv_task body | membership check + log on drop |
| FR-005 | `src/network.rs` | **unchanged** by design; FR-005 is enforced by the file diff being empty |
| FR-006 | §4 Outcome enums + §3 mutation API; `src/node.rs` | sync &self methods, infallible |
| FR-007 | §2 Message envelope construction; existing `Node::send` | call sites update from `Message::Ping(N)` to `Message { topic, payload: MessagePayload::Ping(N) }` |
| FR-008 | §3 subscription state vs §2 emission | no coupling in code; the test for US3 demonstrates emission on unsubscribed topic |
| FR-009 | §3 recv-path read; `src/node.rs` recv_task body | only network-delivered messages enter the snapshot |
| FR-010 | §5 PeerListConfig + §6 ConfigError::InvalidTopic | top-level subscribed_topics + loader path |
| FR-011 | `src/node.rs` recv_task body | info log on drop; field shape per `research.md` §7 |
| FR-012 | `src/main.rs`, `src/config.rs`, `src/node.rs` | parsing in CLI/loader; Node constructor takes parsed HashSet<TopicId> |
| FR-013 | §3 Subscription Set read API; `src/node.rs` | snapshot Vec<TopicId> via clone-under-lock |
| FR-014 | `src/node.rs` mutators | tracing events; field shape per `research.md` §7 |
| FR-015 | §3 Subscription Set lock-serialized access; `src/node.rs` | linearizability contract; primitive choice in `research.md` §2 |

## 8. State-transition diagram (subscription set, single Node)

```text
                       subscribe(T) → Added
        ┌──────────────────────────────────────────────────┐
        │                                                  │
        │                                                  ▼
   ┌─────────────────┐                            ┌─────────────────┐
   │  T ∉ subs(N)    │                            │  T ∈ subs(N)    │
   │                 │                            │                 │
   │  drops T on rcv │                            │  retains T on   │
   │                 │                            │  rcv            │
   │                 │                            │                 │
   └─────────────────┘                            └─────────────────┘
        ▲                                                  │
        │                                                  │
        └──────────────────────────────────────────────────┘
                       unsubscribe(T) → Removed

   self-loop on left state:  subscribe(T) on T ∉ subs is the only
   transition out;  unsubscribe(T) on T ∉ subs returns NotSubscribed
   without state change.

   self-loop on right state: unsubscribe(T) on T ∈ subs is the only
   transition out;  subscribe(T) on T ∈ subs returns AlreadyPresent
   without state change.
```

Each transition is atomic under FR-015's linearizability. The receive-path read is a *query* on this state, not a transition.

## 9. Test fixture impact

`tests/common/mod.rs` (fixture builders):

- The current `two_node_fixture` / similar helpers construct Nodes with a peer set only. Add an additional argument (or builder method) for the initial subscription set. Default: empty HashSet, so 001's existing tests that don't set subscriptions keep compiling.
- A new helper `assert_subscriptions(node, &[…])` MAY be added to encapsulate the "snapshot, sort, assert" idiom for tests that compare subscription sets — non-normative, ergonomic only.

`tests/two_node_ping.rs` (001 US1):

- Adopt the new `Message { topic, payload }` envelope shape. Tests use a placeholder topic (e.g., `TopicId::from_str("test").unwrap()`) and the fixtures subscribe both nodes to that topic so the existing 001 acceptance scenarios continue to pass with the topic dimension trivially satisfied. The fixture default of "subscribe to whatever topic the test fixture uses" makes 001's tests invariant under 002.

`tests/n_node_graph.rs` (001 US2 + 002 US2 / SC-002):

- Extend with 002 US2 acceptance scenarios: 4-node graph with mixed subscriptions; at least 100 emissions across at least 3 topics; per-node snapshot assertions (set equality after sort or as HashSet).

`tests/topic_filter.rs` (new):

- 002 US1 acceptance scenarios: single-topic filter; on-topic retention; off-topic silent drop with info log entry observed via `tracing-test` or equivalent capture mechanism (concrete capture choice deferred to `/speckit-tasks`).

`tests/topic_runtime.rs` (new):

- 002 US3 acceptance scenarios: dynamic transitions (subscribe → message-after → unsubscribe → message-after); idempotent outcomes; snapshot monotonicity.

`tests/config_loading.rs` (001 US3 + 002 US4):

- Extend with 002 US4: TOML containing `subscribed_topics`, absent `subscribed_topics`, empty `subscribed_topics`, invalid topic entry, unknown top-level field. Mirrors the malformed-config path 001 already tests.
