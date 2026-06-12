# Tasks: Topic Registry (Mock, In-Memory)

**Input**: Design documents from `specs/013-topic-registry/`

**Prerequisites**: plan.md, spec.md (FR-001..019, SC-001..010, US1–US4, Clarifications 2026-06-11), research.md (D1–D10), data-model.md, contracts/topic-registry.md, quickstart.md, ADR 0016.

**Tests**: **MANDATORY (TDD).** Constitution Principle II names *both* "registry interaction" *and* "message verification" critical, test-first areas — this feature touches both (a new registry + two new accept-path drop conditions). Every story's test task is authored first and MUST fail against the preceding skeleton/stub before its implementation lands. Tests assert on `TopicRegistryEvent`s, `effective_subscriptions()` / `received_messages()` snapshots, and returned `Vec<Effect>` — **never on log content** (constitution: logs are operator UX; the new `topic_not_registered` / `publisher_not_authorized` causes are not test-anchored). **Declarative test construction (constitution v1.2.0):** multi-step event scripts MUST be built through compact test-only builders beside the type — a new `TopicRegistryScript` + `TopicRegistryEvent` constructors in `src/topic_registry/test_support.rs` (mirroring the merged `MembershipScript`), reusing `MembershipScript` for the membership half of mixed `apply` scripts — not inline struct-literal construction per step.

**ADRs**: ADR 0016 (interface + node integration) was authored at plan time. No further structural decisions expected — if task execution surfaces one, stop and author the ADR first (Principle III).

**Execution order note**: phases are listed by story priority, but execution follows the green-checkpoint sequence. The **registry module (US1)** is built and tested first and is fully independent of the node event loop (SC-007) — it is the mergeable MVP. **Node integration (US2 topic-validity → US3 publisher-authorization → US4 network)** layers on it and on feature 004's merged pure core. Each checkpoint commit leaves `cargo fmt --check && cargo build && cargo clippy --all-targets && cargo test` green (constitution: green checkpoints). Note `Node::new`'s signature changes in US2 (third registry generic) and the topic registry is **mandatory + enforced**, so existing delivery tests must register their topics there.

## Format: `[ID] [P?] [Story] Description`

---

## Phase 1: Setup

**Purpose**: Baseline + the one shared fixture.

- [ ] T001 Verify baseline green on branch `013-topic-registry`: `cargo fmt --check && cargo build && cargo clippy --all-targets && cargo test` at the crate root. Record that `Node::new`'s signature gains a third registry generic in US2 (Phase 4) and that the topic registry is always enforced, so `tests/` delivery callers and `tests/common` helpers are edited there to register the topics they send on.
- [ ] T002 [P] Add a topic-registry test fixture `tests/fixtures/topic-registry.toml` with `[[topic]]` tables: `weather` with two lowercase-hex `publishers`, `sports` with an empty/absent `publishers` (open topic), `chat` open. Used by `from_file` tests in US1. (Authorization integration tests in US3/US4 register topics **programmatically** via `set_topic` with the actual test signer's `PublicKey`, since the fixture's hex keys are illustrative and need not match a signer.)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The registry module skeleton + shared-type prerequisites every story builds on (plan §Structure Decision; ADR 0016). Stubs make tests compile and fail.

