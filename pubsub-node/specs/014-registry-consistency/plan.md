# Implementation Plan: Cross-Registry Consistency Invariant + Declarative Topic Entry

**Branch**: `014-registry-consistency` | **Date**: 2026-06-15 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/014-registry-consistency/spec.md` (+ Clarifications 2026-06-15, five resolutions); the merged **013 topic registry** (PR #55) this extends ([013 plan](../013-topic-registry/plan.md), ADR 0016); 008's membership fold (ADR 0013/0014) and 004's pure core (ADR 0011/0012); the event-queue seam in [`../event-loop-and-registry-contract.md`](../event-loop-and-registry-contract.md); the maintainer design discussion (PR #55 review, team meeting 2026-06-15).

## Summary

> **Rebased onto merged 004-connections (2026-06-17).** 004 merged to `main` mid-implementation; this plan was authored against pre-004 `main`. The scope expanded on rebase (see spec Clarifications 2026-06-17 + ADR 0020 amendment): (a) the `Removed` cascade now also clears `upstream`/`downstream` (FR-010 flipped from "deferred" to in-scope); (b) a new `MembershipEvent::SnapshotComplete` is the **dial trigger**, replacing 004's removed `connection_setup_delay` timer; (c) the readiness gate is built as an in-node oneshot between the two readers; (d) S7/N-015 is resolved. The Design Notes / Source-tree below describe the pre-rebase shape; the as-built deltas are in ADR 0020's amendment.

Elevate the cross-registry consistency that 013 computed as a **read-time intersection** into a **maintained `NodeState` invariant** with **strict drop**, **atomic cascade**, and a **defensive fold** — and refactor the per-topic publisher set into a declarative `TopicEntry`. Uniform posture: *validate, don't assume* across all three folds.

- **The invariant** (`subscriptions ⊆ registered_topics.keys()` and `candidates.keys() ⊆ registered_topics.keys()`): the node holds a single `subscriptions` set that is always a subset of the registered topics — no separate declared/pending buffer, no read-time masking. 013's `subscriptions_snapshot` intersection is replaced; the `subscriptions()` getter returns the maintained set directly.
- **Strict drop**: a self-membership event naming an unregistered topic does **not** enter `subscriptions` (logged, cause `topic_not_registered`); a candidate (other node) on an unregistered topic is **not** recorded; a `PublishersChanged` for a topic with no prior `Registered` is dropped (no `or_default` auto-create). Only `Registered` creates a topic. No auto-promotion — under chain ordering a subscription only arrives after its topic's registration.
- **Atomic cascade**: a topic-registry `Removed { topic }` clears the topic from `subscriptions`, from `candidates`, and from the registered-topics projection within the single synchronous `apply` fold — no inconsistent intermediate state.
- **Cross-stream readiness gate**: because strict drop evaluates each subscription against the *current* registered set, the node warms its registered-topics projection from the topic-registry cold-start burst **before** it begins folding membership. Mechanism: a `TopicRegistryEvent::SnapshotComplete` marker terminating the cold-start burst; `Node::new` drains the topic watch up to that marker (seeding the projection) before spawning the membership reader. This un-defers a minimal slice of the deferred "registry synchronization complete" signal and is the cross-stream ordering the chain follower provides in production.
- **`TopicEntry`**: a declarative type wrapping the authorized-publisher set with `is_open()` / `is_publisher_authorized(key)` (the future home for owners/admins), replacing the inline `set.is_empty() || set.contains(key)` in the receive path. `registered_topics` becomes `HashMap<TopicId, TopicEntry>`.

This is **behaviour-preserving for the steady state** (registered, stays-registered topics accept exactly as in 013/008/003); the observable changes are the eager atomic cascade, strict drop of subscriptions/candidates to unregistered topics, and the defensive fold. **013 SC-004 (subscribe-before-register → effective) is removed**, superseded by the ordering guarantee. (Post-rebase: connection-path work IS included — the cascade clears `upstream`/`downstream`, the `MembershipEvent::SnapshotComplete` dial trigger replaces the `connection_setup_delay` timer, and S7/N-015 is resolved; see the rebase note above.)

## Technical Context

**Language/Version**: Rust, edition 2021, `rust-version = 1.75` (unchanged).

**Primary Dependencies**: `tokio` (the watch channels + reader-task ownership — ADR 0001/0007), `tracing` (operator drop/anomaly logs — ADR 0003). **No new dependencies.**

**Storage**: in-memory `NodeState` projections; no persistence, no new file formats (013's topic-registry file is reused unchanged).

**Testing**: `cargo test` — (a) pure state-machine tests feeding scripted `Vec<Event>` mixing `TopicRegistryUpdate` + `MembershipUpdate` through `apply`, asserting both subset invariants after every step, strict drop (subscriptions + candidates), the defensive fold (no `or_default` create), the atomic cascade, and `Vec<Effect>` emptiness; (b) `TopicEntry` unit tests (openness + authorization predicates); (c) receive-path behaviour-preservation against the 013 accept/drop matrix; (d) an integration test that a cold-start multi-node bring-up converges deterministically (the readiness gate, no spurious drop). `proptest` available for the invariant (a natural property), not required.

**Target Platform**: same as 001–013 (local hosts; in-process `InMemoryNetwork` + `InMemorySubscriptionRegistry` + `InMemoryTopicRegistry`).

**Project Type**: single Rust crate — library + thin CLI binary.

**Performance Goals**: none introduced. The folds stay O(delta): strict-drop is an O(1) `registered_topics.contains_key` per membership topic; the cascade is O(1) removals per structure; `TopicEntry` predicates are O(1)/O(set) lookups as before. The readiness gate adds a one-time cold-start drain at construction (O(#registered topics)), already paid by the existing burst.

**Constraints**: pure fold + receive path exercisable with no async runtime (the readiness gate is the only async-construction touch); the **two invariants** hold for any stream interleaving (SC-001); **strict drop with no auto-promotion** (SC-008); **defensive fold** create-only-on-`Registered` (SC-010); **atomic cascade** leaves no per-topic residue (SC-002/003); **behaviour-preserving** receive path (SC-004); two registries stay **separate** — the readiness gate adds ordering, not a merge (SC-006); node stays read-only toward both registries; logs never a test surface.

**Scale/Scope**: `src/state.rs` — `registered_topics` becomes `HashMap<TopicId, TopicEntry>`; `handle_membership_update` gains strict drop (self) + candidate gating (others); `handle_topic_registry_update` gains the defensive fold + the `Removed` cascade + the `SnapshotComplete` no-op arm; `subscriptions_snapshot` returns the maintained set directly, and `handle_signed_message` (which lives in `state.rs`, **not** `message.rs`) swaps its inline open-topic check for `TopicEntry::is_publisher_authorized`. New `TopicEntry` type in `src/topic_registry/topic_entry.rs` (research D7). `src/topic_registry/mod.rs` — `TopicRegistryEvent` gains `SnapshotComplete` (`#[non_exhaustive]`, additive); `src/topic_registry/in_memory.rs` — `watch()` emits `SnapshotComplete` after the burst. `src/node.rs` — `Node::new` drains the topic watch to `SnapshotComplete` before spawning the membership reader (reader spawn reorder). `src/message.rs` is **unchanged** (it holds the message types, not the receive path). `tests/common` + the 013/008 suites — reworked from the read-time-intersection model to strict drop (the 013 SC-004 test replaced by strict-drop + readiness coverage). One ADR (0020), amending 0016. `IMPLEMENTATION_NOTES` N-015 / S7 updated.

