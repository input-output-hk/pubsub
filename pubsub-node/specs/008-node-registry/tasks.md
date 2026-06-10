# Tasks: Subscription Registry (Mock, In-Memory)

**Input**: Design documents from `specs/008-node-registry/`

**Prerequisites**: plan.md, spec.md (FR-001..021, SC-001..009, US1–US4, Clarifications), research.md (R1–R8), data-model.md, contracts/subscription-registry.md, quickstart.md, ADR 0013, ADR 0014.

**Tests**: **MANDATORY (TDD).** Constitution Principle II names *registry interaction* a critical, test-first feature; the plan's Constitution Check requires tests before implementation. Every story's test task is authored first and MUST fail against the preceding skeleton/stub before its implementation task lands. Tests assert on `MembershipEvent`s, `entry()`/candidate-set snapshots, and returned `Vec<Effect>` — **never on log content** (constitution: logs are operator UX).

**ADRs**: ADR 0013 (source of truth) and ADR 0014 (interface + node integration) were authored at plan time. No further structural decisions expected — if task execution surfaces one, stop and author the ADR first (Principle III).

**Execution order note**: phases are listed by story priority, but execution follows the green-checkpoint sequence. The **registry module (US2 state → US1 stream)** is built and tested first and is fully independent of the node event loop (SC-006) — it is the mergeable MVP. **Node integration (US3 → US4)** layers on it and on feature 004's merged pure core. US1 remains the *acceptance* MVP (the membership stream is the deliverable); US2 (the state it streams) is simply implemented just before it. Each checkpoint commit leaves `cargo fmt && cargo build && cargo clippy --all-targets && cargo test` green (constitution: green checkpoints).

## Format: `[ID] [P?] [Story] Description`

---

## Phase 1: Setup

**Purpose**: Baseline + the one shared fixture.

- [X] T001 Verify baseline green on branch `feat/node-registry`: `cargo fmt --check && cargo build && cargo clippy --all-targets && cargo test` at the crate root. Record that `Node::new`'s signature and `NodeConfig` will change in US3 (Phase 5), so some `tests/` callers and TOML fixtures are edited there (unlike 004, this feature is not behavior-frozen).
- [X] T002 [P] Add a subscription-list test fixture `tests/fixtures/subscription-list.toml` with `[[entry]]` tables for `node-a → ["t1"]`, `node-b → ["t1","t2"]`, `node-c → ["t2"]` (used by `from_file` tests in US2 and the multi-node test in US4).

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The registry module skeleton every story builds on (plan §Structure Decision; ADR 0014). Stubs make tests compile and fail.

- [X] T003 In `src/network.rs`, rename the private `type Registry = Arc<RwLock<HashMap<PeerId, UnboundedSender<RoutingFrame>>>>` alias to `PeerSenders` (module-internal, mechanical — frees the name `Registry` family from confusion with this feature, spec FR-011). Confirm green.
- [X] T004 Create the module `src/subscription_registry/mod.rs` + `src/subscription_registry/in_memory.rs` and wire it in `src/lib.rs` (`mod subscription_registry;` + the `pub use` block from contracts §A). In `mod.rs`: `#[allow(async_fn_in_trait)] pub trait SubscriptionRegistry: Send + Sync + 'static` (read-only, node-facing) with `watch_members`, `entry`; a separate `pub trait SubscriptionRegistryControl: SubscriptionRegistry` with `set_topics`, `unregister` (write surface — node never depends on it) (signatures per data-model/contract); `#[non_exhaustive] pub enum MembershipEvent { Joined { node, topics }, TopicsChanged { node, added, removed }, Left { node } }`; `#[non_exhaustive] pub struct SubscriptionEntry { pub node: PeerId, pub topics: BTreeSet<TopicId> }`; `pub struct MembershipWatch` wrapping `tokio::sync::mpsc::UnboundedReceiver<MembershipEvent>` (not `Clone`; `recv(&mut self) -> Option<MembershipEvent>`); `#[non_exhaustive] pub enum SubscriptionRegistryError` impl `std::error::Error + Debug + Display`. In `in_memory.rs`: `pub struct InMemorySubscriptionRegistry { membership: Mutex<HashMap<PeerId, BTreeSet<TopicId>>>, subscribers: Mutex<Vec<(BTreeSet<TopicId>, UnboundedSender<MembershipEvent>)>> }` with `new()` and **stub** trait-method impls (return `Ok(())` / `Ok(None)` / an empty watch) so US1/US2 tests compile and fail. Rustdoc is implementation-neutral (no FR cites); `//` comments may cite FR-001..003. Compiles green; node untouched.

