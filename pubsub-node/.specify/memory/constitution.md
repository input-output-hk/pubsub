<!--
Sync Impact Report
==================
Version change: 1.0.0 → 1.1.0
Bump rationale: MINOR. Materially expands Engineering Standards and Development
Workflow with conventions that were applied de facto across features 001–003 but
never written down. No principle added, removed, or redefined; no governance
change. The core principle set stays I–V. Ratified by both maintainers (project
author + co-developing architect) on 2026-06-08, per their prior agreement that
these de-facto conventions be codified before the next two features branch.

Added Engineering Standards bullets:
- Logs are operator UX, not a test surface
- Operator-facing strings are implementation-neutral
- Parse at the edge
- Forward-compatible interfaces for known consumers (roadmap-justified)

Added Development Workflow bullets:
- Analysis ledger (analysis.md)
- Spec fidelity is verified against code when code exists

Templates requiring updates:
- ✅ updated: .specify/templates/plan-template.md — Constitution Check note
  now points at the new Engineering Standards (logs-not-tested, neutral strings,
  parse-at-edge, forward-compatible interfaces); no new principle gate added.
- ✅ reviewed, no change: .specify/templates/tasks-template.md — the
  logs-are-not-a-test-surface standard is enforced per-feature in the test tasks;
  no template-structural edit required.
- ✅ reviewed, no change: .specify/templates/spec-template.md — unaffected by
  this amendment.

History:
- 1.0.0 (2026-05-14): initial ratified constitution — five principles
  (I–V) plus Engineering Standards, Development Workflow, and Governance.

Follow-up TODOs: none.
-->

# Cardano PubSub Node Constitution

## Core Principles

### I. Correctness Over Optimization

Code MUST be traceable to a written specification — a formal model in
`pubsub/formal_spec/`, a referenced section of a paper in `pubsub/docs/`, a
design note (including extension proposals under `pubsub/docs/extensions/`), an
ADR, or the feature's `plan.md`. Optimizations (performance, ergonomics,
abstraction) MUST NOT come at the cost of breaking that trace. When correctness
and optimization conflict, correctness wins; the optimization is captured as a
follow-up note or issue, not a silent deviation.

Rationale: the value of this implementation is that it realizes specific
protocol claims. A faster or cleaner version that drifts from the specification
provides no such claim and is harder to reason about than no implementation at
all.

### II. Test-Driven for Correctness Claims

For features that carry a correctness or protocol-behavior claim, tests SHOULD
be written and reviewed before implementation. Exploratory spikes and
disposable scaffolding are exempt. Complex or critical features — envelope
handling, registry interaction, message verification, chain validation, and any
feature explicitly designated as critical in its plan — MUST follow TDD: tests
first, tests fail, implementation makes them pass.

Rationale: protocol guarantees survive contact with implementation only when
the test articulates the guarantee before the code shapes the test.

### III. Document Structural Decisions as ADRs

Structural decisions — architecture, dependency choices, protocol-shaping
options, persistence and concurrency strategy, public-interface contracts —
MUST be captured as ADRs at `docs/decisions/NNNN-title.md`, with rationale,
trade-offs considered, and alternatives rejected. Tactical decisions (naming,
file layout, local refactors) are exempt.

A decision is **structural** if reversing it would require touching unrelated
code, external interfaces, or another protocol layer. A decision is **tactical**
if reversing it is a local rewrite.

Rationale: a PoC that evolves freely needs an audit trail of the choices it
has already foreclosed; without one, the same trade-offs get re-litigated and
the context is lost.

### IV. Specifications as Ambiguity Detectors

Formal models, papers, and design notes serve as ambiguity and conflict
detectors during implementation. When implementing a feature exposes an
ambiguity, contradiction, or gap in a referenced specification, the implementer
MUST surface it — by opening an ADR that documents the chosen interpretation,
or by filing an issue — rather than silently resolving it in code.

Rationale: silent in-code resolutions destroy the specification's value as a
correctness reference. A surfaced ambiguity becomes input to spec maintenance;
a hidden one becomes a future bug whose root cause is invisible.

### V. Specifications Are Read-Only to the Implementation Agent

The implementation agent (Claude Code, or any automated tooling acting in an
implementer role) MUST NOT modify specifications: formal models under
`pubsub/formal_spec/`, papers under `pubsub/docs/`, design notes, and extension
proposals under `pubsub/docs/extensions/` are read-only from the agent's
perspective. Human authors edit specifications; the agent surfaces issues per
Principle IV. Code-side artifacts (ADRs, `plan.md`, `tasks.md`, source under
`pubsub-node/`) remain editable.

Rationale: specifications are the project's design record. Letting the
implementation agent edit them collapses the separation between "what we
decided" and "what we built," and turns specs into another build artifact
rather than a fixed reference.

## Engineering Standards

- **Property-based testing for critical properties.** Where a feature carries a
  property-level claim (chain monotonicity, equivocation detectability,
  signature binding, deduplication idempotence), property-based tests are
  preferred over single-case unit tests. Single-case tests remain appropriate
  for example-driven illustration and regression pins.
- **Observable state transitions.** Protocol-critical state transitions MUST
  emit structured logs sufficient to reconstruct behavior post-hoc (which
  message, which peer, which decision, which outcome). Log volume tuning is a
  deployment concern; the ability to reconstruct is a design concern.
