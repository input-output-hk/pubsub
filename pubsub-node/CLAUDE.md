<!-- SPECKIT START -->
Active feature: **001-minimal-node-scaffold**.

For technical context, project structure, dependencies, and the rationale
behind plan-level decisions, read:

- Plan:      `specs/001-minimal-node-scaffold/plan.md`
- Research:  `specs/001-minimal-node-scaffold/research.md`
- Data:      `specs/001-minimal-node-scaffold/data-model.md`
- Contracts: `specs/001-minimal-node-scaffold/contracts/`
- Quickstart:`specs/001-minimal-node-scaffold/quickstart.md`
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