- [ ] T003 In `src/crypto/mod.rs`, add `Ord, PartialOrd` to `PublicKey`'s derive list (it wraps `Vec<u8>`, which is `Ord`; purely additive — enables `BTreeSet<PublicKey>`, research D9). No other change to `PublicKey`. Confirm green.
- [ ] T004 In `src/error.rs`, add `ConfigError::DuplicateTopicEntry(String)` and `ConfigError::InvalidPublisherKey(String)` variants (topic-registry file load failures, contracts §C), each with a `thiserror` `#[error(...)]` message. Confirm green.
- [ ] T005 Create the module `src/topic_registry/mod.rs` + `src/topic_registry/in_memory.rs` and wire it in `src/lib.rs` (`mod topic_registry;` + the `pub use` block from contracts §A — six items: `InMemoryTopicRegistry, TopicRegistry, TopicRegistryControl, TopicRegistryError, TopicRegistryEvent, TopicRegistryWatch`). In `mod.rs`: `pub trait TopicRegistry: Send + Sync + 'static` (read-only, node-facing) with a single **global** `watch(&self) -> impl Future<Output = Result<TopicRegistryWatch, _>> + Send` (RPITIT with an explicit `Send` bound, no scoping arg — research D2); a separate `#[allow(async_fn_in_trait)] pub trait TopicRegistryControl: TopicRegistry` with `set_topic(&self, TopicId, BTreeSet<PublicKey>)` + `remove_topic(&self, TopicId)` (write surface — node never depends on it); `#[non_exhaustive] pub enum TopicRegistryEvent { Registered { topic, publishers }, PublishersChanged { topic, added, removed }, Removed { topic } }` (publisher sets are `BTreeSet<PublicKey>`, empty ⇒ open); `pub struct TopicRegistryWatch` wrapping `tokio::sync::mpsc::UnboundedReceiver<TopicRegistryEvent>` (not `Clone`; `recv(&mut self) -> Option<TopicRegistryEvent>`; `#[cfg(test)] try_next`); `#[non_exhaustive] pub enum TopicRegistryError` impl `std::error::Error + Debug + Display` (a minimal `Backend(String)` variant). In `in_memory.rs`: `pub struct InMemoryTopicRegistry` holding `topics: Mutex<HashMap<TopicId, BTreeSet<PublicKey>>>` + `subscribers: Mutex<Vec<UnboundedSender<TopicRegistryEvent>>>` (no per-watcher filter — the watch is global) with `new()`, `Default`, and **stub** trait-method impls (return `Ok(())` / an empty watch) so US1 tests compile and fail. Also add `src/topic_registry/test_support.rs` (constitution v1.2.0 declarative-test-construction standard, mirroring `src/subscription_registry/test_support.rs`): test-only `TopicRegistryEvent` constructors taking plain string topic ids + hex/`PublicKey` publisher lists (`registered`, `publishers_changed`, `removed`) and a `TopicRegistryScript` builder chaining them into an ordered sequence; gate it (`#[cfg(test)] mod test_support;` + `pub(crate) use test_support::TopicRegistryScript;`) so the `state.rs` node-side tests can reuse it alongside the merged `MembershipScript`. Rustdoc is implementation-neutral (no FR cites); `//` comments may cite FR-001..003. Compiles green; node untouched.

**Checkpoint**: module skeleton exists, unused by the node; suite still green.

---

## Phase 3: User Story 1 — A topic registry of legitimate topics and their publishers (Priority: P1) 🎯 MVP

**Goal**: topics + authorized publishers can be registered, changed, and removed, and `watch()` yields the current set as a `Registered` cold-start burst then live deltas — the standalone registry (FR-001..010; SC-001/002/006/007).

**Independent Test**: against `InMemoryTopicRegistry` alone, drive `set_topic`/`remove_topic`/`from_file`, open `watch()`, and assert the emitted `TopicRegistryEvent` sequence — **no node** (spec US1; SC-007).

### Tests for User Story 1 (MANDATORY — written first, MUST fail against T005 stubs)

- [ ] T006 [US1] In `src/topic_registry/in_memory.rs` `#[cfg(test)] mod tests`: **cold start** — with `weather→{k1}`, `sports→{k1,k2}`, `chat→{}` registered, `watch()` yields a `Registered` per topic carrying its publisher set, `chat` present with an empty set (open, **not** absent) (SC-001, US1-AS1); **live deltas** — after draining the burst, `set_topic(news,{k3})` ⇒ one `Registered{news,{k3}}` (US1-AS2); `set_topic(weather,{k1,k4})` ⇒ one `PublishersChanged{weather, added:{k4}, removed:{}}` (US1-AS3); `remove_topic(sports)` ⇒ one `Removed{sports}` (US1-AS4); **idempotency** — re-`set_topic` with the identical set (incl. an unchanged empty/open set) emits **no** event (SC-006, US1-AS5/6); **open-vs-removed** — `set_topic(t,{})` retains `t` open, distinct from `remove_topic(t)`; **from_file** — `from_file("tests/fixtures/topic-registry.toml")` ⇒ the burst reports the loaded topics + publishers; a file with a duplicate `id` ⇒ `ConfigError::DuplicateTopicEntry`; malformed hex ⇒ `ConfigError::InvalidPublisherKey`; an unknown field ⇒ `ConfigError::Parse` (FR-004, parse-at-the-edge); **drop** — dropping the `TopicRegistryWatch` ends cleanly with no effect on other watches (Edge Cases); **atomicity** — opening `watch()` then immediately issuing a write yields that write **exactly once** in the drained sequence (no gap, no duplicate at the burst/live boundary) (FR-007). No log assertions. `proptest` optional for the upsert/idempotency property. Fail against stubs.

