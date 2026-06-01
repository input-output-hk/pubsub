# Research: Topics + Topic-Subscription Filtering

**Feature**: 002-topic-subscription-filtering

**Created**: 2026-05-30

**Purpose**: capture plan-level design choices that the spec deferred or that emerged during planning, with Decision / Rationale / Alternatives entries for each. This file resolves any `NEEDS CLARIFICATION` from `plan.md`'s Technical Context (there are none after `/speckit-clarify`) and fixes the shape of artifacts that downstream phases assume.

---

## §1. Message envelope shape: how does `topic` attach to a Message?

**Decision**: introduce a new envelope struct `Message { topic: TopicId, payload: MessagePayload }` where `MessagePayload` is the existing enum that was previously named `Message` in 001 (today its only variant is `Ping(N)`). The old name `Message` is reused for the envelope; the inner enum is renamed to `MessagePayload`. All call sites that previously constructed `Message::Ping(N)` migrate to `Message { topic, payload: MessagePayload::Ping(N) }`.

**Rationale**: a top-level field for `topic` keeps the topic a first-class component of every Message, satisfying FR-001 directly. Any payload variant inherits the topic dimension without per-variant duplication. The shape composes cleanly with future feature 003's envelope additions (`publisher_id`, `parent_hash`, `sequence`, `timestamp`, `signature`) — they slot in next to `topic` rather than inside the payload variants. The forwarded copy invariant in FR-009 ("forwarded delivery is a valid receipt") is observable on the envelope as a whole, not on a topic-stripped payload.

**Alternatives considered**:

- **Embed `topic` inside each `Message` variant** (e.g., `Message::Ping { topic, n }`). Rejected: per-variant duplication; adding 003's envelope fields (`publisher_id`, `signature`, …) would explode each variant; the topic accessor would have to pattern-match across variants; FR-009's forwarded-receipt language assumes a uniform envelope.
- **Wrap externally only at the network boundary** (`Envelope { topic, message }` constructed in network layer). Rejected: contradicts FR-001 ("Every `Message` MUST carry a `topic` field"); the topic must live on the type that callers construct, not on a transport-side wrapper.
- **Keep the existing `Message` enum and add a parallel `TopicId` argument to `Node::send`** (so send is `send(&self, to: &PeerId, topic: TopicId, message: Message)`). Rejected: contradicts FR-007 (send signature unchanged in shape) and FR-008 (decoupled — but topic is intrinsic to the message, not a per-send parameter). Also makes "two peers forwarding the same message" awkward because the topic would have to be carried separately.

**Migration cost**: the rename `Message` (enum) → `MessagePayload` and the introduction of `Message` (struct) is a mechanical rename + struct-construction-site update; the only construction sites in 001 are the test fixtures (`tests/common/mod.rs`) and the `Ping(N)` literals in the three integration test files. Tracked as a single early task in `/speckit-tasks`.

## §2. Subscription set storage primitive

**Decision**: `Arc<Mutex<HashSet<TopicId>>>` — the same primitive shape 001 already uses for the received-delivery queue (`Arc<Mutex<Vec<ReceivedDelivery>>>` in `src/node.rs:27`).

**Rationale**: FR-015 requires linearizability across the receive-path filter check, the mutator API, and the snapshot getter. A single `std::sync::Mutex` around the HashSet trivially satisfies this — every operation acquires the lock, performs its work, releases. Acquires-release memory ordering on the lock gives the happens-before guarantee FR-015 needs. The pattern matches 001's existing precedent; no new structural decision is introduced. The recv_task (which already runs inside `tokio::spawn`) acquires this lock synchronously inside an async block, which is acceptable for the in-memory v1 scope (the critical section is `HashSet::contains` — microseconds at most; no blocking I/O).

**Alternatives considered**:

