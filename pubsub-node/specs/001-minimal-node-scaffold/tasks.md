---

description: "Tasks: Minimal PubSub Node Scaffold"
---

# Tasks: Minimal PubSub Node Scaffold

**Input**: Design documents from `/specs/001-minimal-node-scaffold/`

**Prerequisites**: plan.md ✓, spec.md ✓, research.md ✓, data-model.md ✓, contracts/{cli,library-api,peer-list.toml}.md ✓, quickstart.md ✓

**Tests**: Tests at the acceptance-scenario level are MANDATORY for this feature (each User Story has a numbered Acceptance Scenario in spec.md that maps directly to an integration test). Strict red-green-refactor TDD is NOT required for this scaffold per `plan.md` Constitution Check §II ("This scaffold does not carry a protocol-behavior claim"). Tests are authored alongside the substrate they exercise. Where substrate and tests fall in the same task or phase, both land in the same commit; where they span phases (e.g., Phase 2 substrate exercised by Phase 3 US1 tests), substrate lands first in a green-test state, and tests are added in a follow-on commit that keeps the substrate's existing tests green. Either way, every commit MUST satisfy the project's green-checkpoint rule (Constitution §"Development Workflow") — no commit may break `cargo build` or `cargo test`.

**ADRs**: Seven structural decisions are crystallized as ADRs in Phase 2 from `research.md`'s ADR slot table. Authoring them upfront — rather than retroactively after implementation — keeps the audit trail aligned with Constitution Principle III and gives later phases concrete references to cite. ADRs go in `docs/decisions/NNNN-title.md`.

**Organization**: Tasks are grouped by user story. The substrate library (`src/lib.rs` and its modules — except `src/config.rs` and `src/main.rs`) is **foundational** because all three user stories depend on it. US1 is a thin test-only phase that demonstrates the substrate composes correctly (its purpose per spec.md §US1 "Why this priority"). US2 reuses the substrate with no new `src/` code. US3 adds the file-loading + CLI layer.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Different files, no incomplete dependencies — can run in parallel
- **[Story]**: US1 / US2 / US3 — user-story phase tasks only
- File paths are absolute from the crate root (`pubsub-node/`)

## Path Conventions

- **Single Cargo crate** (lib + bin) per `plan.md` "Project Structure"
- Source: `src/`
- Integration tests: `tests/`
- ADRs: `docs/decisions/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Cargo crate skeleton, dependency declarations, lint configuration.

- [ ] T001 Initialize the Cargo crate as `pubsub-node` (lib + bin) at the crate root: create `Cargo.toml` with `name = "pubsub-node"`, `version = "0.1.0"`, `edition = "2021"`, `rust-version = "1.75"`, both `[lib]` and `[[bin]]` sections (the bin's `name` defaults to the crate name). Create stub `src/lib.rs` (empty `// placeholder`), stub `src/main.rs` (empty `fn main() {}`), and `docs/decisions/` directory. Verify `cargo build` succeeds on the empty crate.
- [ ] T002 Add dependencies to `Cargo.toml` per `plan.md` Technical Context: `tokio = { version = "1", features = ["macros", "rt-multi-thread", "sync", "time", "signal"] }`, `serde = { version = "1", features = ["derive"] }`, `toml = "0.8"`, `tracing = "0.1"`, `tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }`, `clap = { version = "4", features = ["derive"] }`, `thiserror = "1"`. Add `[dev-dependencies]` block reserved for test-only crates (initially empty). Verify `cargo build` still succeeds.
- [ ] T003 [P] Configure Clippy lint level in `Cargo.toml` `[lints]` table (deny `rust_2018_idioms`, warn `clippy::pedantic`, allow `clippy::module_name_repetitions`). Run `cargo clippy --all-targets -- -D warnings` and verify it passes on the empty crate.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: All shared substrate the three user stories depend on — every type from `data-model.md` except `PeerListConfig` (US3) and the CLI binary (US3), plus the seven ADRs that capture the structural decisions backing the substrate.

**⚠️ CRITICAL**: No user-story phase work can begin until this phase completes.

### ADRs (Principle III deliverables)

