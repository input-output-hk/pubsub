# Implementation Plan: Deterministic experiments framework

**Branch**: `016-experiments-framework` | **Date**: 2026-07-18 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/016-experiments-framework/spec.md`; plan-input technical direction supplied with the `/speckit-plan` invocation (recorded verbatim in [plan-input.md](plan-input.md)).

## Summary

Build the feature-gated `experiments` module: a deterministic in-process
driver that runs populations of real node cores (`NodeState` + `apply`)
under a round-based wavefront scheduler, measures delivery (coverage, depth,
miss causes, message cost) and topology health (degrees, sinks, SCC-based
goodness with churn pairing), and executes runs/experiments/sweeps in
parallel with a three-artifact, byte-reproducible output contract — plus the
`experiments` front-end binary, two experiments-only strategies (silent
relay, uniform sampler), scripted validation topologies, and the shipped
M2-comparison configurations. Approach per the plan input: no async in the
measurement path; `std::thread::scope` worker pool; one optional
feature-tied dependency (`serde_json`); hand-rolled iterative Kosaraju;
driver-owned determinism (content-keyed wave canonicalisation — no core
changes beyond crate-internal access).

## Technical Context

**Language/Version**: Rust 1.75, edition 2021 (existing crate settings).

**Primary Dependencies**: existing — `serde` (+derive), `toml`, `clap`,
`rand`/`rand_chacha` (seeded sampling), `sha2` (seed derivation);
new — `serde_json`, **optional**, activated only by the `experiments` cargo
feature (JSONL/aggregates encoding; deterministic float formatting via ryu).
Explicitly excluded from the measurement path: `tokio`, channels,
`InMemoryNetwork`. No `rayon` (std `thread::scope` worker pool instead), no
graph library (hand-rolled iterative Kosaraju), no statistics library
(closed-form Wilson 95%).

**Storage**: output files only (sweep manifest JSON, run-records JSONL,
aggregates JSON), written exclusively by the sweep layer; the run itself is
a pure function performing no I/O.

**Testing**: `cargo test` in both configurations — without and with
`--features experiments` — in the green-checkpoint sweep. Determinism
testing layered: in-memory value equality is the workhorse; one focused
serialization test plus one/two file-level byte-diff integration tests
anchor SC-001. `proptest` available for property-style checks (e.g.
accounting identity over generated scripted topologies).

**Target Platform**: developer machines (macOS/Linux); library module +
second binary target `experiments` (`required-features = ["experiments"]`).

**Project Type**: single crate — feature-gated library module + CLI
front-end.

**Performance Goals**: SC-006 — smoke variant < 30 s inside the suite; the
manual M2 operating point (N = 20 000, R ≥ 40) < 1 h on a developer machine.
Populations up to ~10⁵ nodes per run in-process; sweeps of 10³–10⁴ runs.

**Constraints**: byte-identical artifacts for same (config, master seed) at
any worker count and across process restarts; default run records carry
nothing O(N); `experiments` feature off ⇒ crate build, public API, and test
results unchanged; core touched only via crate-internal access (ordering of
core collections delegated to the in-flight connection-link work — spec
Clarifications 2026-07-18).

**Scale/Scope**: nine submodules under `src/experiments/` + one binary;
two experiments-only strategy instances; three output artifact kinds; two
shipped comparison configurations + smoke variant; no protocol changes.

## Constitution Check

*GATE: evaluated pre-Phase 0; re-evaluated post-Phase 1 — both pass.*

- **I. Correctness Over Optimization — ✅** Every measured quantity traces
  to a written reference: the good-graph criterion and comparison values to
  `../formal_spec/hybrid_dissemination/models/` (read-only), the program's
  metrics to `docs/experiments-program.md`, the driver/measurement semantics
  to spec 016 FR-001…FR-033 and this plan's research entries (R1–R12). The
  SCC reduction and the excluded-publisher convention are documented with
  their derivations (research.md R8, data-model.md).
- **II. Test-Driven for Correctness Claims — ✅** This feature *is* a
  correctness claim (the instrument's numbers must be right). The plan
  designates as **critical — TDD required**: driver delivery semantics
  (dedup/fire-once/quiescence), wave canonicalisation determinism,
  propagation-graph extraction + Kosaraju/condensation/goodness, the
  accounting identity, and the coverage/depth/miss-cause metrics. Scripted
  known-topology tests and the two-instrument cross-check are written first
  and must fail before implementation. Non-critical (tests-with): front-end
  flag parsing, quickstart examples.
- **III. Document Structural Decisions as ADRs — ✅ (planned)** Three ADRs:
  **0032** — deterministic experiments driver (wavefront scheduler,
  driver-owned canonicalisation, participant model, phase orchestration);
  **0033** — experiment output contract & statistics conventions (three
  artifacts, derivability invariant, counts + Wilson 95%, excluded-publisher
  denominator); **0034** — `serde_json` as an optional feature-tied
  dependency (Justified Dependencies standard).
- **IV. Specifications as Ambiguity Detectors — ✅** One divergence already
  surfaced for the humans: the formal folder's ±1σ uncertainty convention
  degenerates at all-good samples — carried as the mandated methodology note
  in the documented M2 comparison (spec FR-033) to raise with the
  formal-methods team. Any further model ambiguities found while building
  the comparison configs will be surfaced per the principle, not resolved in
  code.
- **V. Specifications Are Read-Only — ✅** `../formal_spec/` and
  `../docs/` are consumed read-only (extraction of operating-point values
  only); no edits proposed or required.

Engineering Standards applied: **logs are operator UX** (spec FR-017: all
measurement from driver-owned state; no test asserts on logs);
**implementation-neutral operator strings** (the binary's help/errors and
quickstart carry no FR citations); **parse at the edge** (TOML + clap in the
binary; the experiments API takes parsed values — FR-031); **forward-
compatible interfaces justified by named consumers** (the dissemination-
model dispatch enum's consumers are the experiment program's later stages
and the in-flight 015-publisher-links feature; participant storage's
Level-2 headroom is named by the program's adversary stages — spec FR-011,
FR-022); **declarative test construction** (the `scripted` module is the
test-only builder layer for topologies and event scripts); **justified
dependencies** (ADR 0034); **reproducible tests and simulations** (the
feature's core requirement — FR-024/FR-026; no wall-clock anywhere).

## Project Structure

### Documentation (this feature)

```text
specs/016-experiments-framework/
├── spec.md              # Feature specification (post-clarify)
├── plan.md              # This file
├── plan-input.md        # Verbatim /speckit-plan input (technical direction)
├── research.md          # Phase 0 — decisions R1–R12
├── data-model.md        # Phase 1 — entities, record schemas, phase machine
├── contracts/
│   ├── output-artifacts.md   # manifest / run-records / aggregates contract
│   └── sweep-config.md       # TOML sweep description + CLI invocation contract
├── quickstart.md        # Phase 1 — build, run, sweep, replay, M2 demo procedure
├── checklists/requirements.md
└── tasks.md             # Phase 2 (/speckit-tasks — not created by /speckit-plan)
```

### Source Code (repository root)

```text
Cargo.toml                       # [features] experiments = ["dep:serde_json"];
                                 # [[bin]] experiments (required-features)
