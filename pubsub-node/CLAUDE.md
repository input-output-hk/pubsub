<!-- SPECKIT START -->
**Feature roadmap (read first if planning the next feature)**: `specs/ROADMAP.md` — feature list 002–012 in dependency order, architectural anchors (edge-vs-golden config split, connection-direction inversion), and per-feature open questions.

**Active feature**: **003-message-envelope-mock-crypto** (planning phase complete; tasks pending).

For technical context, project structure, dependencies, and the rationale
behind 003's plan-level decisions:

- Plan:      `specs/003-message-envelope-mock-crypto/plan.md`
- Research:  `specs/003-message-envelope-mock-crypto/research.md`
- Data:      `specs/003-message-envelope-mock-crypto/data-model.md`
- Contracts: `specs/003-message-envelope-mock-crypto/contracts/`
- Quickstart:`specs/003-message-envelope-mock-crypto/quickstart.md`
- ADR 0009:  `docs/decisions/0009-crypto-trait-shape.md` (crypto trait shape — concrete byte newtypes, no associated types, mock-crypto factory shape)
- ADR 0010:  `docs/decisions/0010-protocol-message-type-hierarchy.md` (Message enum + SignedMessage / PlainMessage split + 001 Envelope → RoutingFrame rename + MessageHash::of(&PlainMessage) content-anchored hash choice; authored post-/speckit-plan when the type-conflation concern surfaced)

**Most recently completed feature**: **002-topic-subscription-filtering** (the topic dimension). Its artifacts under `specs/002-topic-subscription-filtering/` remain the canonical reference for everything 003 inherits unchanged (the `topic_drop` emitter is renamed to `message_dropped` / `cause = "topic_not_subscribed"` in the same commit as 003's `invalid_signature` emitter lands per FR-015; everything else from 002 — the `TopicId` newtype, the subscription set, the `subscribe`/`unsubscribe` API, the `subscriptions()` snapshot, the TOML `subscribed_topics` field — propagates unchanged). 002's contracts cross-reference back to 001 where they extend rather than replace.

**Workstream-level docs (sibling to feature dirs)**:
- `specs/IMPLEMENTATION_NOTES.md` — deferred implementation questions to revisit (currently N-001 for local-emission/local-receipt under a future REST API; N-002 for self-addressing under connection-based transports in feature 004+; N-003 for chain-integrity / equivocation / publisher-authorization validation under the future registry features 008 / 012; N-004 for CBOR-canonical encoding swap under feature 009 or first cross-language consumer; N-005 for the `MessageHash::of(&PlainMessage)` content-anchored hash decision, to revisit when downstream features first operationally consume the hash — chain-integrity validation at 008 / 012, future caching / dedup).
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