- **Logs are operator UX, not a test surface.** The structured logs required
  above exist for operators and post-hoc reconstruction. Correctness is asserted
  through state-observation surfaces — getters, return values, snapshots — never
  through log content. Automated tests MUST NOT assert on log strings, log
  events, or their fields (including via source-grep over emitter call sites).
  When a feature's acceptance scenarios describe log output, that text is
  descriptive operator UX, not a test-anchored contract.
- **Operator-facing strings are implementation-neutral.** CLI help, log events,
  `eprintln!`/stderr text, rustdoc, and library-consumer documentation describe
  behavior in stable terms and MUST NOT cite FR identifiers, clarification
  numbers, or spec-section references. Those citations live only in source `//`
  comments and Spec-Kit artifacts (`spec.md`, `plan.md`, `tasks.md`,
  `data-model.md`, `contracts/`), which are feature-scoped and decay across
  iterations.
- **Parse at the edge.** Core constructors and domain types take already-parsed,
  in-memory values; file I/O, deserialization (TOML, JSON, CBOR), and
  CLI-argument parsing live in a thin loader/CLI layer at the process boundary.
  The core MUST be testable without touching the filesystem or the argument
  vector.
- **Forward-compatible interfaces for known consumers.** Where `specs/ROADMAP.md`
  names a downstream feature that will consume an interface, prefer the shape
  that consumer needs over the simplest stub — `#[non_exhaustive]` enums for
  protocol-message and error types, trait-at-construction with
  concrete-at-storage (`Arc<dyn Trait>`), opaque newtypes over raw primitives,
  async signatures where a real transport will require them. The anticipatory
  shape MUST be justified by a feature actually on the roadmap; shaping for a
  consumer that no roadmap entry names is over-abstraction and a Principle I
  violation, not an application of this standard.
- **Justified dependencies.** External runtime or test dependencies require an
  ADR documenting why an in-tree implementation was insufficient. Standard
  language toolchain components and the project's chosen test framework are
  exempt.
- **Reproducible tests and simulations.** Tests, property runs, and simulations
  MUST be reproducible from a recorded seed. No assertions may depend on
  wall-clock time; time-sensitive logic uses an injected clock.

## Development Workflow

- **Green checkpoints.** Every commit on a branch MUST compile and pass all
  non-ignored tests. Work-in-progress commits that break this rule are
  permissible only on local branches and MUST be squashed or fixed before
  pushing.
- **Logical increments.** Commits MUST be logical increments that leave the
  repository at a functional checkpoint. The motivating use is bisection and
  rollback: it must be possible to check out any pushed commit and find a
  buildable, testable state.
- **Tracked skips.** Tests intentionally skipped or marked ignored MUST carry a
  reason and a tracking issue or ADR reference in-source. A growing pool of
  un-tracked skips is treated as a violation of Principle II.
- **Spec Kit flow for scoped features.** Substantive feature work flows through
  the Spec Kit lifecycle (`/speckit-specify` → `/speckit-plan` → `/speckit-tasks`
  → `/speckit-implement`). Small fixes, dependency bumps, and documentation
  changes may bypass.
- **Analysis ledger.** When `/speckit-analyze` runs, its findings and their
  resolutions are recorded in the feature's `analysis.md`, following the
  command's own category structure (mirrors
  `specs/001-minimal-node-scaffold/analysis.md`). Commit messages are not the
  ledger; a finding without an `analysis.md` entry is not closed.
- **Spec fidelity is verified against code when code exists.** Cross-artifact
  consistency checking (`/speckit-analyze`, checklists) is the baseline and runs
  whether or not an implementation is present — when analyze runs after
  `/speckit-tasks` and no code exists yet, cross-artifact checking is the whole
  job and remains required. Additionally, once the implementation exists, a
  consistency pass MUST verify artifact claims about the implementation against
  the implementation itself — for example, grep `lib.rs` re-exports and module
  visibility to confirm a contract's public-surface claims — rather than relying
  on cross-artifact agreement alone. A claim that is internally consistent
  across artifacts but contradicted by the code is a defect such a pass MUST
  catch. This is why an analyze round after implementation is valuable, not only
  the pre-implementation pass.

## Governance

- **Maintainers.** This constitution is maintained jointly by the two
  developers working on this repository: the project author and the
  co-developing architect (both human). The implementation agent operates
  under it but does not author amendments.
- **Amendment scope.** Either maintainer MAY propose and patch amendments.
  Substantive changes — adding, removing, or redefining a principle, or
  altering governance itself — require agreement between both maintainers
  before merge. Clarifications, wording fixes, and typographical corrections
  MAY be applied unilaterally.
- **Versioning (SemVer).** MAJOR for backward-incompatible removal or
  redefinition of a principle or governance rule. MINOR for adding a new
  principle, a new section, or material expansion of existing guidance.
  PATCH for clarifications, wording, typos, and non-semantic refinement.
  Ambiguous bumps default to MINOR with the rationale recorded in the Sync
  Impact Report.
- **Dates.** Amendments MUST update the **Last Amended** date. The **Ratified**
  date stays fixed at the original adoption date.
- **Compliance.** PRs SHOULD verify compliance against the current
  constitution. Complexity or design choices that violate a principle MUST be
  justified in an ADR before merge; the ADR is the audit record that the
  trade-off was made deliberately.

**Version**: 1.1.0 | **Ratified**: 2026-05-14 | **Last Amended**: 2026-06-08