src/
├── lib.rs                       # + #[cfg(feature = "experiments")] pub mod experiments;
│                                #   (feature-gated public module — see note below)
├── experiments/                 # the one feature-gated public addition:
│   │                            #   `pub mod experiments` exists only under the
│   │                            #   flag (the bin target consumes it); the
│   │                            #   default build's surface is unchanged (FR-001)
│   ├── mod.rs                   # module root; the experiments API surface
│   ├── population.rs            # Participant, class, seeded population build,
│   │                            #   registry pre-population / faithful-fold scripts
│   ├── driver.rs                # wavefront scheduler, wave canonicalisation,
│   │                            #   per-phase drains, phase orchestration
│   ├── graph.rs                 # extraction dispatch (DisseminationModel, M2 impl),
│   │                            #   iterative Kosaraju, condensation, goodness,
│   │                            #   min-publisher-coverage, degree/sink stats
│   ├── metrics.rs               # drain observation, miss-cause classification,
│   │                            #   sends split, suppressed accounting, identity
│   │                            #   assertion, run-record assembly
│   ├── statistics.rs            # histograms, means/percentiles, Wilson 95%,
│   │                            #   aggregates fold (canonical order)
│   ├── sweep.rs                 # manifest, seed derivation, worker pool,
│   │                            #   canonical-order JSONL streaming, aggregates emission
│   ├── config.rs                # parsed sweep-description types + validation
│   ├── scripted.rs              # declarative scripted-topology builders (test support)
│   └── strategies.rs            # experiments-only: SilentRelay (fan-out),
│                                #   UniformSampler (dial) — never protocol CLI kinds
├── bin/experiments.rs           # front-end: clap flags, TOML load, progress to stderr
└── (core modules)               # unchanged except pub(crate) access where needed

configs/experiments/             # shipped M2-comparison sweep descriptions
├── m2-operating-point.toml      # N=20 000, mu=0.2, RF=24 (manual)
├── m2-bulk-regime.toml          # named point from m2's validation grid (manual)
└── m2-smoke.toml                # suite-sized smoke variant

docs/experiments/                # experiment write-ups (results documents)
└── m2-comparison.md             # T026's documented comparison (tasks
                                 #   amendment 2026-07-19; seeds + tool commit
                                 #   cited for reproducibility)

tests/
└── experiments_framework.rs     # feature-gated integration suite: scripted-topology
                                 #   exactness, determinism (value-level; one file-level
                                 #   byte diff; workers 1 vs K), two-instrument
                                 #   cross-check, smoke variant
```

**Structure Decision**: single-crate feature-gated module per plan input —
the driver needs the crate-internal `apply`/`NodeState`/`Effect` surface, so
a sibling crate is not viable without publicising internals; the
`experiments` cargo feature keeps the default build byte-for-byte unaffected
(SC-008). Test placement follows house convention (unit tests beside
modules, integration suite under `tests/`, gated on the feature).

## Complexity Tracking

No constitution violations to justify. The one new dependency (`serde_json`)
is handled under the Justified Dependencies standard via ADR 0034 rather
than as a violation.