**Checkpoint**: module skeleton exists, unused by the node; suite still green.

---

## Phase 3: User Story 2 — Membership state & writes (Priority: P2) — **executed first**

**Goal**: A node's subscription-list entry can be declared, changed, withdrawn, and looked up; loadable from the subscription-list file (FR-004..006, FR-008).

**Independent Test**: against `InMemorySubscriptionRegistry` alone, drive `set_topics`/`unregister`/`from_file` and assert state via `entry()` read-back — **no watch and no node** (spec US2; SC-006).

### Tests for User Story 2 (MANDATORY — written first, MUST fail against T004 stubs)

- [X] T005 [US2] In `src/subscription_registry/in_memory.rs` `#[cfg(test)] mod tests`: assert via `entry()` read-back — `set_topics(A, {t1})` then `entry(A) == Some(SubscriptionEntry { topics: {t1}, .. })`; updating to `{t1,t2}` reflects in `entry`; `set_topics(A, {})` ⇒ `entry(A) == Some(empty topics)` and is **distinct** from `unregister(A)` ⇒ `entry(A) == None` (FR-005/FR-006); re-`set_topics` with the identical set leaves `entry` unchanged (idempotency substrate, SC-004); `from_file("tests/fixtures/subscription-list.toml")` ⇒ `entry` returns the loaded sets; a file with a duplicate `node_id` or an unknown field ⇒ load error (FR-004, parse-at-the-edge). `proptest` optional for the upsert/idempotency property. Fail against stubs.

### Implementation for User Story 2

- [X] T006 [US2] Implement `set_topics` (upsert the `membership` map; compute `added`/`removed` vs the prior set and hold it for emission — emission wiring lands in US1/T008), `unregister` (remove the entry), `entry` (clone-out a `SubscriptionEntry`), and `from_file` (TOML deserialize at the boundary into `membership`; strict unknown-field rejection per 001; duplicate `node_id` → load error; any `deposit` field ignored). `new()` builds an empty registry. **Checkpoint commit #1**: fmt + build + clippy + test green.

**Checkpoint**: registry state + writes + `entry` + `from_file` complete and unit-tested via read-back; the stream is next.

---

## Phase 4: User Story 1 — Membership stream / candidate source (Priority: P1) 🎯 acceptance MVP

**Goal**: `watch_members(topics)` yields the current members (cold-start `Joined` burst) then live, topic-scoped deltas — the candidate source (FR-007..010; SC-001/002/005).

**Independent Test**: against `InMemorySubscriptionRegistry` alone, populate via `set_topics`, open `watch_members`, and assert the emitted `MembershipEvent` sequence — no node loop (spec US1; SC-006).

### Tests for User Story 1 (MANDATORY — written first, MUST fail before T008)

