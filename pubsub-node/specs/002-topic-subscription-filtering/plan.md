# Implementation Plan: Topics + Topic-Subscription Filtering

**Branch**: `002-topic-subscription-filtering` | **Date**: 2026-05-30 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/002-topic-subscription-filtering/spec.md`

## Summary

Layer a topic dimension onto 001's substrate: every `Message` gains a `topic` field (`TopicId`, opaque newtype parallel to `PeerId`); every `Node` tracks a mutable in-memory `HashSet<TopicId>` of subscribed topics; the receive task filters by membership and silently drops off-topic deliveries with an info-level structured log; the existing `InMemoryNetwork` is untouched (still a dumb pipe routing by `peer_id`). Adds runtime mutators `subscribe` / `unsubscribe` (sync, idempotent, return outcome enums), a `subscriptions()` snapshot getter, info/debug structured logs on mutation events, and a `subscribed_topics` top-level field in the peer-list TOML.

Technical approach (per the spec's Clarifications session 2026-05-29):

- **Parallel-to-PeerId TopicId**: `TopicId` is a UTF-8 `String` newtype with `FromStr`, validated by the same rules as `PeerId` (non-empty, no internal NUL) — no extra character-class restrictions; naming structure (namespacing, scoping) is deferred to the registry feature. Implementation lives in `src/topic.rs`, mirroring `src/peer.rs`'s shape.
- **Linearizable subscription set**: `Node` carries an `Arc<Mutex<HashSet<TopicId>>>` mirroring 001's `Arc<Mutex<Vec<ReceivedDelivery>>>` pattern for `received` (`src/node.rs:27`). FR-015's linearizability contract is satisfied trivially by mutex serialization; the receive task and the mutator API both acquire the same lock. Other primitives (`RwLock`, lock-free) are spec-compatible but the `Mutex` choice matches the precedent and is the only one needed at v1 scale.
- **Message envelope grows by composition, not enum-variant rewrite**: today's `Message::Ping(N)` enum gains a topic field via wrapping. The concrete shape (a struct `Message { topic: TopicId, payload: Ping }` vs an envelope `EnvelopedMessage { topic: TopicId, message: Message }`) is resolved in `research.md` §1; whichever shape lands, the topic is a first-class field, not embedded in a variant.
- **Receive-side filter in the Node**: the recv_task body inspects topic-set membership against the subscription set guarded by the lock, then either pushes to `received` or skips + emits the FR-011 info log. The InMemoryNetwork is untouched per FR-005.
- **Sync mutators on async Node**: `subscribe(&self, topic) -> SubscribeOutcome` and `unsubscribe(&self, topic) -> UnsubscribeOutcome` are synchronous `fn` (not `async fn`) — the body acquires the mutex briefly and returns. They share the same lock as the receive task's filter check, satisfying FR-015 without an additional synchronization primitive. The async Node's other methods (send/receive) are unchanged in shape per FR-007.
- **Snapshot getter mirrors `received_messages()`**: `subscriptions(&self) -> Vec<TopicId>` clones the HashSet's contents under the lock. Entry order in the returned `Vec` is unspecified (set semantics); tests assert against it as a set.
- **Parse at the edge**: the TOML loader gains a `subscribed_topics` field (top-level, optional, string array, deny-unknown applies). `load_peer_list` parses into a new `PeerListConfig.subscribed_topics: Vec<TopicId>` field via `TopicId::FromStr`. The Node constructor takes the parsed `HashSet<TopicId>` as an in-memory argument; the binary's CLI does the file I/O (matches 001 FR-012).
- **One new ADR**: structural decision around subscription mutator shape (sync `&self` mutators with interior mutability, linearizable per FR-015) gets ADR 0008. The other deltas are tactical extensions of 001's existing structure and don't warrant new ADRs.

## Technical Context

**Language/Version**: Rust 1.75+ stable (edition 2021) — unchanged from 001. No new toolchain requirement.

**Primary Dependencies**: unchanged from 001. The five deps (`tokio`, `serde` + `toml`, `tracing` + `tracing-subscriber`, `clap`, `thiserror`) carry through with no version bump. No new crate is required — the subscription set is `std::collections::HashSet`, the lock is `std::sync::Mutex`, the new TOML field rides inside the existing `serde::Deserialize` derive.

**Storage**: N/A — single-process, in-memory only. Subscription state is not persisted across Node restarts (Assumptions section of spec).

**Testing**: `cargo test` with `#[tokio::test]` for async integration tests, unchanged from 001. The await-on-delivery primitive from 001 (`tests/common/mod.rs::await_delivery`) is the canonical test seam for US1 / US2 / US3 / US4 acceptance scenarios — the receive task processes a message asynchronously after `send().await` resolves, so any "subsequently observable" assertion (including post-mutation filter behavior in US3) uses `await_delivery` to avoid races.