### Implementation for User Story 1

- [ ] T007 [US1] Implement `set_topic` (upsert `topics`; compute `added`/`removed` vs the prior publisher set; first registration → `Registered`, changed → one `PublishersChanged`, unchanged → no-op no event), `remove_topic` (remove the entry → `Removed`), `from_file` (TOML deserialize at the boundary into `topics`; strict unknown-field rejection per 001; duplicate `id` → `DuplicateTopicEntry`; decode each publisher hex string → `PublicKey` via a small module-internal hex helper, bad hex → `InvalidPublisherKey`; the module-internal entry type has **only** `id` + optional `publishers` — strict `deny_unknown_fields` applies uniformly, governance fields are not part of the mock format, so any field outside `id`/`publishers` is a load error, no accepted-but-ignored fields — analyze **F1** resolved by simplification), and `watch` (**atomically under the lock**, snapshot the current topics as the `Registered` cold-start burst, register the subscriber's `UnboundedSender`, then return the `TopicRegistryWatch`; fan out each write's event to every subscriber, pruning closed senders — global, no scoping). `new()` builds an empty registry. Unbounded channel (ADR 0007). **Checkpoint commit #1 — registry module complete and independently tested (US1; SC-001/002/006/007). MVP: standalone registry, no node required.**

**Checkpoint**: the topic registry is a usable, tested, standalone module (the MVP).

---

## Phase 4: User Story 2 — A node only subscribes to registered topics (Priority: P2)

**Goal**: the node folds the topic-registry stream into a `registered_topics` projection in `NodeState`; its effective subscription set becomes `subscriptions ∩ registered_topics`, and messages on subscribed-but-unregistered topics are dropped (FR-011..014, FR-016; SC-003/004/009/010). Builds on feature 004's pure core (ADR 0011/0012) and the US1 registry.

**Independent Test**: pure, synchronous — scripted `Vec<Event>` mixing `TopicRegistryUpdate` + `MembershipUpdate` through `apply`, asserting `effective_subscriptions` and `received_messages` and `Vec<Effect>` (spec US2; contract §E).

### Tests for User Story 2 (MANDATORY — written first, MUST fail before T009)

- [ ] T008 [US2] In `src/state.rs` `#[cfg(test)] mod tests`: feed scripted events through `apply` for self-id `S`: `TopicRegistryUpdate(Registered{weather,{}})` + `MembershipUpdate(Joined{S,{weather,ghosttopic}})` ⇒ `effective_subscriptions == {weather}` (`ghosttopic` excluded — not registered, SC-003, US2-AS1); a signed message on `ghosttopic` is **dropped** (not recorded) while a signed message on `weather` from any publisher (open) with a valid signature **is recorded** (US2-AS4 / SC-010); then `TopicRegistryUpdate(Registered{ghosttopic,{}})` ⇒ `effective_subscriptions` gains `ghosttopic` (dynamic, SC-004, US2-AS2); then `TopicRegistryUpdate(Removed{weather})` ⇒ `weather` leaves the effective set and a later `weather` message is dropped (US2-AS3); every `apply` returns an empty `Vec<Effect>`. `proptest` optional for the topic-validity invariant over arbitrary interleavings. Fail against the skeleton handler from T009.

### Implementation for User Story 2