- [X] T007 [US1] In `in_memory.rs` `#[cfg(test)] mod tests`: **cold start** — with `A→{t1}`, `B→{t1,t2}`, `C→{t2}` registered, `watch_members({t1})` yields `Joined` for exactly `A` and `B` (each reporting `t1`) and nothing for `C`/`t2` (SC-001, US1-AS1); **live deltas** — after draining the burst, `set_topics(D,{t1})` ⇒ one `Joined{D,{t1}}`, `unregister(A)` ⇒ one `Left{A}`, an interest change touching the watched set ⇒ one `TopicsChanged` with `added`/`removed` **intersected** with the watched set (US1-AS2/3); **scoping** — a change confined to an unwatched topic ⇒ **no** event (SC-005, US1-AS4); **empty-topic watch** — `watch_members({t4})` yields no burst, a later join is delivered (US1-AS5); **ordering/atomicity** — burst + live deltas are one gap-free, duplicate-free sequence (FR-009); **drop** — dropping the `MembershipWatch` ends cleanly with no effect on other watches (Edge Cases). No log assertions. Fail against stubs.

### Implementation for User Story 1

- [X] T008 [US1] Implement `watch_members`: **atomically under the `membership` + `subscribers` lock**, snapshot the current matching members into a `Joined` cold-start burst and register the subscriber's `UnboundedSender` (FR-009 boundary atomicity), then return the `MembershipWatch`. Wire the **fanout** into `set_topics`/`unregister` (from T006): after mutating state, emit the scoped, watched-topic-**intersected** `MembershipEvent` (`Joined`/`TopicsChanged`/`Left`) to each matching subscriber; prune closed senders; unchanged-set ⇒ no emission (SC-004). Unbounded channel (ADR 0007). **Checkpoint commit #2 — registry module complete and independently tested (US1 + US2; SC-001/002/004/005/006).**

**Checkpoint**: the registry is a usable, tested, standalone module (the MVP) — no node required.

---

## Phase 5: User Story 3 — Node reads its topics and folds candidates (Priority: P3)

**Goal**: the node sources its own topics from its `entry`, folds the membership stream into a self-excluded per-topic candidate set in `NodeState`, and stays read-only (FR-013..018; SC-003/007/009). Builds on feature 004's pure core (ADR 0011/0012) and the US1/US2 registry.

**Independent Test**: pure, synchronous — scripted `Vec<Event>` of `MembershipUpdate` through `apply`, asserting `NodeState` candidate sets and `Vec<Effect>` (spec US3; contract §5); plus the source-of-truth invariant test.

### Tests for User Story 3 (MANDATORY — written first, MUST fail before T010)

- [X] T009 [US3] In `src/state.rs` `#[cfg(test)] mod tests`: feed scripted `Event::MembershipUpdate(..)` sequences through `apply` for self-id `S` and assert `NodeState.candidates`: `Joined{A,{t1}}` ⇒ `t1→{A}`; `Joined{S,{t1}}` (own id) ⇒ **not** added anywhere (self-exclusion, SC-003, US3-AS2); `TopicsChanged{A, added:{t2}, removed:{t1}}` ⇒ `A∈t2, A∉t1`; `Left{A}` ⇒ `A` absent everywhere; every `apply` returns an empty `Vec<Effect>` (`Effect` uninhabited). `proptest` optional for self-exclusion over arbitrary sequences. Fail against the stub handler from T010's skeleton.

### Implementation for User Story 3