**Target Platform**: Linux + macOS developer workstations — unchanged from 001.

**Project Type**: Single Cargo crate, library + binary — unchanged from 001. The 002 deltas extend `src/` rather than restructure it.

**Performance Goals**: None at this stage. Spec SC-001's 30-second wall-clock budget for US1 is trivially satisfied. SC-002's 100-send / 3-topic cross-cut is a correctness assertion, not a performance one.

**Constraints**:

- Single-process scope (inherits from 001).
- No cryptographic operations (inherits from 001; explicitly deferred to 003).
- Network unchanged (FR-005) — topic filtering is strictly receive-side, not transport-side.
- Snapshot semantics: `received_messages()` (001 FR-006) and `subscriptions()` (FR-013) both return owned, race-free snapshots.
- Linearizability across filter check, mutators, and snapshot getter (FR-015).
- Logging facility: same `tracing` stack as 001; 002 adds info-level events for state-changing mutations and topic drops, and debug-level events for idempotent no-op mutations (FR-014). 001's default log level (`info`, per `contracts/cli.md:17`) surfaces both info and warn events without explicit configuration; FR-014's debug events are invisible at the default and reachable with `--log-level debug`.

**Scale/Scope**: 2 ≤ N ≤ 10 nodes per demonstration (US2 inherits 001's bound). At least 3 topics in the US2 / SC-002 cross-cut. At least 100 sequential emissions across at least 3 topics per SC-002. Subscription set size per node is unbounded by spec; in practice US1–US4 exercise sets of 0–3 topics.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Evaluated against `.specify/memory/constitution.md` v1.0.0.

### Initial gate (before Phase 0)

- **I. Correctness Over Optimization** — ✅ **pass**. Every plan claim traces to a numbered FR in `spec.md` or a bullet in `## Clarifications`. No optimization-led decisions; the in-memory mutex pattern is deliberately the least-clever option that satisfies FR-015.
- **II. Test-Driven for Correctness Claims** — ✅ **pass (with note)**. 002 does **not** carry a protocol-behavior claim (it is an in-memory data-shape extension + a receive-side filter; no cryptographic verification, no chain semantics, no registry interaction — those are 003 / 008 / 012). Tests remain mandatory at the acceptance-scenario level (US1 / US2 / US3 / US4) but `cargo test` integration suites suffice; strict red-green-refactor TDD is reserved for the protocol-critical features starting at 003 per the constitution's "Complex or critical features" carve-out. `/speckit-tasks` will preserve this rule for downstream features even though it relaxes here.
- **III. Document Structural Decisions as ADRs** — ⚠ **at-risk → mitigated**. One new structural decision is introduced by 002 — the sync-mutator-with-interior-mutability shape for `subscribe` / `unsubscribe` on an async Node (locks the public API surface for the lifetime of the feature; reversing it would touch every caller). Slot: ADR 0008 (filename `docs/decisions/0008-subscription-mutator-shape.md`). The other 002 deltas (TopicId parallel to PeerId; subscription set as `Arc<Mutex<HashSet<TopicId>>>` mirroring `received`'s `Arc<Mutex<Vec<…>>>`; topic field on Message; TOML schema extension) are tactical extensions of 001's existing shape — direct application of patterns already covered by ADRs 0001–0007, not new structural decisions. Mitigation: ADR 0008 is tracked as a deliverable in `/speckit-tasks`, not deferred indefinitely.
- **IV. Specifications as Ambiguity Detectors** — ✅ **pass**. Four ambiguities surfaced during `/speckit-clarify` (subscriptions getter, TopicId char-class scope, mutation logging, concurrency contract) are recorded as Q/A bullets in spec.md's Clarifications section and encoded as FR-013 / FR-014 / FR-015. Plan-level items the spec deferred (concurrency primitive choice, Message envelope shape, `subscriptions()` return type, ADR scope) are addressed in `research.md` with explicit Decision / Rationale / Alternatives. None are silently resolved in code.
- **V. Specifications Are Read-Only** — ✅ **pass**. This plan does not propose edits to `../formal_spec/`, `../docs/`, or `../docs/extensions/`. Files touched: `specs/002-topic-subscription-filtering/{plan.md, research.md, data-model.md, contracts/*.md, quickstart.md}` (agent-editable Spec-Kit artifacts), `CLAUDE.md` (agent context, not a protocol specification), `docs/decisions/0008-…` (new ADR, code-side). The spec itself was edited only during `/speckit-specify` and `/speckit-clarify`, both of which are explicit spec-authoring phases.

Engineering Standards specifically engaged:

- *Observable state transitions* — Node emits `tracing` events for: subscribe state-change (info), unsubscribe state-change (info), subscribe idempotent no-op (debug), unsubscribe idempotent no-op (debug), off-topic drop (info). All five carry the receiver's own peer id and the topic; the drop event additionally carries the sender's peer id. Event names are stable strings (e.g., `topic_subscribed`, `topic_unsubscribed`, `topic_drop`) suitable for operator grep.
- *Justified dependencies* — no new dependencies are added by 002. The Rust stdlib (`HashSet`, `Mutex`) and existing 001 deps cover all needs. ADR 0008 covers the one structural choice; no new dependency ADR slots are required.
- *Reproducible tests* — no wall-clock dependencies introduced. US2 / SC-002's 100-emission cross-cut uses a deterministic sequence or recorded seed (matches 001 SC-005's reproducibility rule via Engineering Standards "Reproducible tests").

Development Workflow specifically engaged:

- *Green checkpoints* / *Logical increments* — `/speckit-tasks` will order tasks so every task closure leaves the crate compiling and `cargo test` green. Concrete ordering plan: introduce `TopicId` (T?-?? before any consumer touches it), then `subscribed_topics` config field + loader extension, then `Message` envelope shape, then `Node` subscription state + receive-path filter, then mutators + getter, then ADR 0008, then integration tests one per user story.

### Post-Phase-1 gate (re-evaluated 2026-05-30)

All Phase 1 artifacts now exist (`research.md`, `data-model.md`, `contracts/library-api.md`, `contracts/peer-list.toml.md`, `quickstart.md`). Re-running the gate against concrete content:

- **I. Correctness Over Optimization** — ✅ **pass**. Every entity in `data-model.md` and every contract clause in `contracts/library-api.md` + `contracts/peer-list.toml.md` traces back to a numbered FR. `data-model.md §7` is the explicit cross-reference matrix (FR → entity → file). No optimization-led decisions appear in the artifacts — the `Arc<Mutex<HashSet>>` choice is documented in `research.md §2` as "the natural and only reasonable answer at this scale", not a performance optimization.
- **II. Test-Driven for Correctness Claims** — ✅ **pass (note unchanged)**. 002 is not protocol-critical (no crypto, no chain semantics, no registry interaction — those start at 003). `quickstart.md` §§2–6 enumerates one integration-test file per user story (`topic_filter.rs` / `n_node_graph.rs` extension / `topic_runtime.rs` / `config_loading.rs` extension); `/speckit-tasks` will schedule those test tasks alongside (not strictly before) implementation tasks. Strict red-green-refactor TDD remains reserved for the protocol-critical features starting at 003.
- **III. Document Structural Decisions as ADRs** — ✅ **pass**. `research.md §8` records one new ADR slot for 002: ADR 0008 (`docs/decisions/0008-subscription-mutator-shape.md`) covering the sync `&self` mutator + interior mutability + linearizability decision. The other 002 deltas (TopicId mirroring PeerId, `Arc<Mutex<HashSet<…>>>` mirroring 001's existing `Arc<Mutex<Vec<…>>>`, Message envelope shape, TOML field extension, tracing field names) are tactical extensions of 001's established patterns and are recorded in `research.md` / `data-model.md` / `contracts/` rather than fresh ADRs. `/speckit-tasks` will materialize ADR 0008 as an ADR-authoring task with its own logical-increment commit.
- **IV. Specifications as Ambiguity Detectors** — ✅ **pass**. Every plan-level item that emerged during planning is recorded in `research.md` with Decision / Rationale / Alternatives — Message envelope shape (§1), concurrency primitive (§2), mutator signature (§3), `subscriptions()` return type (§4), Outcome enum design (§5), TOML field shape (§6), tracing field names (§7). The "Open follow-ups" section at the end (§9) records v2+ items so they cannot be silently rediscovered later.
- **V. Specifications Are Read-Only** — ✅ **pass**. Files touched by this plan run: `specs/002-topic-subscription-filtering/{plan.md, research.md, data-model.md, contracts/library-api.md, contracts/peer-list.toml.md, quickstart.md}` (all agent-editable Spec-Kit artifacts), and `CLAUDE.md` (agent context, not a protocol specification — the SPECKIT block was updated to reference 002's artifacts). No edits to `../formal_spec/`, `../docs/`, or `../docs/extensions/`.

**Gate verdict**: all five principles ✅ pass, no entries in Complexity Tracking. Plan is cleared for `/speckit-tasks`.

## Project Structure

### Documentation (this feature)

```text
specs/002-topic-subscription-filtering/
├── plan.md              # This file (/speckit-plan output)
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   ├── library-api.md   # 002 deltas to the Rust public surface
│   └── peer-list.toml.md # 002 deltas to the TOML schema
├── checklists/
│   └── requirements.md  # Auto-generated by /speckit-specify
├── spec.md              # Feature spec (input)
└── tasks.md             # Phase 2 output (/speckit-tasks — NOT created here)
```

001's `contracts/cli.md` is **inherited unchanged** by 002 — no new CLI flags, no behavior change on existing flags. 002 contracts therefore omit a `cli.md` file; the 001 file remains canonical.

### Source Code (crate root: `pubsub-node/`)

```text
pubsub-node/
├── Cargo.toml                # unchanged (no new deps)
├── src/
│   ├── lib.rs                # re-exports extended: + TopicId, SubscribeOutcome, UnsubscribeOutcome
│   ├── peer.rs               # unchanged
│   ├── topic.rs              # NEW: TopicId newtype, FromStr, parallel to peer.rs's PeerId
│   ├── network.rs            # unchanged (FR-005)
│   ├── node.rs               # extended: subscription field, mutators, getter, recv-path filter
│   ├── message.rs            # extended: topic field on Message (shape per research.md §1)
│   ├── received.rs           # unchanged (ReceivedDelivery wraps the new Message shape)
│   ├── config.rs             # extended: subscribed_topics field + InvalidTopic ConfigError variant
│   ├── error.rs              # extended: + ConfigError::InvalidTopic
│   └── main.rs               # extended: pass parsed HashSet<TopicId> to Node::new
├── tests/
│   ├── two_node_ping.rs      # unchanged (001 US1; Pings still flow when topics align)
│   ├── n_node_graph.rs       # extended: topics layered onto the star graph (002 US2 / SC-002)
│   ├── topic_filter.rs       # NEW: 002 US1 acceptance scenarios (single-topic filter, drop log)
│   ├── topic_runtime.rs      # NEW: 002 US3 acceptance scenarios (dynamic transitions, idempotency)
│   ├── config_loading.rs     # extended: + 002 US4 scenarios (subscribed_topics in TOML)
│   └── common/
│       └── mod.rs            # extended: fixture builders take a subscription set parameter
├── docs/
│   └── decisions/
│       ├── 0001-…             # 001's existing ADRs (unchanged)
│       ├── …
│       ├── 0007-…             # 001's existing ADRs (unchanged)
│       └── 0008-subscription-mutator-shape.md   # NEW: ADR for FR-006 / FR-013 / FR-015 shape
└── specs/                    # this directory
```

**Structure Decision**: Extend 001's single-Cargo-crate layout in place. The new `src/topic.rs` mirrors `src/peer.rs`'s shape (newtype, `FromStr`, validation, error type), keeping the topic-vs-peer parallel structure visible at the file level. `src/network.rs` does **not** appear in the diff — FR-005's "network unchanged" property is enforced by the file diff itself. All test changes either extend existing 001 files (where the test scope still applies — `n_node_graph.rs`, `config_loading.rs`) or add new files dedicated to 002's user stories (`topic_filter.rs` for US1; `topic_runtime.rs` for US3). One ADR is added (`docs/decisions/0008-…`); 001's seven ADRs are not modified.

## Complexity Tracking

*No Constitution Check violations require justification at this stage.*

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|--------------------------------------|
| *(none)* | — | — |
