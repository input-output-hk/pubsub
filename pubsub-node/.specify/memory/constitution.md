<!--
Sync Impact Report
==================
Version change: template (unratified) → 1.0.0
Bump rationale: First ratified constitution for the pubsub-node implementation.
Initial adoption establishes the principle set, engineering standards, development
workflow, and governance rules. Subsequent amendments follow SemVer per the
Governance section.

Modified principles (placeholder → concrete):
- [PRINCIPLE_1_NAME] → I. Correctness Over Optimization
- [PRINCIPLE_2_NAME] → II. Test-Driven for Correctness Claims
- [PRINCIPLE_3_NAME] → III. Document Structural Decisions as ADRs
- [PRINCIPLE_4_NAME] → IV. Specifications as Ambiguity Detectors
- [PRINCIPLE_5_NAME] → V. Specifications Are Read-Only to the Implementation Agent

Added sections:
- Engineering Standards
- Development Workflow
- Governance

Removed sections: none.

Templates requiring updates:
- ✅ updated: .specify/templates/plan-template.md — Constitution Check section
  now enumerates the five principles as concrete gates (pass / at-risk /
  violation) and points back to Engineering Standards and Development Workflow.
- ✅ updated: .specify/templates/tasks-template.md — "Tests are OPTIONAL"
  framing replaced with Principle II conditional rule; added an ADR-tasks note
  pointing to Principle III.
- ✅ reviewed, no change: .specify/templates/spec-template.md — existing
  `[NEEDS CLARIFICATION: ...]` marker already implements Principle IV (surface
  ambiguity, do not silently resolve). No edit required.
- ⚠ pending review (out of scope for this skill run):
  pubsub-node/CLAUDE.md — currently points to "the current plan" only; the
  next CLAUDE.md update should add a pointer to this constitution alongside.

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

**Version**: 1.0.0 | **Ratified**: 2026-05-14 | **Last Amended**: 2026-05-14
