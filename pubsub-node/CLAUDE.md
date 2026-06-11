<!-- SPECKIT START -->
**Feature roadmap (read first if planning the next feature)**: `specs/ROADMAP.md` — feature list 002–012 with architectural anchors (edge-vs-golden config split, connection-direction inversion) and per-feature open questions. **Feature numbers are IDs, not a strict build order** — see the note at the top of ROADMAP.

**Active features (two, developed in parallel as of 2026-06-08)**:

- **004 — node event-loop refactor** (project author's branch `004-node-event-loop`). **Merged to `main` (PR #50).** Full artifact set under `specs/004-node-event-loop/` (spec + clarifications, plan, research, data-model, contracts, quickstart, tasks 11/11 complete, checklists CHK001–023, analysis.md with three analyze sessions incl. the post-implementation verify-against-code pass). Structural decisions in ADR 0011 (`docs/decisions/0011-pure-state-transition-core.md`: crate-internal `NodeState` + pure `apply` → `Vec<Effect>` with `Effect` uninhabited, named per-variant handlers — `apply` → `handle_message_received` → `handle_signed_message` — tracing-as-ambient-effect carve-out) and ADR 0012 (`docs/decisions/0012-node-state-sharing-and-lifecycle.md`: `Arc<Mutex<NodeState>>` with sync getters, sync subscribe/unsubscribe retained, spawn-in-constructor + drop-abort). Code shape: `src/state.rs` is the crate-internal pure core (+ sync state-machine unit tests); `src/node.rs` is the async shell (queue, loop = lock→apply→execute-effects, named `network_mailbox_loop` producer). Parity proven: the 002/003 integration suite passes **unmodified**. **Rescoped to the refactor only**: the connection model is deferred to a follow-on `004-connections` feature (branch + spec dir under the 004 umbrella) after this merges.
- **008 — subscription registry** (branch `feat/node-registry`, spec dir `specs/008-node-registry/`). **Specified + planned** (spec under review in PR #51; plan/research/data-model/contracts authored — see [`specs/008-node-registry/plan.md`](specs/008-node-registry/plan.md)). The in-memory **subscription list** (node membership; distinct from the topic registry, ~007 — no shared trait): `SubscriptionRegistry` trait + `InMemorySubscriptionRegistry` (`new()`/`from_file`), `SubscriptionEvent` (Joined/TopicsChanged/Left, identity+interest only), `SubscriptionWatch` (push, mirrors `NetworkHandle`/ADR 0007). Node integration on 004's pure core: `Event::SubscriptionUpdate(SubscriptionEvent)` + `handle_subscription_update` folding a per-topic candidate set into `NodeState` (self-excluded; **distinct from the config bootstrap `peers`**, resolving N-007); `Node::candidates` getter. **The node is read-only; the subscription list — not config — is the source of truth for a node's own interests** (ADR 0013): `Node::new` takes `Arc<dyn SubscriptionRegistry>`, drops `initial_subscriptions`, sources interests via `interests_of(self_id)` (fail-fast if absent), and 002's `subscribed_topics` config field is removed. Interface + integration in ADR 0014 (`docs/decisions/0014-subscription-registry-interface-and-node-integration.md`). Seam-variant rename `RegistryUpdate`→`SubscriptionUpdate` pending a heads-up to the 004 author + a one-line update to ADR 0011's comment. Docs PR #52 fixes the `joining.md` config-vs-chain authority ambiguity. Next: `/speckit-tasks`.

Their shared boundary (the node event queue) is specified in **`specs/event-loop-and-registry-contract.md`** — **both features' specs MUST cite it** (both do).

**Most recently completed feature**: **003-message-envelope-mock-crypto** — the signed message envelope + mock crypto. Canonical reference under `specs/003-message-envelope-mock-crypto/` (plan, research, data-model, contracts, quickstart). Key decisions:

- ADR 0009 `docs/decisions/0009-crypto-trait-shape.md` — crypto trait shape (concrete byte newtypes, no associated types, mock-crypto factory shape).
- ADR 0010 `docs/decisions/0010-protocol-message-type-hierarchy.md` — `Message` enum + `SignedMessage` / `PlainMessage` split + 001 `Envelope` → `RoutingFrame` rename + `MessageHash::of(&PlainMessage)` content-anchored hash.
- Introduced the `Signer` / `Verifier` traits + `crypto::mock`, the `Arc<dyn Verifier>` `Node::new` parameter, and the `message_dropped` / `cause` drop-event convention (which replaced 002's `topic_drop`). Everything 002 contributed (the `TopicId` newtype, the subscription set + `subscribe`/`unsubscribe` API, the `subscriptions()` snapshot, the TOML `subscribed_topics` field) propagates unchanged.

**Workstream-level docs (sibling to feature dirs)**:
- `specs/event-loop-and-registry-contract.md` — shared contract for the two active features (004 / 008); both their specs cite it.
- `specs/IMPLEMENTATION_NOTES.md` — deferred implementation questions to revisit (currently N-001 for local-emission/local-receipt under a future REST API; N-002 for self-addressing under connection-based transports in feature 004+; N-003 for chain-integrity / equivocation / publisher-authorization validation under the future registry features 008 / 012; N-004 for CBOR-canonical encoding swap under feature 009 or first cross-language consumer; N-005 for the `MessageHash::of(&PlainMessage)` content-anchored hash decision, to revisit when downstream features first operationally consume the hash — chain-integrity validation at 008 / 012, future caching / dedup; N-006 for the construction-failure (duplicate-registration) integration test, deferred from 004's parity scope to 004-connections; N-007 for `peers` placement — shell field today; **resolved for 008**: the `SubscriptionUpdate` arm folds an interest-derived candidate set into `NodeState`, kept distinct from the config bootstrap `peers` shell field (ADR 0014); the static `peers` field stays on the shell for the future dialer; revisit again at 005 (`PeerView`)).
<!-- SPECKIT END -->

# pubsub-node — agent guidance

Rust implementation of the Cardano PubSub node. Project-level context is in the parent `pubsub/CLAUDE.md`; this file covers what an agent working inside `pubsub-node/` needs to know.

## Authoritative documents

- **Constitution**: `.specify/memory/constitution.md` (v1.1.0) — five principles (I–V) plus Engineering Standards and Development Workflow rules; honour all of them before authoring code or specs. (Notably: logs are operator UX, not a test surface; operator-facing strings carry no FR/spec citations; parse at the edge; forward-compatible interfaces are justified by a ROADMAP consumer; `/speckit-analyze` findings are recorded in the feature's `analysis.md`.)
- **Feature specs** (agent-editable): `specs/NNN-<short-name>/spec.md`, produced by `/speckit-specify`.
- **ADRs** (agent-authored): `docs/decisions/NNNN-<title>.md` — structural decisions, per Constitution Principle III.
- **Protocol specifications (READ-ONLY per Principle V)**: `../formal_spec/`, `../docs/`, `../docs/extensions/`. Surface ambiguity via ADR or issue; do not edit.

## Spec Kit workflow

Features flow through: `/speckit-specify` → `/speckit-plan` → `/speckit-tasks` → `/speckit-implement`. Small fixes may bypass.

### Manual branch step — do this BEFORE `/speckit-specify`

`.specify/` lives inside the parent `pubsub/` git repo, not at the git root. Because of this subfolder layout, the Spec Kit git extension does **not** auto-create the feature branch — `/speckit-specify` emits:

    [specify] Warning: Git repository not detected; skipped branch creation

This is expected, not an error: Spec Kit tracks the feature by directory name (`specs/NNN-<short-name>/`); the git branch is **your** responsibility, and you MUST cut it yourself **before** running `/speckit-specify` so the feature work is isolated from `main`.

Procedure:

1. **Confirm you are on `main`** — `git branch --show-current` MUST print `main`. If you are already on a feature branch, do NOT branch again; go straight to `/speckit-specify`.
2. **Cut the branch from `main`**, named `NNN-<short-name>`:

       git checkout -b NNN-<short-name>     # e.g. 008-topic-registry

3. Then run `/speckit-specify <description>`.

Make the branch's short-name match what Spec Kit derives from your description so the branch and the `specs/NNN-<short-name>/` directory line up. If they diverge it's harmless — the directory name is the canonical feature identifier; the branch is for git/PR workflow only.

**Next-up branches** (per the two active features above):

- `004-node-event-loop` — the node event-loop refactor + connection model (project author).
- `008-topic-registry` — the mock topic registry (co-developing architect).

Per the parallel-work plan in `specs/event-loop-and-registry-contract.md`, the refactor (004) lands to `main` first; 008 then branches from the updated `main` so `Event` / `EventQueue` / `spawn_producer` already exist. If both must start from today's `main` at once, see the "seam commit" note in that doc.

## Commit discipline (Constitution §3)

- Every commit MUST compile and pass all non-ignored tests.
- Commits MUST be logical increments leaving the repo at a functional checkpoint.
- Skipped/ignored tests MUST carry a reason and a tracking issue or ADR reference.