Each ADR transcribes the corresponding `research.md` section into the standard `Context / Decision / Consequences / Alternatives` ADR shape. Reference the section in the ADR body so the source-of-truth chain is visible.

- [ ] T004 [P] Author ADR 0001 in `docs/decisions/0001-async-runtime-tokio.md` from `research.md` §1 (async runtime: tokio).
- [ ] T005 [P] Author ADR 0002 in `docs/decisions/0002-toml-via-serde.md` from `research.md` §2 (config via serde + toml).
- [ ] T006 [P] Author ADR 0003 in `docs/decisions/0003-logging-via-tracing.md` from `research.md` §3 (structured logging via tracing + tracing-subscriber).
- [ ] T007 [P] Author ADR 0004 in `docs/decisions/0004-cli-via-clap.md` from `research.md` §4 (CLI via clap derive).
- [ ] T008 [P] Author ADR 0005 in `docs/decisions/0005-typed-errors.md` from `research.md` §5 (typed errors via thiserror; no anyhow in library).
- [ ] T009 [P] Author ADR 0006 in `docs/decisions/0006-receive-task-and-registration.md` from `research.md` §6 + §8 (recv-task model conjoined with registration timing).
- [ ] T010 [P] Author ADR 0007 in `docs/decisions/0007-network-handle-actor-pattern.md` from `research.md` §12 (NetworkHandle as actor-handle — tx/rx split, channels over callbacks). Re-walk and pin URLs at authoring time per the "Note on URL stability" in research §12.

### Substrate types

Each module's file path, public type list, derived traits, invariants, and FR trace are normative in `data-model.md` §1–§7. Implement to that spec.