- **`tokio::sync::Mutex` (async mutex)** instead of `std::sync::Mutex`. Rejected: the critical section is purely CPU-bound (HashSet membership check + maybe an insert), so blocking the executor for a few hundred nanoseconds is preferable to a Future round-trip on every receive. 001 uses `std::sync::Mutex` for `received`; consistency wins.
- **`std::sync::RwLock<HashSet<TopicId>>`**. Rejected: the read path (receive filter) and write path (subscribe/unsubscribe) execute at comparable frequencies; RwLock optimizes for many-readers-one-writer, which isn't the workload. Adds API complexity (separate read / write guards) without throughput benefit at v1 scale.
- **Lock-free** (`arc-swap`, `crossbeam` epoch, `evmap`). Rejected: contradicts the Constitution "Justified dependencies" rule (we'd need an ADR + a new dep); the workload doesn't motivate it; FR-015's linearizability is harder to argue for lock-free structures than for Mutex.
- **Per-topic locks / sharded** (`Arc<Mutex<…>>` per topic). Rejected: 002's scale is tiny (US2 exercises 3 topics); coarse-grained locking is fine; sharded locks complicate the `subscriptions()` snapshot semantics (atomic across all shards is non-trivial).

The lock primitive choice is tactical (Mutex is the natural and only reasonable answer at this scale); the *contract* it implements (FR-015 linearizability) is structural and pinned by the spec. The plan-level choice does not need an ADR.

## §3. Subscription mutator signature: `&self` vs `&mut self`, sync vs async

**Decision**: `fn subscribe(&self, topic: TopicId) -> SubscribeOutcome` and `fn unsubscribe(&self, topic: TopicId) -> UnsubscribeOutcome` — **synchronous** `fn` (not `async fn`), taking `&self` (not `&mut self`).

**Rationale**:

- **Sync, not async**: the body is a brief lock acquire + HashSet operation + release. No I/O, no scheduling-aware work. `async fn` would force every caller into a `.await` for no benefit and would propagate Send/Sync trait bounds into the public API surface. FR-006 explicitly specifies "synchronous, in-memory mutators".
- **`&self`, not `&mut self`**: the Node is shared between the externally-visible caller surface and the background recv_task (both hold the same `Arc<Node>` or equivalent). `&mut self` would require exclusive access, which is incompatible with that sharing pattern. Interior mutability via the lock from §2 lets the Node API expose `&self` methods while still mutating internal state. This mirrors 001's existing `received_messages(&self)` and `peers(&self)` accessors — none of those are `&mut self`.

**Alternatives considered**:

- **`async fn subscribe(&self, …)`**: rejected per FR-006 ("synchronous"). Also rejected because async signatures here would suggest scheduling-aware work that doesn't exist.
- **`&mut self` mutators**: rejected for the sharing pattern reason above. Would force the caller into `Arc<Mutex<Node>>` externally, redundant with the interior lock.
- **Static methods that take `Arc<Mutex<HashSet>>` directly**: rejected as ergonomic backslide; the lock is an implementation detail and should not appear in the public API.

This is the new structural decision that warrants ADR 0008 (see §8 below). Reversing it (e.g., switching to async, or to a free-standing function, or to `&mut self`) would touch every caller, so the constitution's "structural decision" trigger fires.

## §4. `subscriptions()` snapshot return type and ordering

**Decision**: `pub fn subscriptions(&self) -> Vec<TopicId>` — an owned `Vec` whose entry order is **unspecified** (set semantics, not sequence). The implementation acquires the lock, clones the HashSet into a Vec, releases.

**Rationale**: FR-013 requires (a) an owned snapshot whose lifetime is independent of subsequent mutations, (b) ordering unspecified ("the underlying state is a set"). `Vec<TopicId>` is the cheapest owned collection that satisfies (a). The clone is `O(n)` where `n` is the subscription-set size — bounded for any realistic deployment, trivial for v1 (sets of 0–3 topics in tests).

**Alternatives considered**:

- **`HashSet<TopicId>`** return. Rejected: gives the caller a "freshness" illusion (it's still a snapshot, not the live state) without any iteration benefit; `Vec` is more idiomatic for callers that just want to iterate or assert against `vec!["T1".into(), "T2".into()]`-style fixtures (after a sort).
- **`Box<[TopicId]>`**. Rejected: marginally cheaper than `Vec` (no spare capacity), but `Vec` is more familiar and the size difference is invisible at this scale.
- **Sorted `Vec<TopicId>`** (so tests get a deterministic order without sorting themselves). Rejected: imposes ordering semantics the spec explicitly says are unspecified; tests that genuinely want order-independent comparison should compare as sets; the sort cost is paid even by callers that don't care.

Tests asserting against the result will sort or convert-to-`HashSet` at the assertion site (e.g., `let mut got = node.subscriptions(); got.sort(); assert_eq!(got, vec!["T1".into(), "T2".into()])`), preserving the set-vs-sequence distinction.

## §5. `SubscribeOutcome` / `UnsubscribeOutcome` enum design

**Decision**: two distinct enums in `src/node.rs` (or a shared `outcomes.rs` if a third Outcome arrives later — not now):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscribeOutcome { Added, AlreadyPresent }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsubscribeOutcome { Removed, NotSubscribed }
```

Both `#[non_exhaustive]` would be tempting for future-extension headroom but is rejected (see Alternatives).

**Rationale**:

- **Separate enums per operation**: each enum's variants name the operation's outcome in the operation's own terms (Added/AlreadyPresent for subscribe; Removed/NotSubscribed for unsubscribe). A shared `MembershipChange { Changed, Unchanged }` would be more compact but loses the directionality information that callers and log messages care about. Each enum has exactly two variants — small, exhaustive, no Result wrapping needed (FR-006: no failure modes in this iteration).
- **`Copy`-able**: both enums are `Copy` since the variants carry no payload. Reduces caller friction (no clone needed for logging + matching).
- **No `#[non_exhaustive]`**: Result wrapping is the explicit forward-extension path for failure variants (FR-006 records this); marking the Outcome enums non-exhaustive would suggest future variants can be added directly, but the spec says future *failure* modes go in a `Result::Err`, not a new Outcome variant. Keep the enums closed; if a third Outcome is needed (unlikely), it's a deliberate breaking change with an ADR.

**Alternatives considered**:

- **`bool`-typed return (like `HashSet::insert`)**: cleaner but loses the named Outcome semantics that callers and log lines reference. FR-006 spells the Outcome variants by name.
- **Single shared enum**: rejected as above (directionality matters in logs).
- **`Result<SubscribeOutcome, SubscribeError>` with a `SubscribeError` that is currently uninhabited (`Infallible`-style)**: rejected as over-anticipatory; the call site doesn't need `?` propagation today. When real failures arrive, the API change is `Outcome -> Result<Outcome, Error>` — a single ripple that downstream callers must handle. The cost of the change is low; the cost of premature `Result` is API noise everywhere.

## §6. TOML field shape for `subscribed_topics`

**Decision**: top-level plain string array, `subscribed_topics = ["T1", "T2"]`. Optional (absent or `[]` both valid; both yield an empty subscription set). Each entry parsed via `TopicId::from_str`. Strict-unknown-fields policy (`#[serde(deny_unknown_fields)]`) from 001 continues to apply.

The loader (renamed in 002 from `load_peer_list` to `load_node_config` — see CHK017 resolution) gains the same two-stage validation pipeline 001 already uses for peers: first parse the raw TOML into `RawNodeConfig { peers: …, subscribed_topics: Vec<String> }`, then run `TopicId::from_str` on each string to surface validation failures with the offending entry and the file path. Validation failure becomes `ConfigError::InvalidTopic { entry, path, source }`, the structural mirror of `ConfigError::InvalidPeer`.

**Rationale**: covered exhaustively in the spec's pre-spec chat and Clarifications session 2026-05-29 (Q1 of round 1 affirms PeerId-rules-only). Plain string array matches the data shape (just IDs). Plain array vs table-array (`[[subscribed_topics]] id = "…"`) is the simpler shape at zero loss of expressiveness for 002. If 003+ ever needs per-topic config (priority, retention, etc.), the field can grow to a table-array as a deliberate breaking change with an ADR — the same migration path 001 left open for peers.

**Alternatives considered**: see spec's pre-spec chat. Recorded here only by reference; not re-litigated.

## §7. Off-topic drop log + mutation log fields and event markers

**Decision**: use the existing 001 `tracing` facility with the following event shapes (target string: `pubsub_node::node`, matching 001's convention from `src/node.rs:51`):

```text
# FR-011 — off-topic drop (info level, one entry per dropped message):
tracing::info!(
    target: "pubsub_node::node",
    event = "topic_drop",
    self_id = %node.self_id,
    from = %envelope.from,
    topic = %envelope.message.topic,
);

# FR-014 — subscribe state change (info level, on Added outcome):
tracing::info!(
    target: "pubsub_node::node",
    event = "topic_subscribed",
    self_id = %node.self_id,
    topic = %topic,
);

# FR-014 — unsubscribe state change (info level, on Removed outcome):
tracing::info!(
    target: "pubsub_node::node",
    event = "topic_unsubscribed",
    self_id = %node.self_id,
    topic = %topic,
);

# FR-014 — subscribe idempotent no-op (debug level, on AlreadyPresent):
tracing::debug!(
    target: "pubsub_node::node",
    event = "topic_subscribe_noop",
    self_id = %node.self_id,
    topic = %topic,
    reason = "already_present",
);

# FR-014 — unsubscribe idempotent no-op (debug level, on NotSubscribed):
tracing::debug!(
    target: "pubsub_node::node",
    event = "topic_unsubscribe_noop",
    self_id = %node.self_id,
    topic = %topic,
    reason = "not_subscribed",
);

# FR-010 (extended) — TOML loader detected duplicate `subscribed_topics` entry
# (warn level, one event per duplicated topic per load call):
tracing::warn!(
    target: "pubsub_node::config",
    event = "topic_config_duplicate",
    topic = %duplicate_topic,
    config_path = %path.display(),
);
```

The duplicate-warn event uses target `pubsub_node::config` (not `pubsub_node::node`) because it is emitted from the loader (`config::load_node_config`) before any Node exists. Consequently the event does NOT carry `self_id` — the operator's process invocation (`--self-id <id>` on the CLI, plus the `config_path`) is sufficient context. This is asymmetric with the other 002 events (all of which carry `self_id`); the asymmetry is intentional and recorded here for the audit trail.

**Rationale**:

- **Stable `event` field**: gives operators a deterministic key to grep / filter on (`event=topic_drop`, etc.). The wording matches the verb forms used in the spec.
- **`self_id` always present**: disambiguates which node emitted the event in a multi-node integration test or multi-instance deployment.
- **`topic` always present**: necessary for any operator triaging "why is this topic not getting through?". The drop event additionally carries the sender's `from` (sender's logical peer id), per FR-011.
- **No payload in the log entry**: out of spec (FR-011 only requires topic / sender / self id / event marker); avoids accidentally logging sensitive payload bytes.
- **No FR or spec-section citation in the strings**: matches the saved-feedback convention for operator-facing strings.

**Alternatives considered**:

- **One `event = "subscription_change"` event with an `outcome` field** (Added / Removed / AlreadyPresent / NotSubscribed). Rejected: stable string markers are easier to grep individually; combining them under one event name forces operators to filter on `outcome`, doubling the cognitive load.
- **Use 001's `tracing::warn!` for the drop** to match 001 FR-010's warn-on-drop pattern. Rejected: 001's drop is a network-level error (unregistered peer id), which warrants warn; 002's drop is an expected, in-spec filter behavior, which warrants info. FR-011 normatively specifies info level.
- **Log target string `pubsub_node::topics`** for the 002-specific events. Rejected: 001 uses `pubsub_node::node` for events emitted from the Node module; consistency wins.

## §8. ADR slot summary

| Slot | Title | Trigger | Status |
|------|-------|---------|--------|
| 0001–0007 | (existing 001 ADRs — async runtime, TOML/serde, tracing, clap, thiserror, receive-task model, NetworkHandle actor-handle) | 001 | Existing — referenced, not modified |
| **0008** | **Subscription mutator shape: sync `&self` mutators with interior mutability, linearizable per FR-015** | 002 | **NEW — to be authored** |

**ADR 0008 scope**: documents the choice of (a) `fn` not `async fn`, (b) `&self` not `&mut self`, (c) interior mutability via `Arc<Mutex<HashSet<TopicId>>>` matching 001's `Arc<Mutex<Vec<ReceivedDelivery>>>` pattern, (d) linearizability as the normative contract per FR-015. Lists alternatives (async mutators, &mut self, RwLock, sharded locks) and the reasons they were rejected. Notes the forward-extension path: when failure modes appear (registry validation, persistence I/O), the return type becomes `Result<Outcome, Error>` — that's a follow-on ADR, not a revision of 0008.

The other 002 deltas are tactical extensions of 001's existing structure and don't merit their own ADRs. Per Constitution Principle III ("a decision is **structural** if reversing it would require touching unrelated code, external interfaces, or another protocol layer; a decision is **tactical** if reversing it is a local rewrite"), each of the below qualifies as tactical — reversing any of them is a local rewrite within `pubsub-node/`, with no ripple across protocol layers, external interfaces, or unrelated subsystems:

- TopicId mirroring PeerId: parallel application of an established pattern.
- Subscription set as `Arc<Mutex<HashSet<…>>>`: direct application of 001's existing pattern for mutable Node state.
- Message envelope shape (§1): a localized rename + composition, not a structural choice that ripples across protocol layers; the alternative shapes considered in §1 are recorded here for the audit trail.
- TOML field extension (`subscribed_topics`): an additive schema extension, same kind of change as adding a column to a config file.
- Tracing field names / event markers (§7): tactical naming choices recorded in `research.md` and `contracts/library-api.md`.

## §9. Open follow-ups (v2+ items)

Captured so they cannot be silently rediscovered later:

- **Registry-driven topic validation**: when feature 008 introduces the registry, `subscribe(T)` may grow a registry-lookup precondition. The lookup lives in the *wrapper* layer (admin API / orchestrator), not the Node — that decision is locked by the spec's pre-spec chat. Node's `subscribe` will keep its current `Outcome`-only return; the wrapper will wrap that in `Result<Outcome, AdminError>` when the registry says "no". A future ADR will record the wrapper layer's shape when feature 008 lands.
- **Persistence of subscription state**: deferred per spec Assumptions. When persistence arrives, the natural pattern is a sidecar / wrapper that snapshots the subscription set to disk and replays on startup — keeping the Node a pure in-memory state machine. A future ADR will record the persistence wrapper's shape.
- **`Subscriptions` opaque type vs raw `Vec<TopicId>`**: if a future feature wants the snapshot to carry additional fields (e.g., per-topic metadata), `subscriptions()` may return an opaque `Subscriptions` type with accessors. For 002, raw `Vec<TopicId>` is sufficient; the change is non-breaking if `Subscriptions` exposes `IntoIterator<Item = TopicId>` and `Deref<Target = [TopicId]>`.
- **Connection-close on misbehavior**: feature 004+ may escalate off-topic drops to closing the offending peer's connection (workstream-level note in `specs/ROADMAP.md`). The receive-path filter in 002 is the seam that future code will hook into.
- **Self-addressing under connection-based transports**: workstream-level note N-002 in `specs/IMPLEMENTATION_NOTES.md`. Trigger: feature 004 (connection-oriented network model).
- **Local emission vs local receipt**: workstream-level note N-001 in `specs/IMPLEMENTATION_NOTES.md`. Trigger: introduction of an operator-facing admin / REST API.
- **`SubscribeOutcome` / `UnsubscribeOutcome` extension**: if a future iteration needs richer outcome semantics (e.g., RegistryUnknown for a registry-validation path), the API change wraps the existing enum in a `Result`. A follow-on ADR will document the wrapping when the trigger fires.