## Constitution Check

*GATE: evaluated before Phase 0; re-evaluated after Phase 1 design — both pass.*

- **I. Correctness Over Optimization** — ✅ Every behaviour traces to: spec.md FR-001..015 + SC-001..010 + the five 2026-06-15 clarifications; the maintainer design discussion (strict drop, atomic cascade, dedicated publisher structure, exclude metadata); the chain-follower ordering premise (`docs/node-lifecycle/` — both registries are chain-derived artifacts); 013/ADR 0016 (the projection + accept path this reshapes); 008/ADR 0014 (the membership fold this gates) and 004/ADR 0011/0012 (the pure core); `IMPLEMENTATION_NOTES` N-015 / data-model S7 (the cross-registry invariant this establishes). No optimization breaks the trace; the readiness gate is a correctness requirement of strict drop, not a performance choice.
- **II. Test-Driven for Correctness Claims** — ✅ **Critical: the constitution names "registry interaction" as MUST-TDD, and this reshapes the registry fold + accept path.** `/speckit-tasks` MUST order, per slice: the invariant + strict-drop + cascade + defensive-fold state-machine tests before the `state.rs` fold changes; the `TopicEntry` unit tests before the type wires into the receive path; the receive-path no-regression test (013 matrix) before the inline-check move; the cold-start readiness convergence test before/with the `Node::new` reorder. The two subset invariants (SC-001) are natural `proptest` properties.
- **III. Document Structural Decisions as ADRs** — ✅ One ADR: **ADR 0020** `docs/decisions/0020-cross-registry-consistency-and-readiness.md` (authored with this plan, **amends ADR 0016**) — the maintained single-set invariant + strict drop (replacing 013's read-time intersection); symmetric candidate gating; the defensive topic-registry fold (create-only-on-`Registered`, replacing `or_default`); the **atomic cascade** semantics; the **`SnapshotComplete` readiness marker + `Node::new` drain-then-spawn ordering** (the un-deferred minimal sync slice) and why a synchronous-drain or a point-read snapshot was rejected; the `TopicEntry` type and its placement. The removal of 013 SC-004 is recorded as a consequence.
- **IV. Specifications as Ambiguity Detectors** — ✅ The cross-registry event-ordering ambiguity the 013 PR flagged (N-015 / S7) is **resolved here, not silently**: the spec Clarifications + ADR 0020 record the chain-order premise, node-side enforcement by dropping, and the readiness gate. The `or_default`-vs-defensive tension (013's lenient fold vs the new posture) was surfaced in clarify and resolved (defensive), with FR-008 amended rather than quietly diverging.
- **V. Specifications Are Read-Only** — ✅ No edits to `pubsub/docs/` or `pubsub/formal_spec/`. `event-loop-and-registry-contract.md` (agent-editable workstream doc) gains at most a one-line note that the topic-registry reader now seeds before the membership reader; no seam change (the readiness marker rides the existing watch stream + `spawn_producer`).

**Engineering Standards applied**: **logs are operator UX** — every test asserts on the subscription/candidate/registered snapshots and `received_messages()`, never on the `topic_not_registered` drop logs; the new anomaly log reuses the `message_dropped`/`cause` convention and is not test-anchored. **Operator strings implementation-neutral.** **Parse at the edge** — no new parsing; the readiness gate is in-memory event handling. **Forward-compatible interfaces** — `SnapshotComplete` is an additive `#[non_exhaustive]` variant justified by the **012 on-chain reader** (its async chain-sync needs exactly this readiness signal, which a mock-only synchronous-drain would not generalize to) and by 004 (the invariant precondition); `TopicEntry` is the ROADMAP-justified seam for 012 governance fields. **Declarative test construction** (v1.2.0) — the reworked multi-fold scripts reuse/extend `TopicRegistryScript` + `MembershipScript` (the 013/008 builders), not inline literals; `SnapshotComplete` gets a script constructor. **Reproducible tests** — no wall-clock; deterministic folds; seeded mock crypto. **No new dependencies.**

## Project Structure

### Documentation (this feature)

```text
specs/014-registry-consistency/
├── spec.md              # /speckit-specify + Clarifications 2026-06-15 (5 resolutions)
├── plan.md              # This file
├── research.md          # Phase 0: D1..D9 design decisions
├── data-model.md        # Phase 1: NodeState invariant, TopicEntry, fold transitions, readiness
├── quickstart.md        # Phase 1: pure-fold vignettes + cold-start convergence
├── contracts/
│   └── registry-consistency.md  # Phase 1: fold semantics + SnapshotComplete + TopicEntry + getter surface
├── checklists/
│   └── requirements.md  # Spec quality checklist (complete; 0 open markers)
└── tasks.md             # /speckit-tasks output (NOT created by /speckit-plan)
```

ADRs (live outside the feature dir):

```text
docs/decisions/
└── 0020-cross-registry-consistency-and-readiness.md   # authored with this plan; amends 0016
```

### Source Code (repository root)

```text
src/
├── state.rs                # EXTENDED — registered_topics: HashMap<TopicId, TopicEntry>;
│                           #   handle_membership_update: strict drop (self subscriptions) + candidate gating (others);
│                           #   handle_topic_registry_update: defensive fold (create-only-on-Registered, no or_default),
│                           #     Removed cascade (subscriptions + candidates + projection, atomic in-fold),
│                           #     SnapshotComplete no-op arm;
│                           #   subscriptions_snapshot returns the maintained set directly (no intersection);
│                           #   handle_signed_message: the inline open-topic check (authorized.is_empty() || contains)
│                           #     is replaced by entry.is_publisher_authorized(key) — the receive path lives HERE,
│                           #     not in message.rs; position + outcome unchanged (US2);
│                           #   #[cfg(test)] mod tests — both invariants, strict drop, candidate gating, defensive fold,
│                           #     cascade, no-regression matrix, Vec<Effect> emptiness
├── topic_registry/
│   ├── mod.rs              # CHANGED — TopicRegistryEvent gains SnapshotComplete (#[non_exhaustive], additive);
│   │                       #   doc: the burst now terminates with SnapshotComplete
│   ├── in_memory.rs        # CHANGED — watch() emits SnapshotComplete after the cold-start Registered burst
│   ├── test_support.rs     # CHANGED — TopicRegistryScript gains a snapshot_complete() step + constructor
│   └── topic_entry.rs      # NEW — pub(crate) struct TopicEntry { publishers }
│                           #   is_open() / is_publisher_authorized(&PublicKey); apply_publishers_diff(added, removed)
├── node.rs                 # CHANGED — Node::new opens the topic watch, drains+folds to SnapshotComplete
│                           #   (seeding registered_topics) BEFORE spawning the membership reader; then spawns the
│                           #   topic-reader producer to drain live deltas from the same watch
├── lib.rs                  # CHANGED — re-export SnapshotComplete via the existing TopicRegistryEvent re-export
│                           #   (TopicEntry stays pub(crate) — internal projection; not a public surface item)
└── (message.rs event.rs error.rs crypto/ peer.rs network.rs subscription_registry/ — UNCHANGED;
     message.rs in particular carries no receive-path logic — handle_signed_message is in state.rs)

tests/
├── common/mod.rs           # EXTENDED — convergence helpers updated to the maintained-set model; a cold-start
│                           #   multi-node helper that asserts deterministic convergence (readiness gate)
├── (013/008 suites — reworked from read-time-intersection to strict drop; the 013 SC-004 subscribe-before-register
│    test is replaced by strict-drop + readiness coverage)
└── registry_consistency_*.rs  # NEW — invariant + cascade + candidate gating + defensive fold + cold-start convergence
```

**Structure Decision**: the change is concentrated in the existing pure core (`src/state.rs`) and the registry module (`src/topic_registry/`), preserving 013's layout. `TopicEntry` is **crate-internal** (`pub(crate)`) — it is the node's projection representation, not a public surface item; the public `TopicRegistryEvent` keeps carrying `BTreeSet<PublicKey>` (the fold builds a `TopicEntry` from it), so no public type leaks and the 012 reader is unaffected beyond emitting `SnapshotComplete`. The readiness marker rides the **existing single watch stream** (no second accessor, no point-read — honoring 013 FR-001's watch-only design), and the gate is realized as **construction ordering** in `Node::new` (drain to marker, then spawn membership reader), keeping the two registries separate (FR-009).

## Design Notes (decision-record pointers)

Consolidated in [research.md](./research.md); structural rationale in ADR 0020 (amends 0016).

1. **Maintained single subscription set + strict drop, not read-time intersection** — `NodeState.subscriptions` is kept ⊆ `registered_topics` by the fold; a membership topic not registered is dropped (not buffered), no auto-promotion. Replaces 013's two-sets-ANDed-at-accept-time (013 ADR 0016 note 4). `subscriptions_snapshot` returns the set directly. (research D1; spec FR-001/003/005a)
2. **Symmetric candidate gating** — `candidates` records a (peer, topic) only if the topic is registered; same invariant on `candidates.keys()`. Hands 004 a registered-only candidate set. (research D2; spec FR-003a)
3. **Defensive topic-registry fold (create-only-on-`Registered`)** — `PublishersChanged` for an unregistered topic is dropped + logged (no `or_default`); `Removed` of an unknown topic is a no-op; only `Registered` creates. Amends 013's lenient fold (013 FR-013). (research D3; spec FR-008)
4. **Atomic cascade on `Removed`** — the single `apply` fold drops the topic from subscriptions, candidates, and the projection together; the synchronous-under-lock fold (ADR 0012) makes it atomic by construction — no partial state is observable. (research D4; spec FR-002)
5. **`SnapshotComplete` readiness marker + drain-then-spawn ordering** — the topic watch's cold-start burst terminates with `SnapshotComplete`; `Node::new` drains the watch to that marker (seeding `registered_topics`) before spawning the membership reader. Chosen over (a) a synchronous drain-until-empty, which assumes the burst is fully queued before `watch()` returns — true for the mock, **false for 012's async chain-sync**; and over (b) a point-read snapshot accessor, which reintroduces the point-read 013 FR-001 deliberately excluded. The marker generalizes to 012 and stays watch-only. (research D5; spec FR-005)
6. **`TopicEntry` declarative type** — `pub(crate) struct TopicEntry { publishers: BTreeSet<PublicKey> }` with `is_open()` (empty set) and `is_publisher_authorized(&PublicKey)` (open OR contains); the receive path calls these instead of the inline check; the future home for owners/admins (012). Crate-internal — the public `TopicRegistryEvent` keeps `BTreeSet<PublicKey>`. (research D6/D7; spec FR-006/007/008)
7. **013 SC-004 removed** — the subscribe-before-register dynamic is intentionally superseded by strict drop + the ordering guarantee; its test is reworked. Recorded as an ADR 0020 consequence and an N-015 update. (research D8; spec FR-004/012)
8. **Connection cascade** — ~~deferred~~ **in scope (superseded 2026-06-17 rebase)**: the `Removed` cascade clears `upstream`/`downstream`, S7/N-015 is resolved, and the `MembershipEvent::SnapshotComplete` dial trigger replaces the removed `connection_setup_delay` timer. (research D9; spec FR-010; ADR 0020 amendment)

## Complexity Tracking

No constitution violations; table omitted. (The readiness gate adds construction ordering, justified as a correctness requirement of strict drop and forward-compatible with the 012 reader — not speculative generality.)