- [ ] T009 [US2] In `src/event.rs` add `Event::TopicRegistryUpdate(TopicRegistryEvent)` (enum stays `#[non_exhaustive]`). In `src/state.rs`: add `registered_topics: HashMap<TopicId, BTreeSet<PublicKey>>` to `NodeState` + an `effective_subscriptions(&self) -> Vec<TopicId>` accessor (the `subscriptions ∩ registered_topics.keys()` intersection); add one dispatch line in `apply` to a private `handle_topic_registry_update(&mut NodeState, TopicRegistryEvent) -> Vec<Effect>` that folds **only** `registered_topics` (`Registered` inserts/replaces the publisher set, `PublishersChanged` applies the add/remove diff, `Removed` drops the entry); returns `Vec::new()`. In `handle_signed_message`, add the **registered?** check after the existing subscribed-check and before signature verification: if `!registered_topics.contains_key(topic)` drop with cause `topic_not_registered` (operator UX). Leaves `subscriptions`/`candidates` (008) untouched. Makes T008 pass.
- [ ] T010 [US2] Wire the shell in `src/node.rs`, `src/main.rs`, and the `tests/` callers (public-surface change, contracts §B): `Node::new` adds the topic registry **generically** as a third registry param — `async fn new<N: Network, R: SubscriptionRegistry, T: TopicRegistry>(…, registry: Arc<R>, topic_registry: Arc<T>)` (`Arc<T>`, *not* `Arc<dyn>`); register a node-owned reader producer via `spawn_producer` (a named `async fn` that calls `topic_registry.watch()`, drains it, and pushes `Event::TopicRegistryUpdate`, symmetric with the 008 membership reader; logs at `error` if the watch cannot open). Add the public `Node::effective_subscriptions(&self) -> Vec<TopicId>` getter (lock-and-clone). Update `main.rs` to build `InMemoryTopicRegistry::from_file` and pass it. Update **every** existing `Node::new` call site in `tests/` + `tests/common` helpers to the new signature, injecting a topic registry that registers the topics each test delivers on (open or with the test signer's key) and awaiting effective-subscription convergence before send-then-observe (the `await_effective_subscriptions` helper, mirroring 008's `await_subscriptions`). `Node::subscriptions`/`candidates`/`peers` unchanged; `NodeError` unchanged.
- [ ] T011 [US2] Integration tests in `tests/` (topic-validity + isolation): a node configured `S→{weather,ghosttopic}` (subscription registry) against a topic registry where only `weather` is registered converges to `effective_subscriptions == {weather}` and drops `ghosttopic` traffic (SC-003); registering `ghosttopic` later makes it effective without restart (SC-004); a registered+subscribed+open topic with a valid signature is recorded exactly as pre-013 (no regression, SC-010); the topic-registry projection never alters `peers`/`candidates`/`subscriptions` data (SC-009). **Checkpoint commit #2 — topic-validity node integration delivered.**

**Checkpoint**: a node effectively subscribes only to registered topics; the topic-validity invariant holds; existing delivery tests pass with topics registered.

---

## Phase 5: User Story 3 — A node rejects messages from unauthorized publishers (Priority: P3)

**Goal**: on the inbound signed-message path, a message whose publisher is not in its topic's non-empty authorized set is dropped before signature verification; open topics accept any publisher (FR-015; SC-005). Additive to the US2 accept path.

**Independent Test**: pure, synchronous — `apply` signed messages through `NodeState` with `registered_topics` folded, asserting accept/drop by publisher and ordering (spec US3; contract §E).

### Tests for User Story 3 (MANDATORY — written first, MUST fail before T013)

- [ ] T012 [US3] In `src/state.rs` `#[cfg(test)] mod tests`: with `S` effectively subscribed to `weather` and `weather`'s authorized publishers folded as `{k1}`: a validly-signed `weather` message from `k1` ⇒ recorded (US3-AS1); from `k2` (not authorized) ⇒ dropped, not recorded (US3-AS2); after folding `weather` to open (`PublishersChanged` removing `k1`, or `Registered{weather,{}}`) the `k2` message ⇒ recorded (US3-AS3); a **valid-signature** message from an unauthorized publisher is still dropped, and a **tampered** (invalid-signature) message from an *authorized* publisher is dropped on verification — demonstrating the authorization check is ordered **before** verification (US3-AS4 / FR-015); every `apply` returns an empty `Vec<Effect>`. `proptest` optional for the authorization invariant. Fail before T013.