- [ ] T011 [P] Implement error types in `src/error.rs`: `ConfigError { Io { path, source }, Parse { path, source }, InvalidPeer(String) }`, `NetworkError { DuplicateRegistration(PeerId) }`, `NodeError { Network(#[from] NetworkError) }`. All `#[derive(Debug, thiserror::Error)]` with the `#[error("...")]` strings per `data-model.md` §7.
- [ ] T012 [P] Implement `src/peer.rs`: `pub struct PeerId(String)` with `Display`, `Debug`, `Eq`, `PartialEq`, `Hash`, `Clone`, `FromStr` (rejecting empty / NUL-containing strings with a `PeerIdError::{Empty, ContainsNul}` enum), `serde::Deserialize`, `serde::Serialize`. `pub trait PeerDescriptor: Clone + Send + Sync + 'static { fn id(&self) -> &PeerId; }`. `pub struct BasicPeerDescriptor { pub id: PeerId }` implementing `PeerDescriptor`. Per `data-model.md` §1 and contracts/`library-api.md` `PeerId` section. Include unit tests for `PeerId::from_str` rejection cases (empty, internal NUL) — these are the only Phase-2 unit tests. **Error-location note**: `PeerIdError` lives in `src/peer.rs` (not in `src/error.rs`) by deliberate choice — parse errors are co-located with the type they parse, following the `std::num::ParseIntError` pattern. The broader policy is documented in `data-model.md` §8: cross-module errors (`ConfigError` / `NetworkError` / `NodeError` per T011) centralise in `src/error.rs`; `PeerIdError` is the documented exception. An implementer should NOT consolidate `PeerIdError` into `src/error.rs`.
- [ ] T013 [P] Implement `src/message.rs`: `#[non_exhaustive] pub enum Message { Ping(u64) }` with `Clone`, `Debug`, `Eq`, `PartialEq`. Per `data-model.md` §2.
- [ ] T014 [P] Implement `src/received.rs`: `pub struct ReceivedDelivery { pub from: PeerId, pub message: Message }` with `Clone`, `Debug`, `Eq`, `PartialEq`. Per `data-model.md` §6.
- [ ] T014a [P] Implement `src/config.rs` (types only) per `data-model.md` §3: `pub struct PeerEntry { pub id: PeerId }` and `pub struct PeerListConfig { pub peers: Vec<PeerEntry> }`, both with `#[derive(Debug, Clone)]`. No `serde` derives yet, no loader function. Defines the real-shape type so Phase 2 substrate code can reference it directly; `serde::Deserialize` + `load_peer_list` are added additively in T022 (Phase 5). Per `data-model.md` §3 and contracts/`peer-list.toml.md` schema.
- [ ] T015 Implement `src/network.rs` (single file; the types are tightly coupled): `pub(crate) struct Envelope { from: PeerId, message: Message }`; `pub trait Network: Send + Sync + 'static` with `async fn register(&self, id: PeerId) -> Result<NetworkHandle, NetworkError>`; `#[derive(Clone)] pub(crate) struct NetworkSender` (wraps an `Arc<RwLock<HashMap<PeerId, UnboundedSender<Envelope>>>>` registry); `pub struct NetworkHandle { self_id, tx: NetworkSender, rx: UnboundedReceiver<Envelope> }` with `pub fn id`, `pub async fn send(&self, to: &PeerId, message: Message) -> Result<(), NetworkError>` (drop + `tracing::warn!(target = "pubsub_node::network", peer_id = %to, "send dropped: unregistered peer id")` on unregistered id per FR-010 / CHK020 / CHK036), `pub(crate) fn take_receiver(&mut self) -> UnboundedReceiver<Envelope>`; `pub struct InMemoryNetwork` with `pub fn new() -> Self`, `impl Network for InMemoryNetwork` registering in the registry hashmap and creating the per-peer mpsc. Emit `tracing::debug!` for `send.accepted`. Depends on T011–T014. Per `data-model.md` §4 and contracts/`library-api.md` Network / NetworkHandle sections.
- [ ] T016 Implement `src/node.rs`: `pub struct Node { handle: NetworkHandle, peers: Vec<BasicPeerDescriptor>, received: Arc<Mutex<Vec<ReceivedDelivery>>>, recv_task: tokio::task::JoinHandle<()> }`. `pub async fn new(self_id: PeerId, peer_list: PeerListConfig, network: Arc<dyn Network>) -> Result<Self, NodeError>` — call `network.register(self_id)`, then `handle.take_receiver()`, spawn the recv task that loops `rx.recv().await` and pushes a `ReceivedDelivery` into `received` (under the `Mutex`) with `tracing::debug!(target = "pubsub_node::node", from = %env.from, "recv")` on each receipt. The constructor maps `peer_list.peers` (a `Vec<PeerEntry>` after T014a) into the Node's `peers: Vec<BasicPeerDescriptor>` field by extracting each entry's `id` into a `BasicPeerDescriptor`. `pub async fn send(&self, to: &PeerId, message: Message) -> Result<(), NodeError>` forwards through `self.handle.send`. `pub fn id`, `pub fn peers`, `pub fn received_messages(&self) -> Vec<ReceivedDelivery>` (snapshot clone per FR-006 / CHK019). `impl Drop` aborts `recv_task`. Uses the real `PeerListConfig` from `src/config.rs` (T014a) directly — no stub indirection. Depends on T011–T015 (T014a in particular). Per `data-model.md` §5.
- [ ] T017 Wire `src/lib.rs` re-exports per `data-model.md` §8 (form (a): explicit top-level re-exports for every public type, including errors): `pub use peer::{PeerId, PeerIdError, PeerDescriptor, BasicPeerDescriptor}; pub use message::Message; pub use network::{Network, NetworkHandle, InMemoryNetwork}; pub use received::ReceivedDelivery; pub use config::{PeerEntry, PeerListConfig}; pub use node::Node; pub use error::{ConfigError, NetworkError, NodeError};`. The `load_peer_list` function is *not* re-exported here — it lands additively in T022. Callers reach errors via the flat namespace (`pubsub_node::ConfigError`, etc.) — consistent with how every other public type is re-exported. Add `#![forbid(unsafe_code)]` at the crate root and a top-of-file module doc-comment that points to `specs/001-minimal-node-scaffold/`. Depends on T011–T016 (including T014a). Verify `cargo build` is green.
- [ ] T018 Implement `tests/common/mod.rs` per `data-model.md` §10: `pub struct TwoNodeFixture { pub network: Arc<InMemoryNetwork>, pub a: Node, pub b: Node }`; `pub async fn two_node_fixture() -> TwoNodeFixture` (constructs the network and two nodes with ids `"node-a"` / `"node-b"`, A's peer set = {B}, B's peer set = {A}); `pub async fn await_delivery(node: &Node, expected_sender: &PeerId, expected_message: &Message, timeout: Duration) -> Result<(), AwaitError>` polling `node.received_messages()` every 1 ms until a matching entry appears or the budget is exhausted; `pub enum AwaitError { Timeout(Duration) }` with `thiserror::Error`. Depends on T017. Per contracts/`library-api.md` "Test-harness contract" section.

