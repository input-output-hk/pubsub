# Implementation Plan: Minimal PubSub Node Scaffold

**Branch**: `001-minimal-node-scaffold` | **Date**: 2026-05-18 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/001-minimal-node-scaffold/spec.md`

## Summary

Build the smallest credible substrate for a `pubsub-node` in Rust: a `Node` type, a `Network` abstraction with an `InMemory` variant, a `Ping(N)` message, and a TOML-driven peer-list loader exposed via a CLI binary. The scaffold demonstrates two-node connectivity (US1), generalises to N-node graphs (US2), and supports per-node TOML configuration (US3) — all under explicit Trust / Liveness / No-Crypto assumptions captured in the spec.

Technical approach (per the spec's Clarifications session 2026-05-17):
- **Async-from-v1**: `Node` and `Network` expose `async fn` sends/receives (FR-011) so the interface and the test harness already accommodate future networked transports.
- **Decoupled send/observability**: `send().await` resolves on enqueue; recipients process into their `received_messages()` record subsequently (FR-013). Tests use an explicit await-on-delivery primitive.
- **Parse at the edge**: the binary's CLI loads + parses TOML and hands the Node constructor an already-parsed `PeerListConfig` value (FR-001 / FR-012). The library itself is filesystem-free.
- **Identity is separate from peer view**: the node's own id is a constructor argument / `--self-id` CLI flag, never a field in the TOML (FR-012).
- **Abstract descriptor type**: a `PeerDescriptor` trait with an `id()` accessor; the v1 concrete impl is a thin wrapper over a UTF-8 string (FR-009). Future iterations add fields (addresses, keys) without breaking callers.

## Technical Context

**Language/Version**: Rust 1.75+ stable (edition 2021). Native `async fn` in traits (stabilised in 1.75) lets us declare the `Network` trait directly without `async-trait` macro overhead. To keep that natural shape we make `Node::new` generic over `N: Network` rather than taking `Arc<dyn Network>` — `async fn` in trait is not `dyn`-compatible on stable, and a v1 PoC with a single implementor pays no price for monomorphisation. The trait's lint `async_fn_in_trait` (about uninferrable `Send` bounds) is allowed for now because `InMemoryNetwork`'s body is `Send` by inference; this is flagged for revisit when a second `Network` impl arrives (see `research.md` "Open follow-ups").

**Primary Dependencies**:
- `tokio` (runtime + sync primitives — chosen in research.md §1)
- `serde` + `toml` (config parsing — research.md §2)
- `tracing` + `tracing-subscriber` (structured logging — research.md §3)
- `clap` (CLI flag parsing — research.md §4)
- `thiserror` (error enums — research.md §5)

Each of these is structural per Constitution Principle III and is covered by a planned ADR; see `research.md` §"ADR slot summary" for the authoritative list (currently 7 slots, 0001–0007).

**Storage**: N/A — single-process, in-memory only. No persistence in this iteration.

**Testing**: `cargo test` with `#[tokio::test]` for async integration tests. Test layout: integration tests under `tests/` (one file per user story) plus a `tests/common/` module exposing the await-on-delivery helper and fixture builders. No property-based testing required at this scaffold stage (no property-level correctness claims per Engineering Standards bullet 1).

