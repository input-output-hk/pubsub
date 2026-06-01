<!-- SPECKIT START -->
**Feature roadmap (read first if planning the next feature)**: `specs/ROADMAP.md` — feature list 002–012 in dependency order, architectural anchors (edge-vs-golden config split, connection-direction inversion), and per-feature open questions.

**Active feature**: **002-topic-subscription-filtering** (planning phase complete; tasks pending).

For technical context, project structure, dependencies, and the rationale
behind 002's plan-level decisions:

- Plan:      `specs/002-topic-subscription-filtering/plan.md`
- Research:  `specs/002-topic-subscription-filtering/research.md`
- Data:      `specs/002-topic-subscription-filtering/data-model.md`
- Contracts: `specs/002-topic-subscription-filtering/contracts/`
- Quickstart:`specs/002-topic-subscription-filtering/quickstart.md`

**Most recently completed feature**: **001-minimal-node-scaffold** (the substrate). Its artifacts under `specs/001-minimal-node-scaffold/` remain the canonical reference for everything 002 inherits unchanged (network substrate, async send/receive shape, `PeerId` / `PeerDescriptor`, TOML loader pipeline, `--self-id` / `--config` / `--log-level` CLI flags). 002's contracts cross-reference back to 001 where they extend rather than replace.

**Workstream-level docs (sibling to feature dirs)**:
- `specs/IMPLEMENTATION_NOTES.md` — deferred implementation questions to revisit (currently N-001 for local-emission/local-receipt under a future REST API; N-002 for self-addressing under connection-based transports in feature 004+).
<!-- SPECKIT END -->

# pubsub-node — agent guidance

Rust implementation of the Cardano PubSub node. Project-level context is in the parent `pubsub/CLAUDE.md`; this file covers what an agent working inside `pubsub-node/` needs to know.

## Authoritative documents

- **Constitution**: `.specify/memory/constitution.md` — five principles governing implementation work here. Honour all five before authoring code or specs.
- **Feature specs** (agent-editable): `specs/NNN-<short-name>/spec.md`, produced by `/speckit-specify`.
- **ADRs** (agent-authored): `docs/decisions/NNNN-<title>.md` — structural decisions, per Constitution Principle III.
- **Protocol specifications (READ-ONLY per Principle V)**: `../formal_spec/`, `../docs/`, `../docs/extensions/`. Surface ambiguity via ADR or issue; do not edit.

## Spec Kit workflow

Features flow through: `/speckit-specify` → `/speckit-plan` → `/speckit-tasks` → `/speckit-implement`. Small fixes may bypass.

### Manual branch step (project-specific quirk)

`.specify/` lives inside the parent `pubsub/` git repo, not at the git root. The Spec Kit git extension deliberately does not auto-create branches in this layout — `/speckit-specify` will emit:

    [specify] Warning: Git repository not detected; skipped branch creation

This is expected, not an error. Spec Kit tracks the feature by directory name (`specs/NNN-<short-name>/`); the git branch is your responsibility.

Before invoking `/speckit-specify <description>`, cut the branch manually:

    ls specs/                              # peek at the next available number
    git checkout -b NNN-<short-name>       # e.g. 001-minimal-node-scaffold

If your branch's short name matches what Spec Kit derives from the description, the resulting `specs/NNN-<short-name>/` directory will line up with the branch. If they diverge, it's harmless — the directory name is the canonical feature identifier; the branch is for git/PR workflow only.

## Commit discipline (Constitution §3)

- Every commit MUST compile and pass all non-ignored tests.
- Commits MUST be logical increments leaving the repo at a functional checkpoint.
- Skipped/ignored tests MUST carry a reason and a tracking issue or ADR reference.