### Implementation for User Story 3

- [ ] T013 [US3] In `src/state.rs` `handle_signed_message`, add the **authorized?** check between the registered-check (T009) and signature verification: let `pubs = registered_topics[topic]`; if `!pubs.is_empty() && !pubs.contains(signed.plain.publisher_id.as_public_key())` drop with cause `publisher_not_authorized` (operator UX); an empty `pubs` (open) accepts any publisher. Existing causes/order otherwise unchanged. Makes T012 pass. **Checkpoint commit #3 — publisher authorization delivered (N-003 publisher-auth slice closed).**

**Checkpoint**: unauthorized publishers are rejected before verification; open topics accept any; the authorized-publisher invariant holds.

---

## Phase 6: User Story 4 — A network of nodes shares one topic registry (Priority: P4)

**Goal**: multiple nodes sharing one `Arc<InMemoryTopicRegistry>` (alongside the shared subscription registry) enforce the same legitimacy + authorization decisions, with no operator (FR-009, US4; SC-008).

**Independent Test**: build three `Node`s sharing one subscription registry + one topic registry; poll `effective_subscriptions()`/`received_messages()` to steady state and assert per-node effective sets + accept/drop by publisher (spec US4).

### Tests for User Story 4 (MANDATORY)

- [ ] T014 [US4] Integration test in `tests/` mirroring quickstart §3: a shared `InMemorySubscriptionRegistry` (`node-a→{weather}`, `node-b→{weather,sports}`, `node-c→{weather,ghosttopic}`) + a shared `InMemoryTopicRegistry` (`weather→{k1}`, `sports→{}` open; `ghosttopic` **not** registered), one `Arc` each across the three nodes (configured by `node_id` + bootstrap only); poll each node's `effective_subscriptions()` to steady state and assert `node-c` drops `ghosttopic` (SC-003, US4-AS1); a `weather` message from `k1` is accepted by every `weather` subscriber while one from a non-`k1` publisher is dropped by all (SC-005/SC-008, US4-AS2); registering a new topic `news→{k1}` is picked up by nodes whose subscription-list entry includes `news`, without restart (US4-AS3). Asserts on `effective_subscriptions()`/`received_messages()` snapshots only.
- [ ] T015 [US4] Add any multi-node test-harness helper needed to make T014 ergonomic (e.g. extend the `tests/common` builder to wire N nodes to a shared network + subscription registry + topic registry). No production-code change expected beyond US2/US3 — if T014 forces one, stop and review. **Checkpoint commit #4.**

**Checkpoint**: end-to-end multi-node demonstration with both registries; uniform legitimacy + authorization.

---

## Phase 7: Polish & Cross-Cutting

- [ ] T016 [P] Rustdoc pass on `src/topic_registry/{mod,in_memory}.rs`, `Node::effective_subscriptions`, and the changed `Node::new` — stable library terms, **no FR/spec citations** (constitution: implementation-neutral). Also refresh the stale `Node` doc-comment that still references the removed `subscribe`/`unsubscribe` API (pre-ADR-0015 leftover on `main`).
- [ ] T017 [P] Walk `specs/013-topic-registry/quickstart.md` end-to-end; update snippets if names/signatures drifted from what landed.
- [ ] T018 Cross-feature + ledger updates: update `specs/IMPLEMENTATION_NOTES.md` N-003 to record the publisher-authorization slice **closed** by 013 (ADR 0016) vs what remains deferred to 012 (equivocation / parent-hash / sequence / deposit); confirm the new **N-009** identity-unification note (topic-registry publisher `PublicKey` ≡ subscription-list node id at 011) is present and accurate against the landed code; add a one-line "second node-owned registry reader (topic registry)" note to `specs/event-loop-and-registry-contract.md` (no contract change — reuses the existing `spawn_producer` seam; `Event::TopicRegistryUpdate` is a new variant, not a rename, so no 004-author sign-off needed).
- [ ] T019 Final full sweep (`cargo fmt --check && cargo build && cargo clippy --all-targets && cargo test`) and self-check the contracts §E verification items (lib.rs diff shows only the intended six `pub use` additions; `Node::new` third generic + `effective_subscriptions`; `handle_signed_message` checks ordered registered? → authorized? → verify; `grep "pub " src/topic_registry/in_memory.rs` shows the module-internal TOML decode type + hex helper private; `handle_topic_registry_update` private in `state.rs`; `PublicKey` gains only `Ord, PartialOrd`) ahead of the formal `/speckit-analyze` round (findings recorded in `analysis.md` per the constitution). Final commit.

