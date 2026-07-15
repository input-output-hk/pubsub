# Implementation Plan: Publisher links and dissemination-model configurations (M3/M4/M5)

**Branch**: `015-connection-link-model` (spec dir `015-publisher-links`) | **Date**: 2026-07-15 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/015-publisher-links/spec.md`

## Summary

Extend the M2 node so M3, M4, and M5 (`../formal_spec/hybrid_dissemination/models/`)
are per-node flag combinations, as a **minimal extension**: two new public shapes
(`LinkKind`, `LinkKey`), the existing `upstream`/`downstream` collections re-keyed
by `LinkKey`, the existing three strategy traits reused (publisher seams =
optional second instances; fan-out gains the message origin), one wire byte
(the link kind, signed), and two config enums (`PublisherAdmission`, fan-out
kind). No direction enum, no link-store/view abstraction, no new traits. The
abandoned first cut (`archive/015-full-exploration`) is the reference for the
validated semantics and correctness traps; its behaviour is preserved, its
abstraction layer is not. Design decisions: [research.md](research.md); shapes
and transitions: [data-model.md](data-model.md); observable surface:
[contracts/link-kinds-and-seams.md](contracts/link-kinds-and-seams.md).

## Technical Context

**Language/Version**: Rust (2021 edition, workspace toolchain)

**Primary Dependencies**: existing crate set only — tokio (shell), sha2 (edge predicate), clap (CLI), thiserror, tracing. No new dependencies.

**Storage**: N/A (in-memory state; mock registries from TOML)

**Testing**: cargo test — synchronous state-machine tests (`src/state/tests`), strategy unit tests, multi-node integration tests (`tests/`)

**Target Platform**: single-process experiment harness (in-memory network); Linux/macOS dev machines

**Project Type**: library + binary crate (`pubsub-node`)

**Performance Goals**: none protocol-critical; per-topic link reads become `BTreeMap` range walks (no regression vs today's full scans)

**Constraints**: minimal-shapes constraint from the spec Input (binding); pre-existing tests untouched except the mechanical getter rename + the deliberate wire-layout pin update (research R3); every commit a green checkpoint

**Scale/Scope**: ~12 source files touched, 2 new strategy files (`all_links.rs`, fan-out kind), 1 ADR, 2 ported integration test files

## Constitution Check

- **I. Correctness Over Optimization** — ✅ every behaviour traces: M3/M4/M5 semantics to the model READMEs (read-only), mechanism decisions to research.md R1–R14 and ADR 0032 (planned), carried correctness requirements to spec FR-002/010/011 + SC-006.
- **II. Test-Driven for Correctness Claims** — ✅ protocol-behaviour feature: the receive-gate change, owner-binding, severance target, fan-out origin split, and symmetric reciprocity get failing tests before implementation (tasks will order them so); mechanical renames/reshapes are covered by the existing suite staying green.
- **III. Document Structural Decisions as ADRs** — ✅ ADR `0032-publisher-links-and-model-family.md` planned (research R14); it also records the supersession relationship to the archive branch's ADRs 0032–0036.
- **IV. Specifications as Ambiguity Detectors** — ✅ known model-vs-realisation gap (exactly-k private picks approximated by binomial predicate draws) is documented in the spec Assumptions and quickstart, inherited from the archive's analysis; no new ambiguity encountered during planning. Any found during implementation goes to the ADR or an issue.
- **V. Specifications Are Read-Only** — ✅ no edits under `pubsub/formal_spec/` or `pubsub/docs/`.

Engineering Standards: logs stay operator UX (all assertions via the four getters); operator-facing strings neutral (flag help names behaviours, not FRs); parse at the edge (`PublisherAdmission`/kind selectors parse in `main.rs`/kind enums, core takes typed values); forward compatibility consumer-justified (no `publisher-edge-sym` domain, no third fan-out kind — research R6/R7); declarative test construction (ConnectionScript gains publisher-kind steps; existing steps unchanged).

## Project Structure

### Documentation (this feature)

```text
specs/015-publisher-links/
├── spec.md
├── checklists/requirements.md
├── plan.md              # this file
├── research.md          # R1–R14
├── data-model.md        # shapes, invariants, per-handler transitions
├── contracts/link-kinds-and-seams.md
├── quickstart.md        # M2–M5 recipes
└── tasks.md             # /speckit-tasks output (next)
```

### Source Code (repository root: `pubsub-node/`)

```text
src/
├── connection_state.rs        # LinkState rename; LinkKind + LinkKey; ConnectionScript publisher steps
├── message.rs                 # PlainConnection.kind; signed_bytes kind byte; layout pin update
├── state.rs                   # map reshape; publisher heartbeat pass; kind dispatch; gate + severance; getters
├── state/tests/…              # getter renames only (mechanical), new gate/severance/publisher tests
├── node.rs                    # NodeStrategies-taking constructor; four link getters
├── main.rs                    # flag renames + publisher/fanout/admission/symmetric flags
├── lib.rs                     # re-exports (LinkKind, LinkKey, LinkState, PublisherAdmission, AllLinks, kinds)
└── strategies/
    ├── view.rs                # NodeView: upstream + downstream map borrows
    ├── edge.rs                # is_valid_edge_in + publisher/sym wrappers (is_valid_edge unchanged)
    ├── config.rs              # four-slot NodeStrategies; params gain symmetric + publisher degree
    ├── connection/            # expected_links rename; HashGatedConnection kind/symmetric fields
    ├── acceptance/            # link_scan; kind-aware admit_prelude; kind/symmetric fields
    └── fanout/                # trait signature (+origin, map); forward_to_all update; all_links.rs; kind.rs

tests/
├── connections.rs, bounded_selection.rs, … # getter renames only
├── publisher_links.rs         # ported: unconditional establishment, owner-binding, M3 recipe
└── model_family.rs            # ported: M4 reciprocity/coverage, M5 foreign-hop chain

docs/decisions/
└── 0032-publisher-links-and-model-family.md
```

**Structure Decision**: single existing crate; no new modules beyond two fan-out
strategy files and their kind selector — the feature is an extension of five
existing seams, not a new subsystem.

## Complexity Tracking

No constitution violations to justify. The one deliberate test-edit exception
(wire layout pin) is recorded in research R3 and re-pins the new canonical
layout in the same commit that changes it.
