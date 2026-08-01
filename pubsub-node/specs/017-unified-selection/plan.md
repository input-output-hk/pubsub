# Implementation Plan: Unified selection plane

**Branch**: `017-unified-selection` | **Date**: 2026-07-31 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/017-unified-selection/spec.md`;
plan-input technical direction supplied with the `/speckit-plan` invocation
(recorded verbatim in [plan-input.md](plan-input.md)).

## Summary

Collapse the four dial-side selection strategies and the four acceptance
baselines into one implementation per seam over two fed knobs — the bucket
count (hash-gate width, never derived: the `resolve_buckets` derive arm and
the balanced-B formula are deleted) and the pick count (exactly
min(K, gate survivors) seeded uniform picks, promoted from the
experiments-only `UniformSampler` to a node capability). The CLI becomes
knob-only and presence-activated with zero boundary values replacing the
`none` kinds; acceptance verification follows the seam's bucket count (with
an explicit opt-out) and the accept cap is fed absolutely; the fan-out
default flips to `forward-to-all` (M3's exclusivity becomes the marked
case); `--selection-seed` supplies the node's sampling randomness. The
(bucket absent, pick = RF) point plus the ADR 0034 symmetric handshake
realises the formal M4 exactly — the deferred label is upgraded and the
experiments config gains the symmetric switch so the M4 recipe gets a
recorded baseline. Approach per the plan input: A→B commit ordering —
commit A reproduces the current sampler derivation byte-exactly (baseline
byte-diff gate), commit B lands the honest preimage (per-seam draw domains,
epoch nonce + self-id, `push_len_prefixed`) with one re-baseline and a
statistical m2-comparison re-run.

## Technical Context

**Language/Version**: Rust 1.75, edition 2021 (existing crate settings).

**Primary Dependencies**: existing only — `rand`/`rand_chacha` (seeded
sampling; verified already unconditional — the `experiments` feature gates
only `serde_json`), `sha2` (seed/predicate derivation), `clap`, `serde` +
`toml`. **No manifest change; no new dependency** (research R3).

**Storage**: none new — config files read at the edges; the experiment
output contract (three artifacts) is unchanged.

**Testing**: `cargo test` in both configurations (default and
`--features experiments`) in the green-checkpoint sweep with fmt + clippy
(pedantic). TDD-critical (research R12): `Selection` draw semantics
(exactly-min(K, survivors), gate/pick composition, boundary values,
heartbeat stability, epoch re-randomisation, fleet-shared-seed
independence, the commit-A `UniformSampler` equivalence pin),
`UnifiedAcceptance` admission matrix (incl. cap 0 → explicit `Rejected`),
M4 topology properties (reciprocity, min degree ≥ K, mean ≈ 2K), and the
baseline byte-diff + determinism battery on the experiments side.
Startup-validation tests assert on values/exit behaviour, never log text.

**Target Platform**: developer machines (macOS/Linux); same two binaries
(`pubsub-node`, feature-gated `experiments`).

**Project Type**: single crate — strategy-layer rework + CLI edge +
experiments config; **no `NodeState`, wire, or handler changes**.

**Performance Goals**: no instrument regressions — the draw keeps the
sampler's O(candidates) collect per node/topic (N-035 untouched); baseline
re-recording ~25 s at the M2 operating point (`--workers 10`); the suite's
smoke variant stays inside its 016 budget.

**Constraints**: commit A byte-diffs `runs.jsonl`/`aggregates.json`
identical against `notes/experiments-baselines/` (manifests differ in tool
commit + config text only); the M2 point keeps the formal selection
family's semantics exactly; cross-version byte identity NOT otherwise
required; A→B commit ordering pinned; every commit a green checkpoint;
pre-release — no deprecation aliases for deleted flags/config keys.

**Scale/Scope**: ~10 strategy-layer files deleted, 2 added; `main.rs` Args +
validation rewrite; 4 experiments files touched (config, population, sweep
axis vocabulary, shipped TOMLs) + 1 new M4 config; 2 ADRs (0039, 0040) + 2
implementation notes (N-036, N-037) + N-032 trigger update; quickstart +
m2-comparison + program-doc E12 line + CLAUDE.md stanza; 20 recipe commands
documented.

## Constitution Check

*GATE: evaluated pre-Phase 0; re-evaluated post-Phase 1 — both pass.*

- **I. Correctness Over Optimization — ✅** Every behaviour traces: the gate
  predicate and B-as-security-lever to `../docs/extensions/bucketed-pull.md`
  (read-only); uniform exactly-RF selection and the M4 floor to
  `../formal_spec/hybrid_dissemination/models/`; constructed reciprocity to
  ADR 0034; the plane semantics, knob domains, and validation split to spec
  017 FR-001…FR-028 and research R1–R12. The commit-A byte-identity pin is
  the trace-preservation proof for the refactor itself.
- **II. Test-Driven for Correctness Claims — ✅** The feature carries
  protocol-behaviour claims (the formal M2 selection family exactly; the M4
  minimum-degree floor); the critical set in Technical Context follows
  tests-first (research R12). Non-critical (tests-with): CLI parsing,
  config deserialisation, docs examples.
- **III. Document Structural Decisions as ADRs — ✅ (planned)** ADR **0039**
  — the unified selection plane (one implementation per seam, fed knobs,
  kind enums deleted, acceptance merge + verification-follows-B, fed caps,
  knob-only CLI, fan-out default flip, verifiable region); ADR **0040** —
  selection randomness derivation (seed chain, preimage, domain strings,
  privacy stand-in, the two-commit derivation swap). Optional docs-commit
  candidate: the configuration-placement rationale ADR (flags vs TOML).
- **IV. Specifications as Ambiguity Detectors — ✅** Two divergences are
  surfaced rather than silently resolved: the formal models' private-
  selection assumption vs the prototype's public seed (N-037, with the ADR
  0040 record) and the gate-failing-dial evidence the incentive layer will
  need vs v1's silent drop (N-036). The verifiable-region restatement
  (verifiability ⟺ bucket count present) is recorded in the spec and ADR
  0039, correcting the earlier pure-gate phrasing.
- **V. Specifications Are Read-Only — ✅** `../formal_spec/` and `../docs/`
  are consumed read-only. Edited docs are all code-side: `pubsub-node/docs/`
  (experiments program E12 line, m2-comparison), spec artifacts, ADRs,
  CLAUDE.md.

Engineering Standards applied: **logs are operator UX** (validation tests
assert on behaviour, not stderr; drop causes stay operator-facing);
**implementation-neutral operator strings** (help text and errors name
flags and behaviour, no FR citations); **parse at the edge** (u64 seed →
32-byte expansion and knob-domain enforcement in the loader; constructors
take parsed values); **forward-compatible interfaces justified by named
consumers** (the seam traits persist for the experiments injectors and the
publisher-pair follow-up; no new speculative shapes — the plane removes
machinery); **declarative test construction** (`strategies/test_support`
builders reworked in place; recipe tests via knob constructors);
**justified dependencies** (none added — research R3); **reproducible
tests and simulations** (the seed chain is the feature; no wall-clock
anywhere).

## Project Structure

### Documentation (this feature)

```text
specs/017-unified-selection/
├── spec.md              # Feature specification (post-clarify)
├── plan.md              # This file
├── plan-input.md        # Verbatim /speckit-plan input (technical direction)
├── research.md          # Phase 0 — decisions R1–R12
├── data-model.md        # Phase 1 — plane knobs, Selection/UnifiedAcceptance, derivations, deletion inventory
├── contracts/
│   ├── node-cli.md      # knob flags, activation, validation matrix
│   └── sweep-config.md  # strategy-table delta, axis vocabulary, boundary points
├── quickstart.md        # Phase 1 — recipes (4 families × M1–M5), guidance, validation procedure
├── checklists/requirements.md
└── tasks.md             # Phase 2 (/speckit-tasks — not created by /speckit-plan)
```

### Source Code (repository root)

```text
src/
├── strategies/
│   ├── mod.rs                     # module wiring updated (selection/unified exports)
│   ├── edge.rs                    # − resolve_buckets, bucket_count, accept_cap; + export is_valid_edge_publisher
│   ├── config.rs                  # SelectionParams/AcceptanceParams reshape; builder phase 1 dissolves;
│   │                              #   NodeStrategies construction incl. optional publisher pair (one Result)
│   ├── view.rs                    # unchanged (candidates_len doc updated)
│   ├── test_support.rs            # fixtures reworked in place (knob-built instances)
│   ├── connection/
│   │   ├── mod.rs                 # trait unchanged; exports updated
│   │   ├── selection.rs           # NEW — Selection (gate → seeded pick; commit A/B derivations)
│   │   └── {connect_to_all,hash_gated,none,kind}.rs   # DELETED
│   ├── acceptance/
│   │   ├── mod.rs                 # trait, Admission, admit_prelude/link_scan reused; exports updated
│   │   ├── unified.rs             # NEW — UnifiedAcceptance (gate: Option, accept_cap: Option)
│   │   └── {accept_from_all,bounded,hash_gated,hash_gated_bounded,none,kind}.rs   # DELETED
│   └── fanout/
│       ├── mod.rs                 # module doc fix (§1.2 item 9)
│       ├── kind.rs                # default documentation flip
│       └── forward_to_relays.rs   # uniform Active check (§1.2 item 1)
├── main.rs                        # Args rewrite (knob flags, --selection-seed, fanout default),
│                                  #   validate_flag_combinations rewrite, seed expansion at the edge
├── experiments/
│   ├── config.rs                  # StrategyTable coordinates; axis rename target_degree→pick_count; + bucket_count axis
│   ├── population.rs              # symmetric threading; Selection/UnifiedAcceptance construction; forward-to-all still rejected
│   ├── strategies.rs              # − UniformSampler (SilentRelay stays)
│   └── sweep.rs                   # axis vocabulary follow-through
├── lib.rs                         # re-export updates (deleted types out, Selection/UnifiedAcceptance in)
configs/experiments/               # shipped TOMLs → coordinate vocabulary; + M4 baseline config
docs/
├── decisions/0039-*.md, 0040-*.md # NEW ADRs
├── experiments/m2-comparison.md   # re-executed values (commit B)
└── experiments-program.md         # E12 status line
specs/IMPLEMENTATION_NOTES.md      # + N-036, N-037; N-032 trigger update
tests/
├── model_family.rs                # knob-built recipes; M4 label evidence; §1.2 items 12–13 absorbed
├── publisher_links.rs             # scenarios under new construction
└── (strategy unit suites move with their types)
CLAUDE.md                          # active-work stanza refresh (docs commit rider)
```

**Structure Decision**: single crate, no new modules beyond the two
strategy files; the feature is a strategy-layer collapse plus edge rework —
the pure core (`NodeState`, `apply`, handlers, wire) is deliberately
untouched, which is what keeps the commit-A byte-identity gate meaningful.

## Complexity Tracking

No constitution violations to justify.