**Checkpoint**: Substrate is complete. `cargo build` and `cargo test` green (no tests yet beyond `PeerId::from_str` rejection cases). User-story phases can start.

---

## Phase 3: User Story 1 — Two-Node Ping Exchange via InMemory Network (Priority: P1) 🎯 MVP

**Goal**: Demonstrate that two Nodes attached to a shared InMemory network can exchange a `Ping(N)` and that the recipient observes it via `received_messages()`. This is the irreducible MVP proof of the scaffold (spec §US1 "Why this priority").

**Independent Test**: Run `cargo test --test two_node_ping`. All 4 tests pass within a few seconds; the test names map 1:1 to US1 AS-1 / AS-2 / AS-3 and SC-005.

### Implementation for User Story 1

- [ ] T019 [US1] Write integration tests in `tests/two_node_ping.rs` covering the three US1 acceptance scenarios. Required tests (each `#[tokio::test]`, each using `mod common;` to access `two_node_fixture()` and `await_delivery`):
  - `ping_delivered_when_a_lists_b` — US1 AS-1: `a.send(b.id(), Message::Ping(42)).await?; await_delivery(&b, a.id(), &Message::Ping(42), Duration::from_secs(1)).await?; assert_eq!(b.received_messages().len(), 1);`
  - `ping_delivered_trust_on_arrival` — US1 AS-2: B's peer set contains NO peers; A's contains B; A sends `Ping(7)`, B still receives (per FR-003).
  - `empty_peer_set_cannot_originate` — US1 AS-3 + spec Edge Cases bullet 1: Node A with empty peer list. Calling `a.send(&PeerId::from_str("ghost").unwrap(), Message::Ping(0))` resolves `Ok(())` (per FR-010 drop-on-unregistered), no panic, no undefined state. Verify ghost's id appears in `tracing` output (use `tracing-test` or capture stderr) if reasonable; otherwise just assert behaviour is observable.