---

## Dependencies & Execution Order

```text
T001 (baseline) ─ T002 [P] (fixture)
  └─ T003 (PublicKey Ord) ─ T004 (ConfigError variants) ─ T005 (module skeleton)
       └─ T006 (US1 tests, fail) ─ T007 (US1 set_topic/remove_topic/from_file/watch+fanout)   ← checkpoint commit #1  [MVP: standalone registry]
            └─ T008 (US2 fold tests, fail) ─ T009 (Event+apply fold+registered? check) ─ T010 (shell/main/tests wiring) ─ T011 (US2 invariant tests)  ← checkpoint commit #2
                 └─ T012 (US3 authz tests, fail) ─ T013 (authorized? check)             ← checkpoint commit #3
                      └─ T014 (US4 multi-node test) ─ T015 (harness helper)             ← checkpoint commit #4
                           └─ T016 [P] / T017 [P] ─ T018 ─ T019 (final)                 ← final commit
```

- **Strictly sequential** through T013 within the registry module / node files (single crate, overlapping files: `in_memory.rs`, `state.rs`, `node.rs`). T002 is the only early [P] (a fixture file); T016/T017 are the late [P] pair (rustdoc vs quickstart). T003/T004 are independent edits to separate files but listed sequentially for a clean foundational checkpoint.
- **Story completion order**: US1 → US2 → US3 → US4 (execution = acceptance priority).
- **Cross-feature dependency**: US2/US3/US4 require feature 004's merged `apply`/`NodeState`/`Effect`/`spawn_producer` and 008's `subscriptions` field + `SubscriptionRegistry` (both on `main`). US1 does not.

## Implementation Strategy

- **Checkpoint = commit**: five commits (T007, T011, T013, T015, T019), each green and bisectable (constitution: logical increments).
- **MVP = checkpoint commit #1**: the standalone, independently-tested topic-registry module (US1, SC-007) — mergeable and usable by tests/other features without the node.
- **TDD gate**: each story's test task precedes its implementation and must fail first (Constitution II — registry interaction **and** message verification are both critical here). No log-content assertions anywhere.
- **Mandatory-registry churn**: the topic registry is enforced, so T010 must register topics for every existing delivery test (the atomic call-site change, analogous to 008's `subscribed_topics` removal); a missed test surfaces as a `topic_not_registered` drop (empty `received_messages`), not a compile error — watch for it.
- **Stop-the-line rule**: if a task forces a new public item beyond contracts §A/§B, a new structural decision, or a log-event rename — stop and get maintainer/ADR review (Principle III).

## Notes

- The node is **strictly read-only** toward the topic registry: the write methods live on a separate `TopicRegistryControl: TopicRegistry` trait, so `Node`'s generic `Arc<T>` (`T: TopicRegistry`) has no write methods in scope. `set_topic`/`remove_topic` are exercised only by `from_file`, test harnesses, and operator stand-ins — never by `Node` (FR-001/FR-005; ADR 0016).
- `TopicRegistry` shares **no trait** with `SubscriptionRegistry` (008) — distinct on-chain artifacts (FR-001; ADR 0016). The two registries compose only via two independent `NodeState` folds intersected at accept time (research D4).
- The registered-topics projection lives in `NodeState`, written only by `handle_topic_registry_update`; it never mutates 008's `subscriptions`/`candidates` or the config `[[peers]]` field (SC-009).
- On-chain decode/governance types (owners/admins/replication/retention/`alive`) are **not** introduced here; the module boundary is fixed for the 012 reader (FR-003/FR-017).
