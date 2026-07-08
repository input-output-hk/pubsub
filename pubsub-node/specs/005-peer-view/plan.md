# Implementation Plan: Verifiable hash-gated connection-selection and bounded acceptance

**Branch**: `005-peer-view` | **Date**: 2026-06-29 (redesigned 2026-07-02) | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/005-peer-view/spec.md`

## Summary

Replace the full-mesh dial/accept policies with the **verifiable bucketed-pull** overlay (`docs/extensions/bucketed-pull.md`) so a node forms a reproducible, adversary-resistant partial topology. The dial side (`HashGatedConnection`) dials candidate `U` on topic `T` at the current interval `I` iff the shared **verifiable edge predicate** `is_valid_edge(genesis, T, self, U, I, B)` holds — `H(genesis, T, self, U, I) mod B == 0`, with `B = max(1, round(|candidates_T| / target_degree))` and a **fixed** configured target (connection) degree `target_degree`, so expected out-degree ≈ `target_degree` and small topics (`B = 1`) connect to all candidates. The inbound side (`VerifiableBoundedAcceptance`) accepts iff the request is membership-valid, the **same** predicate holds (the acceptor recomputes it — verifies rather than trusts), and the node holds fewer than `OC = ⌈target_degree + c·√target_degree⌉` downstream on the topic; over capacity of a legitimate request it sends an explicit `Rejected` (not a severance). A membership or predicate failure is a **silent drop** (distinct causes `membership_validation_failed` / `illegitimate_request`; the new `Admission::RejectIllegitimate`). On receipt of a `Rejected`, the dialer **only** drops the matching pending `AwaitingAccept` upstream; there is **no** retry or back-fill — re-forming is deferred to a future heartbeat-rotation layer + a separate future strategy family, out of scope for 005. Realized degree may settle below `target_degree` after rejections. The existing unbounded policies remain the default; verifiable behaviour is opt-in via the startup parameters `--genesis`, `--target-degree`, `--cap-buffer`. Tests ship with the feature (TDD).

The single dial-trigger event `ConnectionSetup` is renamed `Heartbeat { interval }` (an advancing 0-based counter, driver-fired, no wall-clock); `(genesis, interval)` stand in for the model's per-round beacon `nonce_R`. v1 fires one interval (0) at readiness; periodic heartbeats + cross-interval rotation/teardown are deferred. Fan-out is unchanged (`ForwardToAll`; the former bounded/seeded fan-out is dropped).

This is strategies-only. The **experiment/testing framework** that drives these strategies is a separate later feature. The feature is **coordinated with — not built on** — the co-developing architect's determinism/purity refactor (strategies-as-`apply`-arguments, deterministic scheduling, a flag decoupling the dial trigger from `Synced`): 005 keeps the current strategy injection (`Arc<dyn …>` at `Node::new`) and keeps ordered structures (`BTreeSet`/`BTreeMap`) on its state, so it does not block on that refactor (see research R6).

## Technical Context

**Language/Version**: Rust (workspace toolchain; rust-version 1.75).

**Primary Dependencies**: `sha2` (already a dependency; `crypto::MessageHash` uses `Sha256`) is the hash behind the edge predicate — a fixed, cross-machine-stable algorithm, explicitly NOT `std::hash::DefaultHasher` (unspecified/non-portable, would break FR-002 cross-machine). `is_valid_edge` reduces the leading bytes of `SHA-256` over a domain-separated, length-prefixed canonical encoding of `(genesis, topic, requester, candidate, interval)` modulo `B`. No PRNG is used — the predicate is a pure hash-bucket test, not sampling. `proptest`/a seeded loop (already a dev-dependency) for the SC-003 uniformity sweep. tokio + tracing as today.

**Storage**: N/A (in-memory node state).

**Testing**: `cargo test`; **TDD** (protocol-behaviour + determinism claims → critical per Constitution II). Declarative test construction via the existing `ConnectionScript`, extended with a `rejected` step.

**Target Platform**: library crate + CLI binary (`src/main.rs`).

**Project Type**: single Rust project.

**Performance Goals**: selection O(candidates) per topic (one hash per candidate); no hot path.

**Constraints**: the state-transition stays pure/deterministic — no wall-clock, no randomness drawn at decision time (FR-006); reproducible from the public genesis + interval; ordered structures so effect emission is order-stable and the predicate is order-independent by construction (FR-014).

**Scale/Scope**: per-node strategy logic; multi-node scale + metrics are the separate framework feature.

**Relationship to the determinism/purity refactor (coordination, not a hard dependency)**: the broader refactor (strategies-as-`apply`-arguments, deterministic scheduling, decouple flag) is the co-developing architect's separate workstream. This feature does **not** block on it — it keeps ordered structures (`BTreeSet`/`BTreeMap`) on the state it introduces/touches and keeps strategy objects pure, retaining the current strategy injection and migrating later. See research.md R6. Coordination is to avoid conflicting edits on shared files (`NodeState`, strategy injection sites).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Correctness Over Optimization** — ✅ Every behaviour traces to an FR in `spec.md` or a decision in `research.md`. The verifiable edge predicate (fixed SHA-256 hash-bucket test over ordered inputs) is the correctness mechanism for FR-001/FR-002/FR-007.
- **II. Test-Driven for Correctness Claims** — ✅ **Critical.** Determinism/verifiability (FR-002/SC-001/SC-002), degree ≈ `target_degree` (FR-001/FR-003/SC-004), uniformity (SC-003), acceptance + explicit rejection (FR-007/FR-008), rejection dropping the pending upstream (FR-009) get tests before implementation.
- **III. Document Structural Decisions as ADRs** — ✅ ADRs planned (below).
- **IV. Specifications as Ambiguity Detectors** — ✅ The seam note claims degree caps "slot in without a signature change"; false for both sides (the acceptance side needs current-downstream input + a reason-bearing return; both need the interval) — surfaced as ADR 0025/0030, not silently reshaped.
- **V. Specifications Are Read-Only** — ✅ Only `pubsub-node/` code-side artifacts; no edits to `pubsub/docs/` or `pubsub/formal_spec/`.

**Engineering Standards** — ✅ reproducible-from-genesis is core; ✅ no wall-clock in the transition (dial = `Heartbeat` re-invocation, externally driven); ✅ degree ≈ `target_degree` / `OC` bound / under-fill asserted via getters/snapshots, not log strings; ✅ parse-at-the-edge (`genesis`/`target_degree`/`cap_buffer` parsed in CLI/loader, passed as values into strategy construction); ✅ forward-compatible (`ConnectionAction::Rejected`, `Heartbeat { interval }`) justified by this feature; ✅ declarative test construction (`ConnectionScript`).

### ADRs

- **ADR 0024** — Verifiable hash-gated selection: the shared edge predicate `is_valid_edge` (SHA-256 hash-bucket test over a canonical length-prefixed encoding — not `DefaultHasher`), `bucket_count = max(1, round(candidates / target_degree))` with a **fixed** `target_degree`, and the small-topic connect-to-all floor (`B = 1`). (Retry-to-a-minimum / back-fill is deferred to a future strategy family, out of scope for 005.)
- **ADR 0025** — Acceptance-seam evolution + `ConnectionAction::Rejected`: acceptance return `bool → Admission { Accept, RejectMembership, RejectIllegitimate, RejectOverCapacity }` taking the current downstream view; the acceptor **verifies** the same edge predicate; the explicit `Rejected` action (acceptor → dialer, over-capacity only, not misbehaviour) and the dialer dropping the matching pending upstream on receipt (no retry/back-fill).
- **ADR 0030** — Heartbeat interval + shared edge predicate: `Event::ConnectionSetup` → `Event::Heartbeat { interval: u64 }` (advancing 0-based counter, driver-fired); `NodeState.interval` folded from it; the interval threaded through the seam methods via a grouped `NodeView { subscriptions, candidates, downstream, interval }` (`expected_upstream(&view)` / `admit(emitter, topic, &view)`); the `strategies::edge` module as the single predicate both seams consult.

> ADR 0028 (two-phase strategy construction) and 0029 (`strategies/` module grouping + `edge`) also land with this feature. The **strategies-as-arguments** relocation is owned by the co-developing architect's refactor (its own ADRs on that branch); 005 coordinates with it, keeping the current injection.

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
│   ├── strategies/            # all strategy policy (ADR 0029)
│   │   ├── mod.rs             # module wiring
│   │   ├── edge.rs            # NEW shared verifiable predicate: is_valid_edge, bucket_count, accept_cap (ADR 0024/0030)
│   │   ├── connection/        # mod.rs (ConnectionStrategy trait + NodeView arg), connect_to_all.rs,
│   │   │                      #   hash_gated.rs (NEW HashGatedConnection), kind.rs (ConnectionStrategyKind)
│   │   ├── acceptance/        # mod.rs (trait + NEW Admission + NodeView arg), accept_from_all.rs,
│   │   │                      #   verifiable_bounded.rs (NEW VerifiableBoundedAcceptance), kind.rs
│   │   ├── fanout/            # mod.rs (trait), forward_to_all.rs (unchanged)
│   │   └── config.rs          # two-phase construction (ADR 0028): ConnectionParams/AcceptanceParams, NodeStrategies(Builder)
│   ├── connection_state.rs    # core connection lifecycle state (UpstreamState, test_support) — not a strategy
│   ├── event.rs               # Event::Heartbeat { interval } (renamed from ConnectionSetup, ADR 0030)
│   ├── message.rs             # ConnectionAction::Rejected (tag 0x03)
│   ├── state.rs               # NodeState.interval; handle_heartbeat (folds interval + dials); request verify + capacity branch (+ Rejected send); handle_connection_rejected (drops matching pending upstream only)
│   ├── node.rs                # current-injection construction
│   ├── main.rs/config.rs      # parse --genesis/--target-degree/--cap-buffer + named strategy kinds at the edge; two-phase build
│   └── lib.rs                 # re-export the new public strategy types
└── tests/
    ├── connections.rs        # US1/US2 integration: verifiable topology, over-capacity Rejected, pending-drop
    ├── common/mod.rs         # ConnectionScript `rejected` step; node_with_strategy reused
    └── ...                   # (uniformity sweep is a unit test in strategies/edge.rs)
```

**Structure Decision**: Single Rust project. All strategy policy lives under `strategies/` (ADR 0029): each seam is its own module (`connection/`, `acceptance/`, `fanout/`) — trait in `mod.rs`, one file per implementation — plus `config` (ADR 0028 two-phase construction) and `edge` (the shared verifiable predicate). Connection lifecycle state (`UpstreamState`, `test_support`) is core domain state in `connection_state`, not a strategy (`Admission` stays with the acceptance seam as its return contract). Strategies stay injected as `Arc<dyn …>` at `Node::new` (the **current injection**, not the refactor's strategies-as-arguments shape); 005 keeps ordered structures on its state and coordinates with the parallel refactor rather than depending on it (research R6).

## Complexity Tracking

No Constitution violations. One sequencing dependency, not a violation:

| Item | Note | Guidance |
|------|------|----------|
| Parallel determinism/purity refactor (strategies-as-args, ordered structures, decouple flag) touches shared files | 005 does not block on it — keeps ordered structures on its state, keeps strategies pure, retains current injection | Coordinate with the co-developing architect on shared-file edits (`NodeState`, strategy injection sites) and ordered-type choices (tasks T003). Strategies migrate to the argument shape when the refactor lands; no gating. |