**Target Platform**: Linux + macOS developer workstations. No production deployment target at this stage. CI matrix is a planning-stage placeholder (out of scope for the scaffold's first commit).

**Project Type**: Single Cargo crate, library + binary. `src/lib.rs` exports the public API consumed by the binary (`src/main.rs`) and by integration tests. This is the minimum structure that keeps the CLI / loader layer clearly separate from the domain library (FR-012 layering).

**Performance Goals**: None at this stage. SC-001 bounds end-to-end stand-up + ping verification at 30s of local execution (trivially satisfied by an in-memory hashmap). SC-005 demands 100 sends with N intact.

**Constraints**:
- Single-process scope (spec Assumptions).
- No cryptographic operations (FR-007).
- Peer set static for node lifetime (FR-008).
- Async API for send/receive (FR-011); receive-side observability decoupled from send-completion (FR-013).
- Logging crate must support structured fields with named values. FR-006 and FR-010 both consume the same structured-log facility — there is only one logging style in the system. FR-010 specifically requires warn-level entries for unregistered-peer drops (mandatory); FR-006 permits additional structured output as supplementary observability (optional).

**Scale/Scope**: 2 ≤ N ≤ 10 nodes per demonstration (US2). 100 sequential sends per SC-002 / SC-005. Single-process, single async runtime.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Evaluated against `.specify/memory/constitution.md` v1.0.0.

### Initial gate (before Phase 0)

- **I. Correctness Over Optimization** — ✅ **pass**. Every plan claim traces either to a numbered FR in `spec.md`, a dated bullet in `## Clarifications`, or this plan. No optimization-led decisions; the in-memory network is deliberately under-built.
- **II. Test-Driven for Correctness Claims** — ✅ **pass (with note)**. This scaffold does *not* carry a protocol-behavior claim (the constitution's TDD trigger). Tests are still mandatory at the acceptance-scenario level (US1–US3) but plain `cargo test` integration suites are sufficient; full TDD discipline (red → green) is reserved for protocol-critical features that arrive in later iterations. Tests will be authored alongside (not strictly before) implementation tasks; `/speckit-tasks` will preserve this ordering rule for the upcoming critical features even though it relaxes here.
- **III. Document Structural Decisions as ADRs** — ⚠ **at-risk → mitigated**. Seven structural choices are introduced (async runtime, config parser, logging stack, CLI parser, error model, receive-task model & registration timing, NetworkHandle as actor-handle / channels-over-callbacks). Each gets its own ADR slot listed in `research.md`. Mitigation: ADRs are tracked as deliverables in `/speckit-tasks`, not deferred indefinitely.
- **IV. Specifications as Ambiguity Detectors** — ✅ **pass**. Plan-level items the spec did not pin down (async runtime, receive-task driver, mailbox bounding, await-on-delivery primitive shape) are addressed in `research.md` with explicit Decision / Rationale / Alternatives. None are silently resolved in code.
- **V. Specifications Are Read-Only** — ✅ **pass**. This plan does not propose edits to `../formal_spec/`, `../docs/`, or `../docs/extensions/`. The only spec touched is the feature spec under `specs/001-minimal-node-scaffold/spec.md`, which is agent-editable per the file's own informational note.

Engineering Standards specifically engaged:
- *Observable state transitions* — Node/Network emit `tracing` events for: peer registration, send-accepted, recipient-record-updated, unregistered-peer-drop (FR-010 warn-level requirement).
- *Justified dependencies* — covered by ADRs above. The Rust toolchain (`std`, `core`) and `cargo test` are exempt; everything else is in an ADR.
- *Reproducible tests* — no wall-clock dependencies. The await-on-delivery helper uses `tokio::time::timeout` with an injectable budget so tests do not race against scheduler jitter.

Development Workflow specifically engaged:
- *Green checkpoints* / *Logical increments* — `/speckit-tasks` will order tasks so every task closure leaves the crate compiling and `cargo test` green (e.g., introduce the trait + a stub impl, then real impl, then wire into Node).

### Post-Phase-1 gate (re-evaluated 2026-05-18)

All Phase 1 artifacts now exist (`research.md`, `data-model.md`, `contracts/{cli,library-api,peer-list.toml}.md`, `quickstart.md`). Re-running the gate against concrete content:

- **I. Correctness Over Optimization** — ✅ **pass**. Every entity in `data-model.md` and every contract clause in `contracts/library-api.md` has an FR trace; `data-model.md §9` is the explicit cross-reference matrix. No optimization-led decisions appear in the artifacts.
- **II. Test-Driven for Correctness Claims** — ✅ **pass (note unchanged)**. The scaffold is not protocol-critical. `quickstart.md` §§2–5 enumerates one integration test file per user story (US1 / US2 / US3); `/speckit-tasks` will schedule those test tasks alongside (not after) implementation tasks. Strict red-green-refactor TDD applies to the next iteration's protocol features, not to this scaffolding work.
- **III. Document Structural Decisions as ADRs** — ✅ **pass**. `research.md`'s "ADR slot summary" table lists seven ADRs (0001 through 0007) covering every structural choice the plan introduces (async runtime, config loader, logging, CLI parser, error model, receive-task & registration timing, NetworkHandle actor-handle / channels-over-callbacks per CHK022 follow-up). `/speckit-tasks` will materialise each as an ADR-authoring task with a logical-increment commit.
- **IV. Specifications as Ambiguity Detectors** — ✅ **pass**. Every plan-level item the spec deferred (mailbox bounding, receive driver, ordering, await-on-delivery shape, CLI parsing crate, error model) appears in `research.md` with explicit Decision / Rationale / Alternatives. The "Open follow-ups" section at the bottom of `research.md` records v2+ items so they cannot be silently rediscovered later.
- **V. Specifications Are Read-Only** — ✅ **pass**. Files touched by this plan run: `specs/001-minimal-node-scaffold/{plan.md, research.md, data-model.md, contracts/*.md, quickstart.md}` (all agent-editable Spec-Kit artifacts) and `CLAUDE.md` (agent context, not a protocol specification). No edits to `../formal_spec/`, `../docs/`, or `../docs/extensions/`.

**Gate verdict**: all five principles ✅ pass, no entries in Complexity Tracking. Plan is cleared for `/speckit-tasks`.

## Project Structure

### Documentation (this feature)

```text
specs/001-minimal-node-scaffold/
├── plan.md              # This file (/speckit-plan output)
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   ├── cli.md           # CLI flags, exit codes, error reporting
│   ├── library-api.md   # Public Rust API surface (Node, Network, helpers)
│   └── peer-list.toml.md # TOML peer-list schema
├── spec.md              # Feature spec (input)
└── tasks.md             # Phase 2 output (/speckit-tasks — NOT created here)
```

### Source Code (crate root: `pubsub-node/`)

```text
pubsub-node/
├── Cargo.toml
├── src/
│   ├── lib.rs                 # Re-exports the public surface (Node, Network, PeerDescriptor, …)
│   ├── peer.rs                # PeerId newtype, PeerDescriptor trait, v1 concrete descriptor impl
│   ├── network.rs             # Network trait, InMemoryNetwork struct, registration & routing
│   ├── node.rs                # Node struct, constructor (id + parsed peer set + network handle)
│   ├── message.rs             # Message enum (Ping(N) only at this stage)
│   ├── received.rs            # ReceivedRecord (queryable per-node delivery log per FR-006)
│   ├── config.rs              # PeerListConfig struct + TOML loader (file → parsed value)
│   ├── error.rs               # Error types (thiserror)
│   └── main.rs                # CLI binary (clap; --self-id, --config; parses TOML, builds Node)
├── tests/
│   ├── two_node_ping.rs       # US1 acceptance scenarios
│   ├── n_node_graph.rs        # US2 acceptance scenarios
│   ├── config_loading.rs      # US3 acceptance scenarios (incl. malformed-config error path)
│   └── common/
│       └── mod.rs             # await_delivery helper + fixture builders
├── docs/
│   └── decisions/             # ADRs (0001-async-runtime-tokio.md, etc.)
└── specs/                     # this directory (existing)
```

**Structure Decision**: Single Cargo crate, library + binary, with integration tests under `tests/` (one file per user story). This keeps the FR-012 parse-at-edge boundary visible at the file level — `src/main.rs` and `src/config.rs` own all path/file work; `src/{node,network,peer,message,received}.rs` is filesystem-free and async-only. ADRs live in `docs/decisions/` alongside the crate, following the convention established in `pubsub-node/CLAUDE.md`.

## Complexity Tracking

*No Constitution Check violations require justification at this stage.*

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|--------------------------------------|
| *(none)* | — | — |
