# Tasks: Cross-Registry Consistency Invariant + Declarative Topic Entry

> **Status: implemented (rebased onto merged 004-connections, 2026-06-17).** All tasks below are done. 004 merged to `main` mid-implementation, expanding scope — the extra work folded into the same tasks: T004's cascade also clears `upstream`/`downstream` (FR-010 flip); a new `MembershipEvent::SnapshotComplete` dial trigger (T005) replaces the removed `connection_setup_delay` timer; the readiness gate (T005) is built as an in-node oneshot between the topic and membership readers; 004's S7 test was reworked to the rejection behaviour and the 013 SC-004 tests retired. Full gate green (`fmt`, `clippy -D warnings`, all test binaries, doctests). See spec Clarifications 2026-06-17 + ADR 0020 amendment.

**Input**: Design documents from `specs/014-registry-consistency/`

**Prerequisites**: plan.md, spec.md (FR-001..015, SC-001..010, US1–US2, Clarifications 2026-06-15 ×5), research.md (D1–D9), data-model.md, contracts/registry-consistency.md, quickstart.md, ADR 0020 (amends 0016).

**Tests**: **MANDATORY (TDD).** Constitution Principle II names "registry interaction" critical, test-first — this reshapes the registry fold + the receive path. Each story's test task is authored first and MUST fail against the preceding skeleton before its implementation lands. Tests assert on `subscriptions_snapshot()` / `candidates_snapshot()` / `received_messages()` / registered-state getters and returned `Vec<Effect>` — **never on log content** (the `topic_not_registered` drop/anomaly causes are operator UX, not test-anchored). **Declarative test construction (constitution v1.2.0):** multi-step scripts reuse the merged `TopicRegistryScript` (gains a `snapshot_complete()` step) + `MembershipScript`, not inline struct literals.

**ADRs**: ADR 0020 (cross-registry consistency, defensive folds, readiness gate; amends 0016) was authored at plan time. No further structural decisions expected — if execution surfaces one, stop and author the ADR first (Principle III).

