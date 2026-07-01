# Implementation Plan: Seeded bounded connection-selection and acceptance strategies

**Branch**: `005-peer-view` | **Date**: 2026-06-29 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/005-peer-view/spec.md`

## Summary

Replace the full-mesh dial/accept policies with **bounded** ones so a node forms a reproducible partial topology. The dial side (`SeededBoundedSelection`) picks at most a uniform upstream degree of upstream peers per topic by deterministic keyed-hash ranking of `(seed, self_id, topic, candidate)`; randomness is encapsulated in the strategy object (the seed is a field), so the transition stays pure. The inbound side (`BoundedAcceptance`) admits up to a uniform downstream degree and, over capacity, sends an explicit `Rejected` (not a severance). A rejected dial marks the peer **failed (sticky for the run)** and is back-filled by re-invoking the existing `ConnectionSetup` event over the viable candidate view (candidates minus failed) — no new round/timer. The existing unbounded policies remain the default; bounded behaviour is opt-in via three startup parameters (seed, upstream degree, downstream degree). Tests ship with the feature (TDD).

This is strategies-only. The **experiment/testing framework** that drives these strategies is a separate later feature. The feature is **coordinated with — not built on** — the co-developing architect's determinism/purity refactor (strategies-as-`apply`-arguments, deterministic scheduling, a flag decoupling `ConnectionSetup` from `Synced`): 005 keeps the current strategy injection (`Arc<dyn …>` at `Node::new`) and applies ordered structures (`BTreeSet`) to its own new state, so it does not block on that refactor (see research R6).

## Technical Context

**Language/Version**: Rust (workspace toolchain; rust-version 1.75).

**Primary Dependencies**: `sha2` (already a dependency; `crypto::MessageHash` uses `Sha256`) for the stable ranking digest — explicitly NOT `std::hash::DefaultHasher` (unspecified/non-portable, would break FR-003 cross-machine). `proptest` (already a dev-dependency) for the SC-004 uniformity sweep. tokio + tracing as today.

**Storage**: N/A (in-memory node state).

**Testing**: `cargo test`; **TDD** (protocol-behaviour + determinism claims → critical per Constitution II). Declarative test construction via the existing `ConnectionScript`, extended with a `rejected` step.

**Target Platform**: library crate + CLI binary (`src/main.rs`).

**Project Type**: single Rust project.

**Performance Goals**: selection O(candidates · log upstream_degree) per topic; no hot path.

**Constraints**: the state-transition stays pure/deterministic — no wall-clock, no randomness drawn at decision time (FR-009); reproducible from a recorded seed; ordered structures so results are iteration-order-independent (FR-017).

**Scale/Scope**: per-node strategy logic; multi-node scale + metrics are the separate framework feature.

**Relationship to the determinism/purity refactor (coordination, not a hard dependency)**: the broader refactor (strategies-as-`apply`-arguments, deterministic scheduling, decouple flag) is the co-developing architect's separate workstream. This feature does **not** block on it — it applies ordered structures (`BTreeSet`/`BTreeMap`) to the state it introduces/touches itself and keeps strategy objects pure, retaining the current strategy injection and migrating later. See research.md R6. Coordination is to avoid conflicting edits on shared files (`NodeState`, strategy injection sites).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Correctness Over Optimization** — ✅ Every behaviour traces to an FR in `spec.md` or a decision in `research.md`. The keyed-hash ranking is the correctness mechanism for FR-003/FR-007.
- **II. Test-Driven for Correctness Claims** — ✅ **Critical.** Determinism (FR-003/SC-001), bound (FR-001/SC-002), unbiasedness (FR-007/SC-004), acceptance + explicit rejection (FR-010/FR-011), sticky back-fill (FR-014) get tests before implementation.
- **III. Document Structural Decisions as ADRs** — ✅ ADRs planned (below).
- **IV. Specifications as Ambiguity Detectors** — ✅ The seam note claims degree caps "slot in without a signature change"; false for the acceptance side (needs current-downstream input + a reason-bearing return) — surfaced as ADR 0025, not silently reshaped.
- **V. Specifications Are Read-Only** — ✅ Only `pubsub-node/` code-side artifacts; no edits to `pubsub/docs/` or `pubsub/formal_spec/`.

**Engineering Standards** — ✅ reproducible-from-seed is core; ✅ no wall-clock in the transition (re-dial = `ConnectionSetup` re-invocation, externally driven); ✅ rejection/under-fill asserted via getters/snapshots, not log strings; ✅ parse-at-the-edge (seed/upstream degree/downstream degree parsed in CLI/loader, passed as values into strategy construction); ✅ forward-compatible (`ConnectionAction::Rejected`) justified by this feature; ✅ declarative test construction (`ConnectionScript`).

### Planned ADRs (numbers provisional — next free after 0023; coordinate with the refactor branch)

- **ADR 0024** — Seeded deterministic bounded selection: keyed-hash ranking over `(seed, self_id, topic, candidate)`, stable SHA-256 digest (not `DefaultHasher`), per-network seed / per-node derivation, sticky failed-set + `ConnectionSetup`-driven back-fill.
- **ADR 0025** — Acceptance-seam evolution + `ConnectionAction::Rejected`: acceptance return `bool → Admission { Accept, RejectMembership, RejectOverCapacity }` taking the current downstream view; the explicit `Rejected` action (acceptor → dialer, not misbehaviour) and the dialer's failed-mark on receipt.

> The **strategies-as-arguments** relocation and **ordered-structure** swap are owned by the prerequisite refactor (its own ADRs on that branch); 005 consumes them.

## Project Structure

### Documentation (this feature)

```text
specs/005-peer-view/
├── plan.md          # this file
├── research.md      # Phase 0 — R1–R7
├── data-model.md    # Phase 1 — state/types/transitions
├── quickstart.md    # Phase 1 — constructing & exercising the bounded strategies
├── contracts/       # Phase 1
│   ├── strategy-traits.md
│   └── connection-control.md
└── tasks.md         # Phase 2 (/speckit-tasks — not created here)
```

### Source Code (repository root)

```text
pubsub-node/
├── src/
│   ├── connection/        # per-seam module: mod.rs (trait + UpstreamState + test_support),
│   │                      #   connect_to_all.rs, seeded_bounded.rs (NEW + ranking helper), kind.rs (ConnectionStrategyKind)
│   ├── acceptance/        # mod.rs (trait + NEW Admission), accept_from_all.rs (admit), bounded.rs (NEW BoundedAcceptance), kind.rs
│   ├── fanout/            # mod.rs (trait), forward_to_all.rs (unchanged)
│   ├── message.rs         # ConnectionAction::Rejected (tag 0x03)
│   ├── state.rs           # failed_upstream (BTreeSet); viable-candidate diff in connection-setup; request capacity branch (+ Rejected send); NEW rejected handler; rejections_received counter + getter
│   ├── node.rs            # current-injection construction; rejections_received getter
│   ├── main.rs/config.rs  # parse seed/upstream degree/downstream degree + named strategy kinds at the edge; select bounded vs unbounded
│   └── lib.rs             # re-export the new public strategy types
└── tests/
    ├── bounded_selection.rs  # US1 integration: capped + reproducible topology
    ├── common/mod.rs         # ConnectionScript `rejected` step; node_with_strategy reused for bounded nodes
    └── ...                   # (seed-sweep uniformity is a unit test in connection/seeded_bounded.rs)
```

**Structure Decision**: Single Rust project. Each strategy seam is its own module (`connection/`, `acceptance/`, `fanout/`) — trait in `mod.rs`, one file per implementation — refactored for separation as more impls land. Strategies stay injected as `Arc<dyn …>` at `Node::new` (the **current injection**, not the refactor's strategies-as-arguments shape); 005 applies ordered structures to its own new state and coordinates with the parallel refactor rather than depending on it (research R6).

## Complexity Tracking

No Constitution violations. One sequencing dependency, not a violation:

| Item | Note | Guidance |
|------|------|----------|
| Parallel determinism/purity refactor (strategies-as-args, ordered structures, decouple flag) touches shared files | 005 does not block on it — applies ordered structures to its own state, keeps strategies pure, retains current injection | Coordinate with the co-developing architect on shared-file edits (`NodeState`, strategy injection sites) and ordered-type choices (tasks T003). Strategies migrate to the argument shape when the refactor lands; no gating. |