- [ ] T020 [US1] Add the SC-005 / FR-013-falsifiability test to `tests/two_node_ping.rs`: `ping_n_intact_across_100_sends` — uses a deterministic sequence `0u64..100` (record the choice in an in-file comment per CHK056), sends each `Ping(i)` from A to B in order awaiting `await_delivery` for each, then asserts (a) no duplication — `b.received_messages().len() == 100` — and (b) no loss — every `i` in `0..100` appears exactly once as a `ReceivedDelivery { from: a.id().clone(), message: Message::Ping(i) }` (per CHK057's two-mode falsifiability rule). Test name and assertion shape are stable contract per quickstart.md §4.

**Checkpoint**: US1 is independently functional. The scaffold has proven the substrate composes. **This is the MVP** — could ship here.

---

## Phase 4: User Story 2 — N-Node Graph via Per-Node Configuration (Priority: P2)

**Goal**: Show the InMemory network multiplexes correctly across more than two participants, and that a node's outbound peer set is independent of its inbound traffic (US2 AS-1 + AS-2). No new `src/` code — reuses the substrate.

**Independent Test**: Run `cargo test --test n_node_graph`. Both tests pass within a few seconds.

### Implementation for User Story 2

- [ ] T021 [US2] Write integration tests in `tests/n_node_graph.rs` covering both US2 acceptance scenarios. Use `mod common;` and add a `four_node_star_fixture()` helper inline in this test file (or extended to `tests/common/mod.rs` if it grows shared use — initial home is this file to keep US1 independence intact). Required tests:
  - `four_node_star_isolates_addressed_pings` — US2 AS-1: A's peer set = {B, C, D}; B, C, D have empty peer sets. A sends `Ping(1)` to B, `Ping(2)` to C, `Ping(3)` to D (sequentially, awaiting each delivery). Assert: B's record = `[ReceivedDelivery { from: A, message: Ping(1) }]`, C's = `[Ping(2) from A]`, D's = `[Ping(3) from A]`. No cross-talk.
  - `inbound_traffic_independent_of_outbound_peer_set` — US2 AS-2: Same 4-node graph. Extend the fixture so B, C, D each `send` a `Ping` addressed to A (their peer sets are empty, but FR-003 trust-on-arrival means A receives anyway; the InMemory network routes by registered id, not by the sender's peer set). Assert A's `received_messages()` contains all 3 pings, attributed correctly to B, C, D.
  - `four_node_star_100_send_isolation` — SC-002 conjunction (4-node graph + 100 sequential sends + isolation). Same star fixture as AS-1. Send 100 pings from A round-robin across {B, C, D} with unique `N` values: `for i in 0..100u64 { let target = match i % 3 { 0 => &b, 1 => &c, _ => &d }; a.send(target.id(), Message::Ping(i)).await?; }`. After awaiting all deliveries via `await_delivery`, assert: each of B/C/D's `received_messages()` is exactly the set of `Ping(i)` for `i ≡ 0/1/2 (mod 3)` respectively, all attributed to A; no peer's record contains a Ping from outside its slice; the three record sizes sum to 100; no duplicates within any record. The deterministic `0..100` sequence is the SC-005-style reproducibility hook (CHK056) applied to SC-002.

**Checkpoint**: US1 and US2 are both independently functional.

---

## Phase 5: User Story 3 — Peer Descriptors Loaded from a Config File (Priority: P3)

**Goal**: Move peer-set authoring out of source code into a TOML config file, and expose a CLI binary that loads + validates the config and constructs a Node (US3 AS-1 + AS-2).

**Independent Test**: Run `cargo test --test config_loading` (both tests pass). Run `cargo run -- --self-id node-a --config /tmp/valid-peers.toml` and observe the binary parks until Ctrl-C; run with a malformed TOML and observe exit code 2 with a clear error on stderr.

### Implementation for User Story 3

- [ ] T022 [US3] Extend `src/config.rs` (introduced as types-only in T014a) additively: add `#[derive(serde::Deserialize)]` to `PeerEntry` together with `#[serde(deny_unknown_fields)]` (per peer-list.toml.md Forward-compatibility note), and add `#[derive(serde::Deserialize)]` plus `#[serde(default)]` on `peers` to `PeerListConfig`. Implement `pub fn load_peer_list(path: &Path) -> Result<PeerListConfig, ConfigError>` performing the three-stage pipeline (read → `toml::from_str` → re-validate each `PeerId` via `FromStr`) and mapping failures to `ConfigError::{Io, Parse, InvalidPeer}` with `path` populated. Update `src/lib.rs` re-exports to expose `pub use config::load_peer_list;` (the `PeerEntry` and `PeerListConfig` re-exports already exist from T017). **No `src/node.rs` changes**: the type was already real at Phase 2 time, so existing US1/US2 test call sites and the `two_node_fixture` keep working unchanged.
- [ ] T023 [US3] Write integration tests in `tests/config_loading.rs` covering both US3 acceptance scenarios. Each test creates its TOML in a `tempfile::tempdir()` (add `tempfile` to `[dev-dependencies]`). Required tests:
  - `loads_three_peer_descriptors_from_toml` — US3 AS-1: write a TOML with three `[[peers]]` entries (`"node-b"`, `"node-c"`, `"node-d"`); call `load_peer_list(path)`; assert returned `PeerListConfig.peers` has length 3 with ids in declaration order.
  - `malformed_toml_yields_actionable_error` — US3 AS-2 + FR-001 + CHK047: three sub-cases that each must produce a distinct `ConfigError` variant — (1) syntactically invalid TOML (e.g., unclosed `[[peers]`) → `ConfigError::Parse`; (2) `id = ""` → `ConfigError::InvalidPeer`; (3) `path` to a non-existent file → `ConfigError::Io`. For each, assert the error's `Display` chain includes the offending path and (where applicable) the line/column information surfaced by `toml::de::Error`.
- [ ] T024 [US3] Implement `src/main.rs` per contracts/`cli.md`. Use `#[derive(clap::Parser)]` for `Args { #[arg(long)] self_id: PeerId, #[arg(long)] config: PathBuf, #[arg(long, default_value = "info")] log_level: tracing::Level }`. Initialize `tracing_subscriber` with the `log_level` env filter on stderr. Pipeline: `let args = Args::parse(); init tracing; let cfg = load_peer_list(&args.config).map_err(|e| { eprintln!("pubsub-node: {e}"); std::process::exit(2) })?; let network = Arc::new(InMemoryNetwork::new()); let node = Node::new(args.self_id, cfg, network).await.map_err(|e| { eprintln!("pubsub-node: {e}"); std::process::exit(1) })?; tokio::signal::ctrl_c().await?; drop(node); Ok(())`. Map exit codes per contracts/`cli.md` (0 / 1 / 2 / 64). The `#[tokio::main]` macro wraps `main`. Verify the binary builds (`cargo build`), CLI help prints (`cargo run -- --help`), and the malformed-config path emits exit code 2.

**Checkpoint**: All three user stories are independently functional. The scaffold is complete.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: documentation, lint/format gate, end-to-end quickstart validation, and FR coverage verification.

- [ ] T025 [P] Add rustdoc comments to the public re-exports in `src/lib.rs`: a crate-level doc-comment pointing readers at `specs/001-minimal-node-scaffold/`, plus a one-paragraph `///` doc on each re-exported item describing its role and pointing at the relevant FR(s) it realises (per `data-model.md` §9 cross-reference matrix). Run `cargo doc --no-deps --open` locally and verify the rendered docs are coherent.
- [ ] T026 Run the green-checkpoint gate: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all`. Fix any failures. Constitution §"Green checkpoints" requires this to pass before the implementation phase closes.
- [ ] T027 Walk `quickstart.md` end-to-end manually (timer running) — build, run all three integration test files, run the CLI with both valid + malformed TOML, verify exit codes and error chain rendering per contracts/`cli.md`. Confirm SC-001 (≤30s test runtime), SC-002 (100-send / 4-node tests pass), and SC-004 (the walkthrough completed under 1 hour with only this spec dir's contents as input).
- [ ] T028 [P] Verify FR coverage: walk `data-model.md` §9's cross-reference matrix and confirm each FR (FR-001 through FR-013) is realised by at least one implementation task + (where applicable) at least one test from Phases 3–5. Any FR without a passing implementation+test pair is a regression that must be addressed before this task closes. Record verification in a short comment in `tasks.md` (or a tracking note in the closing commit).

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies — starts immediately.
- **Foundational (Phase 2)**: depends on Setup. **Blocks all user stories.**
- **US1 (Phase 3)**: depends on Foundational. No dependency on US2 or US3.
- **US2 (Phase 4)**: depends on Foundational. No dependency on US1 (independent test file, independent fixtures).
- **US3 (Phase 5)**: depends on Foundational. The `PeerListConfig` type is already defined in its final shape in Phase 2 (T014a), so no stub-swap is required; T022 only extends `src/config.rs` additively with `serde::Deserialize` derives and the `load_peer_list` loader function. US1 and US2 tests use the same `PeerListConfig` shape throughout — Phase 2 and Phase 5 share one type definition.
- **Polish (Phase 6)**: depends on all desired user stories being complete.

### Within Each Phase

- **Phase 2**: ADRs (T004–T010) all [P]. Substrate types (T011–T014a) all [P] — 5 files: error / peer / message / received / config-types. T015 (network.rs) depends on T011–T014 only — **not** T014a, since network.rs does not reference `PeerListConfig`. T016 (node.rs) depends on T011–T015 **and** T014a (Node::new consumes `PeerListConfig`). T017 (lib.rs re-exports) depends on T011–T016 (including T014a). T018 (tests/common) depends on T017.
- **Phase 3 (US1)**: T019 then T020 (same file).
- **Phase 4 (US2)**: T021 (single task).
- **Phase 5 (US3)**: T022 (config.rs) first; T023 (config tests) depends on T022; T024 (main.rs) depends on T022.
- **Phase 6**: T025 [P] with T028 [P]. T026 must pass before T027. T027 sequential.

### Parallel Opportunities

- **Phase 1**: T003 [P] alongside T002 (lint config is independent of the dep list).
- **Phase 2 ADRs**: T004–T010 are seven independent files — all [P]. With seven workers, the ADR slice completes in one round.
- **Phase 2 substrate**: T011, T012, T013, T014, T014a all [P] (five independent files: error / peer / message / received / config-types). T015 / T016 / T017 / T018 are sequential due to dependencies.
- **Phase 3, 4, 5**: once Foundational is done, all three story phases can begin in parallel by different developers (US1 / US2 / US3 each own distinct test files and, for US3, distinct `src/` files).

---

## Parallel Example: Phase 2 substrate

```bash
# Once T011–T014a are scheduled, five workers in parallel:
Worker A: T011 — src/error.rs (error enums)
Worker B: T012 — src/peer.rs (PeerId + PeerDescriptor + BasicPeerDescriptor)
Worker C: T013 — src/message.rs (Message enum)
Worker D: T014 — src/received.rs (ReceivedDelivery)
Worker E: T014a — src/config.rs (PeerEntry + PeerListConfig, types-only)

# Then sequentially:
Worker A: T015 — src/network.rs (Envelope + Network + NetworkHandle + NetworkSender + InMemoryNetwork)
Worker A: T016 — src/node.rs (Node + recv task + Drop)
Worker A: T017 — src/lib.rs re-exports
Worker A: T018 — tests/common/mod.rs (TwoNodeFixture + await_delivery)
```

---

## Implementation Strategy

### MVP First (US1 only)

1. Complete Phase 1: Setup (3 tasks).
2. Complete Phase 2: Foundational (16 tasks — 7 ADRs + 9 substrate).
3. Complete Phase 3: US1 (2 tasks).
4. **STOP and VALIDATE**: `cargo test --test two_node_ping` passes; the scaffold has proven the substrate works. Could ship here — `plan.md`'s SC-001 / SC-005 are both satisfied at this point.

### Incremental Delivery

1. Setup + Foundational + US1 → MVP. Land + commit + tag.
2. Add US2 (1 task) → N-node generality demonstrated. Land + commit.
3. Add US3 (3 tasks) → CLI + TOML loading shipped. Land + commit.
4. Polish (4 tasks) → green gate, quickstart walked, docs rendered, FR coverage verified.

### Parallel Team Strategy

With two developers and the substrate complete:
- Dev A: US1 + US2 (both are test-only; minimal coordination needed)
- Dev B: US3 (config.rs + tests + main.rs is a vertical slice)
- Both converge on Phase 6.

---

## Notes

- Each task should close as a single logical commit per Constitution §"Logical increments". The commit message should reference the task ID and the FR / Acceptance Scenario being realised.
- Every commit must compile and pass `cargo test` per Constitution §"Green checkpoints". There is no stub-replacement step — the `PeerListConfig` type is introduced in its final shape at T014a, and T022 extends it additively with `serde::Deserialize` derives + the loader function.
- ADR tasks (T004–T010) are crystallizations of decisions already documented in `research.md` — they don't introduce new structural choices, they record existing ones in the standard ADR format. Authoring them upfront is cheap and satisfies Principle III before the structural decisions are baked into code.
- Do **not** edit `../formal_spec/`, `../docs/`, or `../docs/extensions/` during any task (Constitution Principle V).
- Tests use the `cargo test` runtime — no separate test runner is required.
- The `tests/common/mod.rs` pattern: each test file in `tests/` that uses the shared harness declares `mod common;` at the top. Rust treats files under `tests/common/` as a non-test module shared across integration test binaries.
- After T024 (main.rs), the binary parks on Ctrl-C indefinitely; there is intentionally no "send a Ping" CLI subcommand at this stage (Ping origination is exclusively a library-level / integration-test concern in v1, per contracts/`cli.md` "Out of scope for v1").