- [X] T010 [US3] In `src/event.rs` add `Event::MembershipUpdate(MembershipEvent)` (enum stays `#[non_exhaustive]`). In `src/state.rs`: add `candidates: HashMap<TopicId, HashSet<PeerId>>` to `NodeState` + a `candidates_snapshot(&self, &TopicId) -> Vec<PeerId>` accessor; add one dispatch line in `apply` to a private `handle_membership_update(&mut NodeState, MembershipEvent) -> Vec<Effect>` that folds `Joined`/`TopicsChanged`/`Left` into `candidates`, **excluding `self_id` locally** (SC-003, FR-016), returning `Vec::new()`. Makes T009 pass.
- [X] T011 [US3] Wire the shell in `src/node.rs`, `src/config.rs`, `src/error.rs`, `src/main.rs`, and the `tests/` callers (public-surface change, contracts §B/§C): `Node::new` drops `initial_subscriptions` and adds `registry: Arc<dyn SubscriptionRegistry>`; at startup it calls `entry(self_id)` → `None` ⇒ fail fast with a new `NodeError` registration-not-found variant (FR-018), else seed `NodeState.subscriptions` from `entry.topics`, call `watch_members(topics)`, and register the node-owned reader producer via `spawn_producer` (a named `async fn` draining the watch and pushing `Event::MembershipUpdate`, symmetric with `network_mailbox_loop`); add the public `Node::candidates(&TopicId) -> Vec<PeerId>` getter (lock-and-clone). Remove `subscribed_topics` from `NodeConfig` (and any TOML fixtures/templates that set it). Update `main.rs` to build `InMemorySubscriptionRegistry::from_file` and pass it. Update every existing `Node::new` call site in `tests/` + `tests/common` helpers to the new signature (drop `initial_subscriptions`, inject a registry pre-seeded with the test node's entry). `Node::peers()` unchanged.
- [X] T012 [US3] Integration tests in `tests/` (source-of-truth + N-007): a node constructed as `S` against a registry whose entry is `S→{t1}` accepts/participates only on `t1` regardless of any other configured value (SC-007); `Node::candidates(t)` is distinct from `Node::peers()` and does not alter the config bootstrap list (SC-009, FR-017); a node whose id has no entry fails construction (FR-018). **Checkpoint commit #3 — node integration delivered.**

**Checkpoint**: the node reads its topics from the registry, folds candidates, stays read-only; the source-of-truth invariant holds.

---

## Phase 6: User Story 4 — A network of in-memory nodes discovers itself (Priority: P4)

**Goal**: multiple nodes sharing one `Arc<InMemorySubscriptionRegistry>` (file-seeded) discover one another via their candidate sets, with no operator and no chain (FR-011, US4; SC-008).

**Independent Test**: build three `Node`s sharing one file-seeded registry `Arc`, configured by `node_id` only; poll `candidates()` to steady state and assert topic-scoped, self-excluded views of the others (spec US4).

### Tests for User Story 4 (MANDATORY)

- [X] T013 [US4] Integration test in `tests/` mirroring quickstart §3: load `InMemorySubscriptionRegistry::from_file("tests/fixtures/subscription-list.toml")` into one `Arc`, share across `Node`s `node-a`/`node-b`/`node-c` (configured by `node_id` + bootstrap only); poll each node's `candidates()` to steady state (003 `await_delivery` pattern) and assert `node-a`'s `t1` candidates `== {node-b}`, `node-b` sees `t1→{node-a}` + `t2→{node-c}`, `node-c`'s `t2` `== {node-b}`, each self-excluded (SC-008, US4-AS1); a 4th node `node-d→{t1}` (via `set_topics` on the shared registry) is observed by `node-a`/`node-b` but not `node-c` (US4-AS2); constructing `ghost` (no entry) errors (US4-AS3). Asserts on `candidates()` snapshots only.

### Implementation for User Story 4

- [X] T014 [US4] Add any multi-node test-harness helper needed to make T013 ergonomic (e.g. a `tests/common` builder that wires N nodes to a shared network + shared registry). No production-code change expected beyond US3 — if T013 forces one, stop and review (it would indicate missing US3 wiring). **Checkpoint commit #4.**

**Checkpoint**: end-to-end multi-node self-discovery demonstrated over the shared in-memory registry.

---

## Phase 7: Polish & Cross-Cutting

- [ ] T015 [P] Rustdoc pass on `src/subscription_registry/{mod,in_memory}.rs`, `Node::candidates`, and the changed `Node::new` — stable library terms, **no FR/spec citations** (constitution: implementation-neutral; FR refs live in `//` comments only).
- [ ] T016 [P] Walk `specs/008-node-registry/quickstart.md` end-to-end; update snippets if names/signatures drifted from what landed.
- [ ] T017 Cross-feature + ledger updates: add the restart-recovery deferred entry to `specs/IMPLEMENTATION_NOTES.md` (FR-018, revisit at 012) and note N-007's 008-side resolution; update `specs/event-loop-and-registry-contract.md`'s seam note and **— with the 004 author's sign-off —** ADR 0011's illustrative comment + the CLAUDE.md SpecKit block from the placeholder to `Event::MembershipUpdate(MembershipEvent)`.
- [ ] T018 Final full sweep (`cargo fmt && cargo build && cargo clippy --all-targets && cargo test`) and self-check the contracts §E verification items (lib.rs diff shows only the intended `pub use` additions; `grep "pub " src/subscription_registry/in_memory.rs` shows the entry-type/fields private; `handle_membership_update` private in `state.rs`) ahead of the formal `/speckit-analyze` round (findings recorded in `analysis.md` per the constitution). Final commit.

---

## Dependencies & Execution Order

```text
T001 (baseline) ─ T002 [P] (fixture)
  └─ T003 (network alias rename) ─ T004 (module skeleton)
       └─ T005 (US2 state tests, fail) ─ T006 (US2 state impl)            ← checkpoint commit #1
            └─ T007 (US1 stream tests, fail) ─ T008 (US1 watch+fanout)    ← checkpoint commit #2  [MVP: standalone registry]
                 └─ T009 (US3 fold tests, fail) ─ T010 (apply+NodeState fold) ─ T011 (shell/config/main/tests wiring) ─ T012 (US3 invariant tests)  ← checkpoint commit #3
                      └─ T013 (US4 multi-node test) ─ T014 (harness helper) ← checkpoint commit #4
                           └─ T015 [P] / T016 [P] ─ T017 ─ T018 (final)   ← final commit
```

- **Strictly sequential** through T012 within the registry module / node files (single crate, overlapping files: `in_memory.rs`, `state.rs`, `node.rs`). T002 is the only early [P] (a fixture file); T015/T016 are the late [P] pair (rustdoc vs quickstart).
- **Story completion order**: US2 → US1 → US3 → US4 (execution); **acceptance priority** remains US1 > US2 > US3 > US4.
- **Cross-feature dependency**: US3/US4 require feature 004's merged `apply`/`NodeState`/`Effect`/`spawn_producer` (already on `main`). US1/US2 do not.

## Implementation Strategy

- **Checkpoint = commit**: five commits (T006, T008, T012, T014, T018), each green and bisectable (constitution: logical increments).
- **MVP = checkpoint commit #2**: the standalone, independently-tested registry module (US1+US2, SC-006) — mergeable and usable by tests/other features without the node. US3 integrates it into the node; US4 demonstrates the multi-node topology.
- **TDD gate**: each story's test task precedes its implementation and must fail first (Constitution II — registry interaction is critical). No log-content assertions anywhere.
- **Stop-the-line rule**: if a task forces a new public item beyond contracts §A/§B, a new structural decision, or a log-event rename — stop and get maintainer/ADR review (Principle III) rather than working around it. The `Event::MembershipUpdate` seam-variant rename is a known cross-feature touch (T017) requiring the 004 author's sign-off.

## Notes

- The node is **strictly read-only** toward the registry: the write methods live on a separate `SubscriptionRegistryControl: SubscriptionRegistry` trait (not the node-facing `SubscriptionRegistry`), so `Node`'s `Arc<dyn SubscriptionRegistry>` has no write methods in scope. `set_topics`/`unregister` are exercised only by `from_file`, test harnesses, and operator stand-ins — never by `Node` (FR-001/FR-018; ADR 0013; analyze F3).
- The candidate set lives in `NodeState` and is **distinct** from the config `[[peers]]` bootstrap field, which is untouched (FR-017; N-007).
- On-chain decode/serialization types are **not** introduced here; the module boundary is fixed for the 012 reader (FR-003).
