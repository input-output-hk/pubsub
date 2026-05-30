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

## 5. `NodeConfig` extension — new field

**Source**: `src/config.rs` (extended; type renamed in 002 from 001's `PeerListConfig` per CHK017 resolution to reflect the broader scope now that it carries both peers and subscribed topics).

**Updated definition**:

```rust
#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct NodeConfig {
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

**Loader behavior** (mirrors the pipeline 001's `load_peer_list` established; renamed in 002 to `load_node_config` alongside the type rename):

```text
load_node_config(path):
  1. Read file at path → ConfigError::Io on failure.
  2. Parse TOML via the shadow type RawNodeConfig (string fields, no
     TopicId::FromStr applied yet) → ConfigError::Parse on syntactic failure.
  3. For each `peers` entry: PeerId::from_str → ConfigError::InvalidPeer on
     rule violation.
  4. For each `subscribed_topics` entry: TopicId::from_str →
     ConfigError::InvalidTopic on rule violation. After validation, scan the
     validated TopicIds for duplicates; for each duplicated TopicId, emit a
     warn-level tracing event (`event=topic_config_duplicate`, fields
     `topic`, `config_path`) per FR-010. Duplicates are NOT a startup
     failure; they are silently absorbed when the downstream Node
     construction converts the Vec<TopicId> into HashSet<TopicId>.
  5. Return NodeConfig with both fields populated (subscribed_topics
     retains the original Vec shape, including any duplicates the operator
     wrote; deduplication is the consumer's concern at the HashSet boundary).
```

Order of validation in step 3 vs step 4 is implementation-internal; per 001 precedent, the iterator collects on first `Err` (fail-fast). Either field's first invalid entry surfaces immediately; subsequent entries are not validated in the same call. This matches the user's mental model from 001 and avoids confusing multi-error reporting at this iteration. The duplicate-scan sub-step in step 4 runs **after** all `TopicId::from_str` calls succeed, so it never fires alongside an `InvalidTopic` error (failed loads emit no duplicate warnings).

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
| FR-010 | §5 NodeConfig + §6 ConfigError::InvalidTopic | top-level subscribed_topics + loader path; duplicate-warn event sub-step |
| FR-011 | `src/node.rs` recv_task body | info log on drop; field shape per `research.md` §7 |
| FR-012 | `src/main.rs`, `src/config.rs`, `src/node.rs` | parsing in CLI/loader; Node constructor takes parsed HashSet<TopicId> |
| FR-013 | §3 Subscription Set read API; `src/node.rs` | snapshot Vec<TopicId> via clone-under-lock |
| FR-014 | `src/node.rs` mutators | tracing events; field shape per `research.md` §7 |
| FR-015 | §3 Subscription Set lock-serialized access; `src/node.rs` | linearizability contract; primitive choice in `research.md` §2 |

## 7.5. Cross-reference matrix (FR → user story / acceptance scenario)

Complement to §7's FR → entity / file matrix. This table maps each FR to the user-story scenarios and success criteria that exercise it, so reviewers can confirm coverage at the behavioral level. "Not test-anchored" rows are deliberate: logs and other operator observability are documented in `quickstart.md` (the operator's reference) but not asserted in automated tests — test discipline anchors on `received_messages()`, `subscriptions()`, and `Outcome` enum returns, never on log content.

| FR | Test-anchored coverage (US / AS / SC) | Operator-observable coverage (quickstart §) | Notes |
|----|---------------------------------------|---------------------------------------------|-------|
| FR-001 (Message has topic) | US1 AS-1/2/3; US2 AS-1/2/3; US3 AS-1–7; US4 AS-1 | quickstart §§2–6 (every example constructs `Message { topic, payload }`) | Pervasive — every scenario constructs Messages |
| FR-002 (TopicId validation) | US4 AS-4 (invalid topic entry fails startup) | quickstart §5 (operator sees `InvalidTopic` error + exit code 2) | |
| FR-003 (Node tracks subscription set) | US1 AS-1/2 (subscribed vs not); US2 AS-1 (per-node subsets); US3 AS-1/3/5 (set transitions) | quickstart §§2–4 | |
| FR-004 (receive-path filter) | US1 AS-1/2 (retain on-topic / drop off-topic); US2 AS-1/3 (per-node filter); US3 AS-1/3/5 (transitions take effect) | quickstart §§2–3 | |
| FR-005 (Network unchanged) | All US — coverage by absence (no scenario asserts new network behavior; the file diff for `src/network.rs` is empty post-002 per plan.md project structure) | quickstart §8 (mental map shows `network.rs # 001 — unchanged`) | Enforced by absence; no positive AS needed |
| FR-006 (subscribe/unsubscribe API + Outcome enums) | US3 AS-2/4/6/7 (Added/Removed/AlreadyPresent/NotSubscribed); SC-005 (idempotency) | quickstart §4 (test list) | |
| FR-007 (send API unchanged) | All US (every send call uses 001's `Node::send(to, message).await` signature) | quickstart §2 | Inherited from 001 |
| FR-008 (subscribe/emit decoupled) | US3 Independent Test (Node A emits on T1 while subscribed to T2 only) | quickstart §4 | The Independent Test exercises this; no separate AS pins it |
| FR-009 (no self-receipt of own emission) | US1 AS-3 (Node emits, does NOT see own message in its snapshot) | quickstart §2 (test `own_emission_not_in_local_snapshot`) | Self-addressing edge case via loopback is operator-observable, not AS-pinned |
| FR-010 (TOML `subscribed_topics` + duplicate-warn) | US4 AS-1/2/3/4/5/6 (present/absent/empty/invalid/unknown-field/duplicate) | quickstart §§5–6 (operator sees warn on duplicate, error on invalid, success on absent) | AS-6 added 2026-05-30 covers the deduplicated-state behavior; the warn log itself is operator UX |
| FR-011 (drop log) | SC-006 (info-level entry visible at default log level) | quickstart §§2, 7 (operator sees `event=topic_drop` lines) | Visibility is asserted via SC-006; log *content* is operator UX, not AS-pinned |
| FR-012 (parse at the edge) | US4 AS-1–6 (operator uses TOML+CLI flow); construction-from-parsed-values is exercised by every test fixture in `tests/common/mod.rs` | quickstart §§5–6 | |
| FR-013 (`subscriptions()` snapshot getter) | US3 AS-1/2/3/4/5/6/7 (every AS reads "A's subscription set is `{…}`" — satisfied by `subscriptions()`); US4 AS-1/3/6 (snapshot equals TOML-loaded set) | quickstart §4 | |
| FR-014 (subscribe/unsubscribe logs) | **Not test-anchored** — log emission is operator UX, not an AS-level assertion. The mutation behavior is anchored via `Outcome` returns (US3 AS-2/4/6/7) and idempotency via SC-005. | quickstart §7 (operator sees `topic_subscribed` / `topic_unsubscribed` / `topic_subscribe_noop` / `topic_unsubscribe_noop` events) | Intentional — logs are observability, tests assert on the API return values |
| FR-015 (linearizable concurrency) | **Not test-anchored at v1** — no AS exercises concurrent mutations from multiple tasks. The single-process Trust assumption keeps this out of v1 scope; recorded as outstanding (see CHK028). | n/a | Forward-flexibility contract that the natural `Arc<Mutex<HashSet>>` impl satisfies trivially |
| SC-001 (US1 in <30s wall-clock) | quickstart §2 (timer-measured by `cargo test`'s "finished in X.XXs" line) | quickstart §2 | |
| SC-002 (4-node × 3 topics × ≥100 emissions cross-cut) | n_node_graph.rs `four_node_star_100_send_topic_isolation` (added in 002) | quickstart §3 | |
| SC-003 (subscription change requires no code) | Operator-facing — observable via either restarting with edited TOML (quickstart §5) or runtime `subscribe`/`unsubscribe` API (US3) | quickstart §§4–5 | |
| SC-004 (contributor reproduces in <1h) | quickstart.md as a whole | n/a | Self-test on the SC contributor budget |
| SC-005 (subscribe/unsubscribe idempotency) | US3 AS-6/7 + dedicated unit tests in topic_runtime.rs | quickstart §4 | |
| SC-006 (drop log visible at default log level) | quickstart §2 (`--nocapture` shows the `event=topic_drop` line) | quickstart §2 | The visibility (default `info` surfaces it) is the assertion; log content is operator UX |
| SC-007 (runtime transitions observable through receive path) | US3 AS-3/5 | quickstart §4 | |

**Gaps surfaced and resolved by this matrix**: FR-010's duplicate-warn case had no AS coverage before 2026-05-30; US4 AS-6 was added to anchor the testable behavior (deduplicated state) — the warn log itself remains operator UX, not test-asserted. FR-014's mutation logs and FR-015's concurrent linearization are documented here as deliberately not test-anchored at v1 (logs are observability; concurrent linearization is forward-flexibility).

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
