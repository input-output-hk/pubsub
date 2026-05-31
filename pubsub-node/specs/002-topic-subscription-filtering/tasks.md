---

description: "Tasks: Topics + Topic-Subscription Filtering"
---

# Tasks: Topics + Topic-Subscription Filtering

**Input**: Design documents from `/specs/002-topic-subscription-filtering/`

**Prerequisites**: plan.md ✓, spec.md ✓, research.md ✓, data-model.md ✓, contracts/{library-api,node-config.toml}.md ✓, quickstart.md ✓ (001's `contracts/cli.md` is inherited unchanged — no 002 contract file in `cli.md`)

**Tests**: Tests at the acceptance-scenario level are MANDATORY for this feature — each user story has numbered Given/When/Then scenarios in `spec.md` that map directly to integration tests, and `data-model.md §7.5` is the FR → US matrix. Strict red-green-refactor TDD is NOT required for 002 per `plan.md` Constitution Check §II ("002 is not protocol-critical — no crypto, no chain semantics, no registry interaction; those start at 003"). Tests are authored alongside the substrate they exercise. Where substrate and tests fall in the same task or phase, both land in the same commit; where they span phases, the substrate task lands first in a green-test state and the test task follows in a commit that keeps the green-checkpoint invariant. The duplicate-warn log (FR-010), mutation logs (FR-014), and drop log (FR-011) are deliberately **not** test-anchored — they are operator UX per `data-model.md §7.5` (CHK027 / CHK038 / CHK050 resolutions). Tests assert on the `Outcome` return values, `received_messages()` snapshot, and `subscriptions()` snapshot, never on log content.

**ADRs**: One new structural decision (ADR 0008 — subscription mutator shape, per `research.md §8`) is authored in Phase 2 from `research.md §3` (sync `&self` mutators + interior mutability + linearizability). 001's ADRs 0001–0007 are unchanged and not re-authored.

**Organization**: Tasks are grouped by user story. The 002 substrate (TopicId, Node subscription state + mutators + getter + receive-path filter, NodeConfig rename + extension, Message envelope rename, ConfigError::InvalidTopic) is **foundational** — all four user stories depend on it. US1–US3 are test-only phases that exercise the substrate; US4 adds the loader-side wiring + CLI extension + TOML tests. Phase 7 is polish (fmt/clippy/test sweep, rustdoc audit, quickstart walkthrough, FR coverage verification).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Different files, no incomplete dependencies — can run in parallel
- **[Story]**: US1 / US2 / US3 / US4 — user-story phase tasks only
- File paths are absolute from the crate root (`pubsub-node/`)

## Path Conventions

- **Single Cargo crate** (lib + bin) per `plan.md` "Project Structure" — unchanged from 001
- Source: `src/`
- Integration tests: `tests/`
- ADRs: `docs/decisions/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: 002 inherits the Cargo crate, dependency set, and lint configuration from 001 (T001–T003 of 001's tasks). No new dependencies are introduced (per `plan.md` "Primary Dependencies: unchanged from 001. … No new crate is required"). Phase 1 is therefore a single baseline-check task confirming the inherited foundation is green before any 002 code edits.

- [ ] T001 Verify the 002 branch baseline is green before touching any code: run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo build`, and `cargo test` from the crate root. All four MUST pass (the 002 branch inherits 001's `e87973e` / `72b671a` checkpoint commits; this task guards against unexpected drift on the branch). No code edits in this task — observation only.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: All shared substrate that all four user stories depend on — the new `TopicId` type, the Message envelope rename (Message struct + MessagePayload enum per `research.md §1`), the CHK017 renames in `src/config.rs` (`PeerListConfig` → `NodeConfig`, `load_peer_list` → `load_node_config`, `RawPeerListConfig` → `RawNodeConfig`), the `subscribed_topics` field + duplicate-warn loader sub-step (FR-010 + CHK025), the Node's subscription state + mutators + getter + receive-path filter (FR-003 / FR-004 / FR-006 / FR-013 / FR-014 / FR-015), the new `ConfigError::InvalidTopic` variant, the `SubscribeOutcome` / `UnsubscribeOutcome` enums, the updated `src/lib.rs` re-exports, the extended `tests/common/mod.rs` fixture, and ADR 0008.

**⚠️ CRITICAL**: No user-story phase work can begin until this phase completes. Several tasks in this phase are **breaking changes** that update many call sites in a single commit to preserve the green-checkpoint invariant (Constitution §"Development Workflow"); they are explicitly NOT parallelizable.

### ADR (Principle III deliverable)

The ADR transcribes `research.md §3` (mutator shape) + `research.md §8` (ADR slot summary) into the standard `Context / Decision / Consequences / Alternatives` ADR shape.

- [ ] T002 [P] Author ADR 0008 in `docs/decisions/0008-subscription-mutator-shape.md` from `research.md §3` and `research.md §8`. Decision: `Node::subscribe` / `Node::unsubscribe` are **sync `fn` on `&self`** with interior mutability via `Arc<Mutex<HashSet<TopicId>>>`, linearizable per FR-015. Alternatives rejected: `async fn` (no scheduling-aware work warrants the Future overhead); `&mut self` (incompatible with the Node-shared-between-tasks pattern, would force `Arc<Mutex<Node>>` externally redundant with the interior lock); `RwLock<HashSet<TopicId>>` (read/write workload comparable; RwLock optimises for many-readers-one-writer); lock-free / sharded (no v1 workload motivates the complexity; FR-015's linearizability is harder to argue for non-mutex primitives); free-standing functions over the lock (ergonomic backslide). Forward-extension note: when failure modes appear (registry validation in feature 008; persistence I/O), the return type becomes `Result<Outcome, Error>` — that's a follow-on ADR, not a revision of 0008 (per `research.md §8` boundary). Cross-reference the constitution Principle III (structural-vs-tactical) citation that lives in `research.md §8`.

### Substrate types (new and modified)

- [ ] T003 [P] Create `src/topic.rs` (new file, parallel to `src/peer.rs`). Implement `pub struct TopicId(String)` with `Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize` (via `#[serde(try_from = "String")]`). `impl FromStr for TopicId` rejecting empty / NUL-containing strings with a `pub enum TopicIdError { Empty, ContainsNul }` (`#[derive(Debug, thiserror::Error, PartialEq, Eq)]`, `#[error("topic id must not be empty")]` / `#[error("topic id must not contain a NUL byte")]`). `impl TryFrom<String> for TopicId` for the serde path. `pub fn as_str(&self) -> &str` accessor. `impl Display` printing the inner string verbatim. Include unit tests in `src/topic.rs` for the two `TopicIdError` rejection cases (empty input, NUL-containing input) — analogous to 001's T012 PeerId rejection tests. Per `data-model.md §1`, `contracts/library-api.md` "TopicId" section, and `research.md §6`. **Error-location note**: `TopicIdError` lives in `src/topic.rs` (not in `src/error.rs`) — mirrors 001's `PeerIdError` co-location in `src/peer.rs` (rationale documented in 001 T012).
- [ ] T004 [P] Extend `src/error.rs`: add the `InvalidTopic(String)` variant to the existing `ConfigError` enum with `#[error("config invalid topic entry: {0}")]`. Mirrors the existing `InvalidPeer(String)` variant's shape exactly (per `data-model.md §6` and `contracts/library-api.md` `ConfigError` section). Independent of T002 and T003 (different file).
- [ ] T005 Rewrite the Message envelope shape in `src/message.rs` per `research.md §1` and `data-model.md §2`: rename the existing `pub enum Message { Ping(u64) }` to `#[non_exhaustive] pub enum MessagePayload { Ping(u64) }` (preserving the existing `#[non_exhaustive]` discipline and derives — `Debug, Clone, PartialEq, Eq`); add a new `pub struct Message { pub topic: TopicId, pub payload: MessagePayload }` with the same derives. **Required ergonomic constructor** `impl Message { pub fn ping(topic: TopicId, n: u64) -> Self { Self { topic, payload: MessagePayload::Ping(n) } } }` — mandatory at this task because `MessagePayload` is not re-exported until T009; making the constructor available now lets the migrated test sites construct Pings via `Message::ping(topic, n)` without needing direct access to `MessagePayload`. **Breaking change**: every call site that constructs `Message::Ping(N)` MUST be updated to `Message::ping(topic, n)` (or the literal `Message { topic, payload: MessagePayload::Ping(N) }` if the file already has `MessagePayload` in scope). Update call sites in this same task to keep the green-checkpoint invariant: `tests/common/mod.rs` (fixture builders construct Pings via `Message::ping(test_topic.clone(), n)` using a placeholder `TopicId` literal — the subscription-set wiring for that literal is added in T010, not here), `tests/two_node_ping.rs` (every Ping constructed in the 001 tests), `tests/n_node_graph.rs` (same). `tests/config_loading.rs` does NOT need updates (it tests TOML loading, not Message construction). Per `data-model.md §2`. NOT parallelizable — touches every test file that constructs Messages.
- [ ] T006 CHK017 rename in `src/config.rs`: rename `pub struct PeerListConfig` → `pub struct NodeConfig`, `pub fn load_peer_list` → `pub fn load_node_config`, and the internal shadow `struct RawPeerListConfig` → `struct RawNodeConfig`. Update every caller in the crate **in the same commit** to preserve the green-checkpoint invariant (Constitution §"Development Workflow"): `src/node.rs` (the `Node::new` parameter name `peer_list: PeerListConfig` → `config: NodeConfig`), `src/main.rs` (the binary's loader invocation and any local bindings), `src/lib.rs` re-exports line (rename the existing `pub use config::{PeerEntry, PeerListConfig, load_peer_list};` line to `pub use config::{PeerEntry, NodeConfig, load_node_config};` — **owned by this task**; T009 covers only the *additions* of new 002 re-exports, not this rename), `tests/common/mod.rs` fixture builders, and `tests/config_loading.rs` (every test that names `PeerListConfig` or calls `load_peer_list`). The TOML on-disk schema is unchanged for the 001 `[[peers]]` portion — only Rust identifiers change. Per `contracts/node-config.toml.md`, `contracts/library-api.md` "NodeConfig" / "load_node_config" sections, and CHK017 resolution (commit `e87973e`). NOT parallelizable — touches every file that names the old identifiers.
- [ ] T007 Extend `NodeConfig` in `src/config.rs` (depends on T003, T004, T006): add the `subscribed_topics: Vec<TopicId>` field with `#[serde(default)]` (absent or empty array both yield empty Vec per FR-010). Extend the `RawNodeConfig` shadow struct correspondingly with `subscribed_topics: Vec<String>` (the loader runs `TopicId::from_str` per-entry in a post-parse pass, so validation failures map to `ConfigError::InvalidTopic` with the offending entry + path — analogous to the existing `InvalidPeer` path). Extend `load_node_config` with the two pipeline additions per `data-model.md §5` step 4 and `contracts/library-api.md`'s `load_node_config` section: (a) validate each `subscribed_topics` entry via `TopicId::from_str` → `ConfigError::InvalidTopic("{path}: {topic_id_error}")` on failure (fail-fast); (b) after all validation succeeds, scan the validated `Vec<TopicId>` for duplicates and emit one `tracing::warn!` per duplicated topic with `target: "pubsub_node::config"`, `event = "topic_config_duplicate"`, `topic = %duplicate_topic`, `config_path = %path.display()` (per FR-010 + `research.md §7` + CHK025 / CHK052 resolutions). Duplicates are NOT a startup failure; the returned `NodeConfig.subscribed_topics` retains the original Vec shape including duplicates (consumer dedupes at the `HashSet` boundary — pinned by `contracts/library-api.md` "Return-shape contract on duplicates"). The `deny_unknown_fields` discipline continues to apply at the top level. Per `data-model.md §5`, `contracts/node-config.toml.md`, FR-010, FR-012.
- [ ] T008 Extend the `Node` type in `src/node.rs` (depends on T003, T005, T006, T007). Add the new field: `subscriptions: Arc<Mutex<HashSet<TopicId>>>` (per `data-model.md §3`; `Arc<Mutex<…>>` primitive choice per `research.md §2`, justified by ADR 0008). Define the two outcome enums inline in `src/node.rs` (alongside the Node type — per `data-model.md §4` "may migrate to a separate src/outcomes.rs if a third Outcome arrives later — not anticipated at v1"): `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum SubscribeOutcome { Added, AlreadyPresent }` and `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum UnsubscribeOutcome { Removed, NotSubscribed }` — both closed enums (NOT `#[non_exhaustive]`) per `research.md §5`. Update `Node::new<N: Network>` signature to `pub async fn new<N: Network>(self_id: PeerId, config: NodeConfig, initial_subscriptions: HashSet<TopicId>, network: Arc<N>) -> Result<Self, NodeError>` (parameter order rationale in `contracts/library-api.md` "Constructor"; the `config` parameter name is the CHK017 rename of `peer_list`; `initial_subscriptions` is a fresh required parameter taken as an already-parsed in-memory value per FR-012). In the constructor body: clone the provided `HashSet<TopicId>` into the new `Arc<Mutex<…>>` field; spawn the recv_task with two `Arc::clone`s (one for `received`, one for `subscriptions`) so the task shares the same lock-protected subscription set as external mutator callers. The recv_task body: for each `Envelope` from `rx.recv().await`, acquire the `subscriptions` mutex briefly, check `HashSet::contains(&envelope.message.topic)`, then either (a) push the `ReceivedDelivery` into `received` if subscribed, or (b) emit one `tracing::info!` with `target: "pubsub_node::node"`, `event = "topic_drop"`, `self_id = %node.self_id`, `from = %envelope.from`, `topic = %envelope.message.topic` and discard the delivery (per FR-004 + FR-011 + `research.md §7`). **Lock-acquisition order**: subscriptions first, then received (per `data-model.md §3` convention). Add the three public methods: `pub fn subscribe(&self, topic: TopicId) -> SubscribeOutcome` and `pub fn unsubscribe(&self, topic: TopicId) -> UnsubscribeOutcome` (both sync, both `&self`, both per FR-006 / FR-014 / FR-015 / ADR 0008) emitting the FR-014 tracing events per `research.md §7` (info on state-change, debug on no-op); `pub fn subscriptions(&self) -> Vec<TopicId>` (FR-013, snapshot via clone-under-lock; entry order unspecified). Per `data-model.md §3` / §4, `contracts/library-api.md` `Node` section, FR-003 / FR-004 / FR-006 / FR-009 / FR-011 / FR-013 / FR-014 / FR-015. **Green-checkpoint update**: to preserve `cargo test` greenness across the T008 commit boundary, this task ALSO updates `tests/common/mod.rs` fixture builders' internal `Node::new` invocations to pass `HashSet::from([test_topic.clone()])` (using the same sentinel `TopicId` literal T005's migration adopted for placeholder Ping construction) as the new `initial_subscriptions` argument — so existing 001 tests' Pings continue to pass through the filter after Node grows its subscription state. T010 still owns the fixture's *public-API* parameter addition (the parameter that lets external test files override this default) + the `assert_subscriptions` helper; T008's update here is the minimal internal plumbing required by the `Node::new` signature change.
- [ ] T009 Add the new 002 re-exports to `src/lib.rs` (depends on T003, T005, T006, T008). The CHK017 rename of the existing `pub use config::…` line is **owned by T006** (handled inline in T006's commit to preserve `cargo build` greenness); this task adds only the new entries per `contracts/library-api.md`'s "Re-exports" section: `pub use topic::{TopicId, TopicIdError}; pub use message::MessagePayload; pub use node::{SubscribeOutcome, UnsubscribeOutcome};`. The existing `pub use message::Message;` line is unchanged by T005's rename (the public name `Message` survives — only the underlying shape changed from enum to struct). The existing `pub use config::{PeerEntry, NodeConfig, load_node_config};` line (post-T006 form) is unchanged by this task. The `RawNodeConfig` shadow type is internal — NOT re-exported (per CHK048 verification). Verify `cargo build` is green and that downstream consumers can name every type listed in `contracts/library-api.md` "Re-exports" via the flat `pubsub_node::…` namespace.
- [ ] T010 Extend `tests/common/mod.rs` (depends on T009). Update the fixture builder(s) — `two_node_fixture` and any sibling helpers — to accept an `initial_subscriptions: HashSet<TopicId>` parameter (or a `Vec<TopicId>` collected into a `HashSet` inside the helper). Default for backward-compatibility: when an existing 001 test calls the fixture without specifying a topic, the helper subscribes both nodes to a sentinel `TopicId::from_str("test").unwrap()` (or analogous) — chosen so the existing 001 tests' Pings (whose topic is constructed with that same sentinel) continue to pass through the filter. Add an `assert_subscriptions(node, &[TopicId])` helper that wraps the "snapshot, sort, assert as set" idiom for tests that compare subscription sets (per `data-model.md §9`). Update the 001 fixture call sites (`tests/two_node_ping.rs`, `tests/n_node_graph.rs`) to pass the sentinel topic — these were already touched in T005's Message-construction migration; T010 just adds the subscription-set parameter. Per `data-model.md §9`, `contracts/library-api.md` "Node constructor".

**Checkpoint**: 002 substrate is complete. `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, and `cargo test` (with all 001 tests still passing under the new Message + subscription discipline) are green. User-story phases can start.

---

## Phase 3: User Story 1 — Single-Topic Subscriber Filters Off-Topic Traffic (Priority: P1) 🎯 MVP

**Goal**: Demonstrate the irreducible filtering behavior — a Node subscribed to a single topic retains messages tagged with that topic and silently drops messages tagged with any other topic. The 002 MVP per spec.md US1 "Why this priority": "Without it, no other behavior (multi-topic, dynamic mutation, TOML loading) has meaning."

**Independent Test**: Run `cargo test --test topic_filter`. All 3 tests pass within a few seconds; the test names map 1:1 to US1 AS-1 / AS-2 / AS-3.

### Implementation for User Story 1

- [ ] T011 [P] [US1] Implement integration tests in `tests/topic_filter.rs` covering the three US1 acceptance scenarios. Required tests (each `#[tokio::test]`, each using `mod common;` to access the extended fixture from T010):
  - `on_topic_message_retained` — US1 AS-1: construct A with `subscriptions = {T1}` and B with arbitrary subscriptions; have B send `Message { topic: T1, payload: MessagePayload::Ping(42) }` to A; await delivery; assert `a.received_messages()` contains exactly one `ReceivedDelivery { from: b.id().clone(), message: Message { topic: T1, payload: MessagePayload::Ping(42) } }`.
  - `off_topic_message_dropped_silently` — US1 AS-2: same fixture (A subscribed to `{T1}`); have B send `Message { topic: T2, payload: MessagePayload::Ping(7) }` to A; after a settle window (use `tokio::time::sleep` with a small budget, e.g., 100ms — matches 001's `await_delivery` polling interval), assert `a.received_messages().is_empty()`. Per the FR-011 / FR-014 / FR-010 test discipline (CHK027 resolution), the test asserts **only** on the snapshot — NOT on log content. The info-level drop log emission is operator UX, exercisable in quickstart §2 with `--nocapture`.
  - `own_emission_not_in_local_snapshot` — US1 AS-3 / FR-009: construct A with `subscriptions = {T1}`, have A emit `Message { topic: T1, payload: MessagePayload::Ping(13) }` addressed to B (a separate peer); assert `a.received_messages().is_empty()` (A doesn't see its own emission). B's snapshot is incidental and not asserted in this test.

**Checkpoint**: US1 is independently functional. The 002 filter behavior is proven at the MVP level. Could ship here.

---

## Phase 4: User Story 2 — Multi-Topic Node in N-Node Graph (Priority: P2)

**Goal**: Show the topic filter composes correctly across multiple topics on a multi-node graph, and that the 001 N-node tests' multi-node fan-out invariants still hold under the 002 topic dimension.

**Independent Test**: Run `cargo test --test n_node_graph`. All US2-specific tests (added in T012) plus the inherited US2-from-001 tests pass; specifically, the SC-002 cross-cut (4-node × 3-topic × 100-emission isolation) lands exact set-equality per the FR-004 filter contract.

### Implementation for User Story 2

- [ ] T012 [P] [US2] Extend `tests/n_node_graph.rs` with US2 / SC-002 acceptance scenarios (additive — 001's existing tests in this file MUST keep passing under the T005 / T010 migrations). New required tests:
  - `four_node_star_three_topics_filtering` — US2 AS-1: construct A, B, C, D with subscriptions `A={T1}`, `B={T1,T2}`, `C={T2,T3}`, `D={T3}` sharing an `InMemoryNetwork`; have a designated peer (e.g., A or an additional fixture node — implementer's choice) emit one `Message { topic: T, payload: Ping(round) }` per topic T ∈ {T1, T2, T3} addressed in turn to each of A/B/C/D; after the round completes (use `await_delivery` per known-expected delivery), assert each node's `received_messages()` snapshot equals exactly the intersection of "intended deliveries to that node" with that node's subscription set. Zero false-positives, zero false-negatives.
  - `four_node_star_topic_interleave_ordering` — US2 AS-2: same fixture; interleave emissions on T2 and T3 across two rounds; assert that within each node's snapshot, the arrival order matches the receive-task's observation order (per `data-model.md §3`). The intent is to confirm that 001's per-sender FIFO ordering (research.md §9 of 001) survives the topic filter layered on top.
  - `four_node_star_100_send_topic_isolation` — US2 AS-3 / SC-002: same fixture; emit 100 sequential `Ping` messages distributed across 3 topics (T1, T2, T3) — use a deterministic sequence or seeded RNG (record the choice in an in-file comment per the Engineering Standards "Reproducible tests" rule; matches 001's T020 / SC-005 / CHK056 precedent). Each emission is addressed to each of A/B/C/D (i.e., 400 deliveries total, or 100 per recipient — implementer picks the exact shape, but the SC-002 condition is "at least 100 emissions × at least 3 topics"). After all `await_delivery`s resolve, assert every node's `received_messages()` snapshot exactly equals `intended_deliveries_for_node ∩ subscriptions(node)`. This is the SC-002 conjunction landing in test form.

**Checkpoint**: US1 + US2 both independently functional. The topic filter composes across N-node graphs with mixed subscriptions.

---

## Phase 5: User Story 3 — Dynamic Subscription Transitions Take Immediate Effect (Priority: P3)

**Goal**: Exercise the runtime `subscribe` / `unsubscribe` API + `subscriptions()` getter at the integration level — transitions take effect on the next inbound message; snapshot grows monotonically; idempotent calls return the no-op outcome without state change; decoupled emission succeeds regardless of emitter's subscription set (US3 AS-8).

**Independent Test**: Run `cargo test --test topic_runtime`. All eight US3 acceptance scenarios pass.

### Implementation for User Story 3

- [ ] T013 [P] [US3] Implement integration tests in `tests/topic_runtime.rs` covering the eight US3 acceptance scenarios. Required tests (each `#[tokio::test]`, all using `mod common;`):
  - `initial_set_filters_inbound` — US3 AS-1: A initialized with `subscriptions = {T2}`; B emits `Ping(1, T1)` then `Ping(2, T2)`; assert `a.received_messages()` contains exactly `Ping(2, T2)`.
  - `subscribe_returns_added_and_updates_set` — US3 AS-2: continuing from AS-1 state; call `a.subscribe(T1.clone())`; assert returned `SubscribeOutcome::Added`; assert `a.subscriptions()` (sorted or as `HashSet`) equals `{T1, T2}`; assert `a.received_messages()` is unchanged by the mutator call itself.
  - `subscribe_makes_subsequent_message_visible` — US3 AS-3: continuing from AS-2 state (subscriptions = `{T1, T2}`); B emits `Ping(3, T1)`; await delivery; assert `a.received_messages()` now contains `Ping(2, T2)` and `Ping(3, T1)` (the previously-retained AS-1 entry plus the new T1 entry).
  - `unsubscribe_returns_removed_and_updates_set` — US3 AS-4: continuing from AS-3 state; call `a.unsubscribe(T1)`; assert returned `UnsubscribeOutcome::Removed`; assert `a.subscriptions()` equals `{T2}`; assert `a.received_messages()` is unchanged by the mutator call (no retroactive removal of the previously-retained T1 entry — snapshot grows monotonically per FR-013 / SC-007).
  - `unsubscribe_makes_subsequent_message_dropped` — US3 AS-5: continuing from AS-4 state (subscriptions = `{T2}`); B emits `Ping(4, T1)` then `Ping(5, T2)`; await delivery for the T2 message; assert `a.received_messages()` contains `Ping(2, T2)`, `Ping(3, T1)`, and `Ping(5, T2)` only — the new `Ping(4, T1)` is dropped, and the previously-retained `Ping(3, T1)` from AS-3 REMAINS.
  - `subscribe_idempotent_returns_already_present` — US3 AS-6 / SC-005: A subscribed to `{T2}`; call `a.subscribe(T2)`; assert returned `SubscribeOutcome::AlreadyPresent`; assert `a.subscriptions()` is unchanged; assert `a.received_messages()` is unchanged.
  - `unsubscribe_idempotent_returns_not_subscribed` — US3 AS-7 / SC-005: A subscribed to `{T2}` (T1 absent); call `a.unsubscribe(T1)`; assert returned `UnsubscribeOutcome::NotSubscribed`; assert `a.subscriptions()` is unchanged; assert `a.received_messages()` is unchanged.
  - `decoupled_emission_succeeds_on_unsubscribed_topic` — US3 AS-8 / FR-008: construct A with `subscriptions = {T2}` (T1 NOT subscribed) and B with `subscriptions = {T1}`; call `a.send(b.id(), Message { topic: T1, payload: MessagePayload::Ping(99) }).await?`; assert the call resolves `Ok(())`; await delivery on B; assert B's `received_messages()` contains the delivery. Decoupling: A's emission succeeds even though A is not subscribed to T1; B's reception is governed by B's subscription set.

**Checkpoint**: US1 + US2 + US3 all independently functional. The runtime API is proven end-to-end.

---

## Phase 6: User Story 4 — Subscriptions Loaded from TOML at Node Construction (Priority: P4)

**Goal**: Wire the operator-facing TOML loading flow — the binary parses `subscribed_topics` from the node-config TOML, hands the resulting `HashSet<TopicId>` to `Node::new` as `initial_subscriptions`, and the resulting Node behaves exactly as the in-process equivalents in US1–US3. Plus the operator-visible duplicate-warn behavior and TOML-error paths.

**Independent Test**: Run `cargo test --test config_loading`. All six US4 acceptance scenarios pass (alongside 001's inherited US3-from-001 config-loading tests). Plus run the binary end-to-end per quickstart.md §5 to verify the CLI surface.

### Implementation for User Story 4

- [ ] T014 [US4] Extend `src/main.rs` CLI to thread the parsed `subscribed_topics` field through to `Node::new`. Concretely (depends on T007 + T008 + T009): after `load_node_config(path)?` returns the `NodeConfig`, construct `let initial_subscriptions: HashSet<TopicId> = config.subscribed_topics.iter().cloned().collect();` (the `HashSet` constructor absorbs duplicates per the "Return-shape contract on duplicates" in `contracts/library-api.md`); pass `initial_subscriptions` to `Node::new(self_id, config, initial_subscriptions, network).await?`. The CLI flag surface from 001 (`--self-id`, `--config`, `--log-level`) is unchanged per the spec's Assumptions ("No new CLI flag") and CHK001 / CHK010 design lock-in — DO NOT add a `--subscribed-topics` flag. The existing `--config <PATH>` flag's behavior on the rest of the file (`[[peers]]` etc.) is unchanged. Verify `cargo run -- --self-id node-a --config <path>` works against a TOML containing both `[[peers]]` and `subscribed_topics`. Per `contracts/node-config.toml.md` and FR-012.
- [ ] T015 [P] [US4] Extend `tests/config_loading.rs` with US4 acceptance scenarios (additive — 001's existing tests MUST keep passing under T006's NodeConfig rename and T010's fixture migration). New required tests:
  - `subscribed_topics_present_yields_initial_set` — US4 AS-1: write a TOML containing `[[peers]] id = "node-b"` and `subscribed_topics = ["a", "b"]` to a tempfile; call `load_node_config(path)?`; assert returned `NodeConfig.subscribed_topics` equals `vec!["a".parse()?, "b".parse()?]` (or as set).
  - `subscribed_topics_absent_yields_empty_set` — US4 AS-2: TOML with `[[peers]]` only, no `subscribed_topics`; load; assert `NodeConfig.subscribed_topics.is_empty()`.
  - `subscribed_topics_empty_array_yields_empty_set` — US4 AS-3: TOML with explicit `subscribed_topics = []`; load; assert empty (identical behavior to AS-2).
  - `invalid_topic_entry_yields_invalid_topic_error` — US4 AS-4: TOML with `subscribed_topics = ["valid", ""]`; load; assert `Err(ConfigError::InvalidTopic(_))` whose message contains both the path and the underlying `TopicIdError::Empty` rendering. A second sub-case with `subscribed_topics = ["valid", "bad\0topic"]` asserting `TopicIdError::ContainsNul` is REQUIRED.
  - `unknown_top_level_field_yields_parse_error` — US4 AS-5: TOML with `[[peers]] id = "node-b"`, `subscribed_topics = ["t1"]`, `unexpected_field = "value"`; load; assert `Err(ConfigError::Parse { … })` whose `toml::de::Error` mentions `unexpected_field` (the strict `deny_unknown_fields` invariant from 001 still holds).
  - `duplicate_subscribed_topic_yields_dedup_set` — US4 AS-6 / FR-010 + CHK025: TOML with `subscribed_topics = ["t1", "t2", "t1"]`; call `load_node_config(path)?`; assert returned `NodeConfig.subscribed_topics` is `vec!["t1", "t2", "t1"]` (retains the original Vec shape — per the Return-shape contract); then construct a Node with `initial_subscriptions = config.subscribed_topics.iter().cloned().collect::<HashSet<_>>()` and assert `node.subscriptions()` (as set) equals `{t1, t2}` (the HashSet boundary absorbs the duplicate). **Per the FR-010 / FR-014 test discipline (CHK027 / user direction during pass-1 walk): the test asserts ONLY on the dedup behavior — NOT on the warn log content.** The warn emission is operator UX, exercisable via quickstart §5.

**Checkpoint**: All four user stories are independently functional. The 002 feature is feature-complete pending Phase 7 polish.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Final hygiene sweep — fmt/clippy/build/test all green, rustdoc audit for new public surface, quickstart walkthrough end-to-end, FR coverage spot-check against `data-model.md §7.5`.

- [ ] T016 Run the green-checkpoint sweep from the crate root in this order: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo build`, `cargo test`. All four MUST pass. Per the saved feedback memory `feedback_cargo_fmt_per_commit` and Constitution §"Green checkpoints". Address any drift inline (formatting, clippy warnings, build errors, test failures) before committing the polish phase.
- [ ] T017 Walk `quickstart.md` end-to-end manually following SC-004's ≤1-hour budget for a fresh contributor. Execute every command in §§2–6 against a clean checkout of the 002 branch (or in an isolated worktree). Specifically: (a) verify `cargo test --test topic_filter` passes (US1 — T011); (b) verify `cargo test --test n_node_graph` passes (US2 — T012); (c) verify `cargo test --test topic_runtime` passes (US3 — T013); (d) verify `cargo test --test config_loading` passes (US4 — T015); (e) run the CLI binary against the example multi-topic TOML from §5 and observe normal startup; (f) run the CLI against the duplicate-topic TOML from §5 and observe the `event=topic_config_duplicate` warn entry; (g) run the CLI against the invalid-topic TOML and observe the `ConfigError::InvalidTopic` error + exit code 2. Update `quickstart.md` in-place if any command, expected output, or test name has drifted from the implementation reality. Per the SC-004 + CHK043 cohesion rule.
- [ ] T018 Rustdoc audit on the new 002 public surface: ensure every newly-public item has a top-of-item `///` doc comment describing behavior in stable, audience-appropriate terms (per the saved feedback memory `feedback_pubsub_node_doc_audiences`). Items to audit (per `contracts/library-api.md` "Re-exports from `pubsub_node` — additions" section): `TopicId`, `TopicIdError`, `MessagePayload` (and the rename of `Message` from enum to struct — the rustdoc on the struct should be re-authored, not inherited), `SubscribeOutcome`, `UnsubscribeOutcome`, `Node::subscribe`, `Node::unsubscribe`, `Node::subscriptions`, the renamed `Node::new` parameter `config: NodeConfig`, `NodeConfig` (and its `subscribed_topics` field), `load_node_config`, `ConfigError::InvalidTopic`. **Operator-facing-string convention applies**: no FR identifier or spec-section citations in rustdoc text (per `feedback_no_fr_citations_in_operator_strings` — rustdoc is consumed by library users, not spec readers). The internals (source `//` line comments, this `tasks.md` file, `data-model.md`, etc.) MAY cite FRs freely. Run `cargo doc --no-deps --document-private-items` and visually inspect the rendered HTML for completeness.
- [ ] T019 FR coverage spot-check against `data-model.md §7.5`. For each FR row in the matrix, verify the cited AS / test exists and asserts what the matrix claims. Specifically confirm: (a) every test name listed in §7.5's "Test-anchored coverage" column corresponds to a `#[tokio::test]` function actually present in the relevant test file; (b) `data-model.md §7.5`'s "not test-anchored at v1" entries (FR-014 mutation logs, FR-015 concurrent linearization) remain genuinely unexercised (no accidental coverage) — these are deliberate per CHK038 / CHK028 resolutions; (c) every SC row's "Test-anchored coverage" cell maps to an existing test or quickstart procedure. Update the matrix in `data-model.md §7.5` if any of these mismatch — the matrix is the canonical traceability surface for `/speckit-analyze` (the next phase) to consume, so it MUST be accurate before that phase runs.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No code dependencies — T001 only confirms the inherited 001 baseline is green.
- **Foundational (Phase 2)**: Depends on T001's green baseline. BLOCKS all user stories.
  - T002 (ADR) is independent — can run first or in parallel with T003 / T004.
  - T003 (`src/topic.rs`) and T004 (`ConfigError::InvalidTopic`) are independent of each other (different files) — can run in parallel.
  - T005 (Message envelope rewrite) depends on T003 (TopicId must exist before Message can carry one). Sequential against any task touching test files.
  - T006 (CHK017 rename in `src/config.rs`) is independent of T005 (different file scope) — could in principle run parallel, but in practice both touch `tests/common/mod.rs` so sequential is safer.
  - T007 (NodeConfig extension + loader) depends on T003, T004, T006.
  - T008 (Node extension) depends on T003, T005, T006, T007.
  - T009 (lib.rs new-re-export additions for the 002 types — the CHK017 rename of the existing config re-export line is owned by T006) depends on T003, T005, T006, T008.
  - T010 (fixture public-API extension + `assert_subscriptions` helper — the fixture's internal `Node::new` invocation was already adapted in T008's commit to keep `cargo test` green) depends on T009.
- **User Stories (Phase 3+)**: All depend on Foundational (Phase 2) completion. Within Phase 2's completion the four user stories are independently testable and can be implemented in parallel by different developers.
  - US1 (T011) is the MVP. Stop-and-validate point: ship-ready after T011.
  - US2 (T012), US3 (T013), and US4 (T014+T015) can be authored in parallel post-T010 since they touch different test files (and US4 additionally touches `src/main.rs`).
- **Polish (Phase 7)**: Depends on all user stories being complete.

### User Story Dependencies

- **US1 (P1)**: depends only on Foundational Phase 2.
- **US2 (P2)**: depends only on Foundational Phase 2. Does NOT depend on US1 — could be implemented first if a developer prefers.
- **US3 (P3)**: depends only on Foundational Phase 2.
- **US4 (P4)**: depends only on Foundational Phase 2 + the CLI extension (T014) which is sequential against `src/main.rs` only.

### Within Each Foundational Task

- Tests for `TopicId::from_str` rejection cases land **in** T003 (same task, same file) so the unit tests cover the validation rules from inception. Matches 001's T012 pattern for `PeerIdError`.
- Three Foundational tasks are explicit single-commit breaking changes that update every call site simultaneously — they MUST leave the crate at `cargo build` + `cargo test` green at task completion: the Message envelope rewrite (T005 — updates every Ping construction site to use `Message::ping(topic, n)`); the CHK017 rename (T006 — updates every caller of the renamed types + the `src/lib.rs` re-export line in the same commit); and the Node extension (T008 — also updates `tests/common/mod.rs` fixture builders' internal `Node::new` invocation in the same commit per pass-1 I1 resolution, so the new 4-arg signature doesn't leave the test build red).

### Parallel Opportunities

- Phase 2: T002, T003, T004 can run in parallel (independent files, no inter-task dependencies).
- Phase 3 / 4 / 5 / 6 (US tests): T011, T012, T013 can run in parallel post-T010. T015 can run in parallel with T011 / T012 / T013 post-T014.
- Phase 7: T016 / T017 / T018 / T019 are sequential (each builds on the previous's green state).

---

## Parallel Example: Foundational Phase

```bash
# Launch the three independent Foundational tasks together:
Task: "Author ADR 0008 in docs/decisions/0008-subscription-mutator-shape.md"
Task: "Create src/topic.rs with TopicId, TopicIdError, FromStr, validation"
Task: "Extend src/error.rs with ConfigError::InvalidTopic variant"
```

## Parallel Example: User Story Phases (post-T010)

```bash
# Different developers can pick up the three test-only US tasks in parallel:
Task: "Implement tests/topic_filter.rs (US1)"
Task: "Extend tests/n_node_graph.rs (US2)"
Task: "Implement tests/topic_runtime.rs (US3)"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: T001 baseline check.
2. Complete Phase 2: T002–T010 (the substrate — ADR + types + rename + Node extension + lib.rs + fixture).
3. Complete Phase 3: T011 (the US1 tests).
4. **STOP and VALIDATE**: Run `cargo test --test topic_filter` + manually verify the off-topic drop via quickstart §2 `--nocapture` invocation. This is the MVP.
5. Ship if ready.

### Incremental Delivery

1. Setup + Foundational → Foundation ready (Phase 2 checkpoint).
2. Add US1 (T011) → Validate independently → MVP ship-ready.
3. Add US2 (T012) → Validate independently.
4. Add US3 (T013) → Validate independently.
5. Add US4 (T014 + T015) → Validate independently.
6. Polish (T016–T019) → 002 feature-complete.

### Parallel Team Strategy

With multiple developers:

1. One developer takes Phase 1 + Phase 2 sequentially through T010 (the substrate is single-threaded because of breaking-rename ripples).
2. Once T010 commits, three developers can pick up:
   - Developer A: US1 (T011)
   - Developer B: US2 (T012)
   - Developer C: US3 (T013)
   - Developer D: US4 (T014 + T015)
3. Stories complete and integrate independently. Polish phase (T016–T019) runs after all stories merge.

---

## Notes

- [P] tasks = different files, no incomplete dependencies.
- [Story] label maps task to specific user story for traceability.
- Each user story should be independently completable and testable.
- Tests assert on `received_messages()`, `subscriptions()`, and `Outcome` return values — never on log content (per the FR-010 / FR-014 test discipline locked in CHK027 resolution).
- Commit after each task or logical group; the green-checkpoint invariant from Constitution §"Development Workflow" (every commit compiles and passes all non-ignored tests) applies throughout.
- The breaking-change tasks (T005, T006, T008) update many files simultaneously — these are the highest-risk green-checkpoint moments; bundle the call-site updates in the same commit as the type/function/signature change. (T005: Message envelope rewrite; T006: CHK017 rename; T008: Node extension + fixture-internal `Node::new` invocation update per pass-1 I1.)
- Stop at any Phase checkpoint (Phase 2 / Phase 3 / Phase 4 / Phase 5 / Phase 6) to validate independently; ship-ready after Phase 3 (MVP).
- Avoid: vague tasks, same-file conflicts on parallel runs, cross-story dependencies that break independence, assertions on log content (anti-pattern per CHK027).