**Execution order note**: phases follow the green-checkpoint sequence. **US1 (the maintained invariant + cascade + readiness gate)** is the P1 deliverable and reshapes the pure-core folds; **US2 (the declarative `TopicEntry`)** is a behaviour-preserving refactor layered on top. US1 keeps `registered_topics` as `HashMap<TopicId, BTreeSet<PublicKey>>` (no `TopicEntry` needed for the invariant); US2 reshapes the value type to `TopicEntry` and moves the receive-path open-topic check onto it. Each checkpoint leaves `cargo fmt --check && cargo build && cargo clippy --all-targets && cargo test` green. (Authored against pre-004 `main`; **rebased onto merged 004** mid-implementation — connection fields now exist, so T004's cascade extends to `upstream`/`downstream`, the readiness gate (T005) doubles as the dial trigger, and the `connection_setup_delay` timer is removed. See the status banner above.)

## Format: `[ID] [P?] [Story] Description`

---

## Phase 1: Setup

**Purpose**: Baseline + flag the reworks this feature forces.

- [X] T001 Verify baseline green on branch `014-registry-consistency`: `cargo fmt --check && cargo build && cargo clippy --all-targets && cargo test` at the crate root. Record the two reworks coming: (a) `handle_membership_update` / `handle_topic_registry_update` / `subscriptions_snapshot` in `src/state.rs` change from read-time intersection to a maintained invariant (US1); (b) the 013 test asserting subscribe-before-register-becomes-effective (013 SC-004) is **removed**, and `Node::new`'s producer-spawn order changes (US1). No code change in this task.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The additive `SnapshotComplete` protocol-event variant both the readiness gate (US1) and the reworked scripts reference. Compiles green, behaviour unchanged until US1 wires it.

- [X] T002 In `src/topic_registry/mod.rs` add `TopicRegistryEvent::SnapshotComplete` (no payload) to the existing `#[non_exhaustive]` enum, documented as the marker terminating a watch's cold-start `Registered` burst (contracts §A; data-model §4). In `src/state.rs` `handle_topic_registry_update`, add a `SnapshotComplete => {}` no-op arm so the explicit match still compiles (the variant is consumed at construction in US1; the fold is idempotent on it). In `src/topic_registry/test_support.rs` add a `snapshot_complete()` constructor and a `TopicRegistryScript::snapshot_complete()` step. `InMemoryTopicRegistry::watch()` does **not** yet emit it (US1 T005). Rustdoc implementation-neutral; `//` may cite FR-005. Confirm green; observable behaviour unchanged.

**Checkpoint**: the readiness marker exists in the type system, unused by the watch/node; suite still green.

---

## Phase 3: User Story 1 — The node never holds a subscription to an unregistered topic (Priority: P1) 🎯 MVP

**Goal**: maintain `subscriptions ⊆ registered_topics.keys()` and `candidates.keys() ⊆ registered_topics.keys()` as `NodeState` invariants — strict drop (self + candidates), defensive registry fold, atomic cascade on `Removed`, and the cross-stream readiness gate so strict drop is correct at cold start (FR-001..005a, FR-008..010, FR-012; SC-001/002/003/008/009/010). Replaces 013's read-time intersection.

**Independent Test**: pure, synchronous — scripted `Vec<Event>` mixing `TopicRegistryUpdate` + `MembershipUpdate` through `apply`, asserting both invariants after every step, strict drop, defensive fold, and the cascade; plus a multi-node cold-start convergence integration test (contract §C/§E).

### Tests for User Story 1 (MANDATORY — written first, MUST fail before T004)

- [X] T003 [US1] In `src/state.rs` `#[cfg(test)] mod tests` (reusing `TopicRegistryScript` + `MembershipScript`): **invariant** — for a self-id `S`, after each step of a mixed script assert both `subscriptions_snapshot() ⊆ registered` and `candidates.keys() ⊆ registered` (SC-001; `proptest` over random interleavings optional); **strict drop (self)** — fold `Registered{weather,{}}`, `SnapshotComplete`, `Joined{S,{weather,ghost}}` ⇒ `subscriptions_snapshot() == {weather}` (`ghost` never entered, SC-008); **no auto-promotion** — then `Registered{ghost,{}}` ⇒ `ghost` still absent; a fresh `TopicsChanged{S,+ghost}` ⇒ now present (data-model §5); **candidate gating** — `Joined{B,{weather,ghost}}` ⇒ `candidates_snapshot(weather)==[B]`, `candidates_snapshot(ghost)==[]` (SC-008); **defensive fold** — `PublishersChanged{ghost,+k1}` with `ghost` unregistered ⇒ `ghost` **not** created (no `or_default`), `is_registered(ghost)==false` (SC-010); **atomic cascade** — with `S` subscribed to `weather` and `B` a `weather` candidate, `Removed{weather}` ⇒ `subscriptions`, `candidates(weather)`, and the projection all empty of `weather` after the one fold (SC-002/003); **no-regression** — a registered+subscribed+open topic with a valid signature is still recorded (SC-004 baseline path); every `apply` returns `Vec::new()`. No log assertions. Fail against the unchanged folds.

### Implementation for User Story 1

- [X] T004 [US1] In `src/state.rs` reshape the folds to the maintained invariant (data-model §5; contract §C): `handle_membership_update` — for the node's **own** entry, admit only currently-registered topics to `subscriptions` (drop others, log `cause = "topic_not_registered"`), `TopicsChanged` adds only registered topics; for **other** nodes, record a `(peer, topic)` candidate only if `topic` is registered (else drop + log); `handle_topic_registry_update` — make it **defensive**: only `Registered` creates an entry, `PublishersChanged` for an unregistered topic drops + logs (no `or_default`), `Removed` performs the **atomic cascade** (remove the topic from `subscriptions`, `candidates`, and `registered_topics` in the one fold), `SnapshotComplete` stays a no-op (T002); `subscriptions_snapshot()` returns the maintained set directly (delete the read-time `subscriptions ∩ registered_topics` intersection). Add an `is_registered(&TopicId)` (or reuse `registered_topics.contains_key`) test accessor if needed. `registered_topics` stays `HashMap<TopicId, BTreeSet<PublicKey>>` here (US2 reshapes it). Makes T003 pass. **No commit here:** activating strict drop **breaks pre-existing 013 tests** that assert the read-time-intersection / subscribe-before-register dynamic (the 013 `state.rs` US2 pure-core test **and** the 013 multi-node integration test). The working tree is intentionally red from T004 until T006 reworks them — the green-checkpoint rule applies to **commits**, and the first green commit in this slice is T006. (T004 + T005 + T006 are one logical green increment; do not push a partial.)

- [X] T005 [US1] Wire the readiness gate (data-model §6; contract §E; research D5). In `src/topic_registry/in_memory.rs` `watch()`: after the cold-start `Registered` burst and before registering the live subscriber, send one `TopicRegistryEvent::SnapshotComplete` (so a fresh watch always observes burst-then-marker-then-deltas, gap-free under the lock). In `src/node.rs` `Node::new`: open the topic-registry `watch()`, drain-and-fold events into `NodeState.registered_topics` **until** `SnapshotComplete` (seeding the projection), **then** spawn the membership reader producer, and spawn the topic-reader producer to continue draining the same watch for live deltas (it pushes `Event::TopicRegistryUpdate`). The membership reader therefore never folds before the registered set is warm. Adjust the existing producer-spawn block accordingly; `Node::new`'s signature is unchanged. (Still uncommitted — lands with T006 as the one green checkpoint; the gate is what makes the cold-start integration tests deterministic under strict drop.)

- [X] T006 [US1] **Rework + integration, then the first green commit of this slice.** Rework the pre-existing 013 tests that assumed the read-time-intersection model: in `src/state.rs` `#[cfg(test)] mod tests`, rework the 013 US2 pure-core test that asserted subscribe-before-register becomes effective (**013 SC-004**) into strict-drop assertions; in `tests/`, rework the 013/008 integration suites and `tests/common` convergence helpers to the maintained-set model (the readiness gate from T005 makes them deterministic). Add the cold-start multi-node convergence test (quickstart §3) — three nodes sharing one subscription registry + one topic registry (pre-populated, one node naming an unregistered `ghost`) converge **deterministically** to each node's `registered ∩ entry` with no spurious drop (SC-009); confirm the remaining delivery suites still pass (steady-state behaviour-preserving). **Checkpoint commit #1 — the maintained invariant integrated end to end: strict drop + defensive fold + atomic cascade (T004) + readiness gate (T005) + reworked 013 tests, all green together. This is the US1 MVP.**

**Checkpoint**: the node maintains both invariants for any interleaving; removal cascades atomically; cold-start converges without races; 013 SC-004 retired — all in one green commit.

---

## Phase 4: User Story 2 — Topic openness and publisher authorization are expressed declaratively (Priority: P2)

**Goal**: introduce `TopicEntry` and express the open-topic / publisher-authorization rule as named predicates, replacing the inline `set.is_empty() || set.contains(key)` — behaviour-preserving against the 013 accept/drop matrix (FR-006/007/008; SC-004/005). Layers on US1.

**Independent Test**: `TopicEntry` unit tests (openness + authorization) + the pure receive-path matrix asserting outcomes identical to the 013 baseline (contract §D/§F).

### Tests for User Story 2 (MANDATORY — written first, MUST fail before T008)

- [X] T007 [US2] In `src/topic_registry/topic_entry.rs` `#[cfg(test)] mod tests`: an entry built from an empty publisher set reports `is_open() == true` and `is_publisher_authorized(any) == true`; an entry from `{k1}` reports `is_open() == false`, `is_publisher_authorized(k1) == true`, `is_publisher_authorized(k2) == false`; `apply_publishers_diff` adds/removes correctly and an emptied set reports open again. Fail before T008 (type does not exist yet).

### Implementation for User Story 2

- [X] T008 [US2] Create `src/topic_registry/topic_entry.rs`: `pub(crate) struct TopicEntry { publishers: BTreeSet<PublicKey> }` with `is_open()`, `is_publisher_authorized(&PublicKey)` (= `is_open() || publishers.contains(key)`), `apply_publishers_diff(added, removed)`, `from_publishers(BTreeSet<PublicKey>)`; declare `mod topic_entry;` in `src/topic_registry/mod.rs` (crate-internal — **not** re-exported; the public `TopicRegistryEvent` keeps `BTreeSet<PublicKey>`, contract §F / research D7). In `src/state.rs` reshape `registered_topics` to `HashMap<TopicId, TopicEntry>`: `handle_topic_registry_update` builds `TopicEntry::from_publishers` on `Registered` and calls `apply_publishers_diff` on `PublishersChanged` (defensive fold + cascade unchanged). In **`src/state.rs` `handle_signed_message`** (the receive path lives in `state.rs`, not `message.rs` — `message.rs` is unchanged), replace the inline `authorized.is_empty() || authorized.contains(key)` open-topic check with `entry.is_publisher_authorized(key)` (the projection now yields a `TopicEntry`); check **position** and **outcome** unchanged. The pre-existing 013 US3 receive-path matrix tests stay green throughout this task — they are the continuous regression guard for this behaviour-preserving refactor. Makes T007 pass.

- [X] T009 [US2] In `src/state.rs` `#[cfg(test)] mod tests`, add an explicit pin asserting the full 013 accept/drop matrix through the pure receive path is **identical** after the `TopicEntry` refactor (open / restricted / authorized / unauthorized / unsubscribed / unregistered / valid+invalid signature) — behaviour preservation (SC-004/005); confirm no remaining inline emptiness-or-membership check on a bare publisher set at the call site (SC-005). (This makes explicit what the surviving 013 US3 tests already guard in T008.) **Checkpoint commit #2 — declarative `TopicEntry`, behaviour-preserving.**

**Checkpoint**: openness/authorization read as `entry.is_open()` / `entry.is_publisher_authorized(key)`; the 013 matrix is unchanged; `TopicEntry` is the seam for future governance fields.

---

## Phase 5: Polish & Cross-Cutting

- [X] T010 [P] Rustdoc pass on `src/topic_registry/topic_entry.rs`, the reshaped `handle_membership_update` / `handle_topic_registry_update`, `Node::new`'s readiness drain, and `subscriptions_snapshot` — stable library terms, **no FR/spec citations** (constitution: implementation-neutral).
- [X] T011 [P] Walk `specs/014-registry-consistency/quickstart.md` end to end; update snippets if names/signatures drifted from what landed.
- [X] T012 Cross-feature + ledger updates: update `specs/IMPLEMENTATION_NOTES.md` **N-015** / data-model staleness row **S7** to record the cross-registry chain-order invariant is now **established** here (maintained `subscriptions/candidates ⊆ registered`, defensive folds, readiness gate) and to state what the **004-connections rebase** must carry through (extend the `Removed` cascade to drop the removed topic's `upstream`/`downstream` entries; may then enforce registration at connection acceptance); add a one-line note to `specs/event-loop-and-registry-contract.md` that the topic-registry reader now **seeds before** the membership reader (no seam change — `SnapshotComplete` rides the existing watch + `spawn_producer`).
- [X] T013 Final full sweep (`cargo fmt --check && cargo build && cargo clippy --all-targets && cargo test`) and self-check the contract §A/§F verification items: `lib.rs` re-exports gain nothing but route `SnapshotComplete` through the existing `TopicRegistryEvent` re-export; `grep "pub " src/topic_registry/topic_entry.rs` shows `TopicEntry` is `pub(crate)` (not `pub`); `Node::new` signature unchanged; `handle_signed_message` check order unchanged (subscribed? → registered? → `is_publisher_authorized` → verify); `registered_topics` is `HashMap<TopicId, TopicEntry>`. Final commit ahead of `/speckit-analyze` (findings recorded in `analysis.md`).

---

## Dependencies & Execution Order

```text
T001 (baseline)
  └─ T002 (SnapshotComplete variant + no-op arm + script step)            [Foundational]
       └─ T003 (US1 invariant/strict-drop/cascade tests, fail)
            └─ T004 (US1 reshape folds → maintained invariant)            ─┐  (no commit:
                 └─ T005 (US1 readiness gate: watch emits + drain-then-spawn) │   working tree
                      └─ T006 (US1 rework 013 tests + cold-start integration)┘   red T004→T006)
                           ↳ checkpoint commit #1  [MVP: US1 invariant, integrated + green]
                           └─ T007 (US2 TopicEntry unit tests, fail)
                                └─ T008 (US2 TopicEntry + projection reshape + receive-path move)
                                     └─ T009 (US2 no-regression matrix)     ← checkpoint commit #2
                                          └─ T010 [P] / T011 [P] ─ T012 ─ T013 (final)
```

- **T004 + T005 + T006 are one logical green increment** — strict drop (T004) breaks pre-existing 013 assertions, the readiness gate (T005) makes the integration tests deterministic, and the 013-test rework (T006) restores green; only T006 commits. This is the green-checkpoint fix (analyze C1): the invariant cannot be split into a green sub-commit, so it lands whole.
- **Strictly sequential** through T009 — overlapping files (`src/state.rs` across T004/T006/T008; `src/topic_registry/` across T002/T005/T008). The only `[P]` pair is the late T010/T011 (rustdoc vs quickstart, different files).
- **Story order**: US1 (P1, the invariant + readiness) → US2 (P2, the declarative refactor). US1 is independently shippable (the MVP). US2 is behaviour-preserving on top.
- **Cross-feature dependency**: all of US1/US2 build on 013 (`TopicRegistry`/`registered_topics`/the accept path) + 008 (`subscriptions`/`candidates`/`MembershipScript`) + 004 event-loop (`apply`/`NodeState`/`Effect`/`spawn_producer`) + **004-connections** (merged: `upstream`/`downstream`, `ConnectionStrategy`, `handle_connection_setup`, `Event::ConnectionSetup`) — onto which this feature was rebased.

## Implementation Strategy

- **Checkpoint = commit**: two story commits (T006, T009) plus the final (T013), each green and bisectable. (The US1 invariant is **one** commit — T004+T005+T006 — because activating strict drop is not green until the readiness gate and the 013-test rework land with it; see analyze C1.)
- **MVP = checkpoint commit #1** (T006): the maintained invariant integrated end to end — strict drop + defensive fold + atomic cascade + the readiness gate + reworked tests, green. (T004 alone, the pure-core fold, is the conceptual core but is not independently green, so it is not a standalone checkpoint.)
- **TDD gate**: T003 before T004; T007 before T008 (Constitution II — registry interaction is critical). No log-content assertions anywhere.
- **Rework awareness**: T006 retires 013 SC-004 and reworks the convergence helpers; a missed delivery-test rework surfaces as a spurious `topic_not_registered` drop (empty `received_messages`), not a compile error — watch for it.
- **Stop-the-line rule**: if a task forces a new public item beyond `SnapshotComplete` (contracts §A), a connection-field touch (out of scope — 004 unmerged), or a new structural decision — stop and get ADR/maintainer review (Principle III).

## Notes

- **Strict drop, not hold/promote**: a single maintained `subscriptions` set; a membership topic not registered is dropped, never buffered; no auto-promotion (013 SC-004 removed). The chain follower's ordering + the readiness gate make this correct (research D1/D5/D8).
- **Defensive folds uniform**: subscriptions, candidates, and the registry projection each reject invariant-violating inputs ("validate, don't assume") — clarify 2026-06-15.
- **`TopicEntry` is crate-internal**: it is the node's projection representation; the public `TopicRegistryEvent` keeps `BTreeSet<PublicKey>`, so the 012 reader is unaffected beyond emitting `SnapshotComplete` (research D7).
- **No connection work**: `NodeState` has no `upstream`/`downstream` on `main`; this feature establishes the invariant the 004-connections rebase relies on (N-015 / S7), and touches nothing connection-related.
