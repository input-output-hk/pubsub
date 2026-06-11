# Implementation Plan: Topic Registry (Mock, In-Memory)

**Branch**: `013-topic-registry` | **Date**: 2026-06-11 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/013-topic-registry/spec.md` (+ Clarifications 2026-06-11); the sibling subscription-list feature [008](../008-node-registry/plan.md) (merged, PR #51) whose shape this parallels; the event-queue seam in [`../event-loop-and-registry-contract.md`](../event-loop-and-registry-contract.md); protocol grounding in `../docs/node-lifecycle/{README,topic-creation}.md` and the formal model `../formal_spec/topic_registry/` (READ-ONLY).

## Summary

The mock, in-memory **topic registry** — the topic-governance half of the on-chain artifacts (`topic id → authorised publisher keys`), distinct from 008's subscription list. It is the source of truth for **which topics legitimately exist** and **who may publish to each** (an empty publisher set = *open* topic). Two halves, mirroring 008:

- **Registry module** (`src/topic_registry/`, independent of the node loop): a read-only, node-facing `TopicRegistry` trait (a single **global** `watch` — no scoping argument, unlike 008's node-keyed watch) + a separate `TopicRegistryControl: TopicRegistry` write trait (`set_topic`, `remove_topic`; operator/test surface) + `InMemoryTopicRegistry` (implements both; seeded from a TOML *topic-registry file* via `from_file`, or programmatically); `TopicRegistryEvent` (`Registered`/`PublishersChanged`/`Removed`, topic id + authorized publisher keys); `TopicRegistryWatch` (single-consumer, push, mirrors `MembershipWatch`/`NetworkHandle` — global cold-start `Registered` burst then live deltas over an unbounded channel).
- **Node integration** (on feature 004's merged pure core, alongside 008's membership fold): a new `Event::TopicRegistryUpdate(TopicRegistryEvent)` variant + a named `handle_topic_registry_update` handler in `apply` folding the stream into a **registered-topics projection** in `NodeState` (`TopicId → authorized publishers`); a node-owned reader producer (via `spawn_producer`) draining the `watch()` stream. Two effects on the existing signed-message accept path (`handle_signed_message`): the **effective subscription set** becomes the intersection of the membership-derived subscriptions (008) and the registered topics (a subscription-list topic absent from the registry is dropped + logged, cause `topic_not_registered`), and inbound messages are dropped (cause `publisher_not_authorized`) when their publisher key is not in the topic's non-empty authorized set — both checks cheap, before signature verification.

This changes `Node::new` (adds the topic registry generically as `Arc<T>` where `T: TopicRegistry`, a third registry parameter beside `Network` and `SubscriptionRegistry`). Because the topic registry is now mandatory at construction and enforced, every test that exercises message delivery MUST register its topics — the analog of 008's `subscribed_topics`-removal churn (an atomic call-site task).

## Technical Context

**Language/Version**: Rust, edition 2021, `rust-version = 1.75` (unchanged).

**Primary Dependencies**: `tokio` (unbounded mpsc for the watch + reader task ownership — ADR 0001/0007), `tracing` (operator logs — ADR 0003), `serde` + `toml` (topic-registry file parsing — ADR 0002, already direct deps). **No new dependencies** — hex decoding of publisher keys at the file edge is a small hand-rolled helper (the crate already hand-rolls lowercase-hex *encoding* in `crypto`; no `hex` crate is introduced).

**Storage**: in-memory registry state; a TOML *topic-registry file* read once at `from_file` construction (parse-at-the-edge), separate from 008's subscription-list file. No persistence.

**Testing**: `cargo test` — (a) registry-module unit tests (write via `TopicRegistryControl` → `watch` → assert the `TopicRegistryEvent` sequence + global cold-start burst), independent of the node loop; (b) synchronous pure-core tests feeding scripted `Vec<Event>` mixing `TopicRegistryUpdate` + `MembershipUpdate` through `apply`, asserting the effective-subscription intersection, publisher-authorization accept/drop, and `Vec<Effect>` emptiness; (c) integration tests for the multi-node topology over shared `Arc`s (getter polling to steady state, the 003/008 `await_*` convergence pattern). `proptest` available, not required.

**Target Platform**: same as 001–008 (local hosts; in-process `InMemoryNetwork` + `InMemorySubscriptionRegistry` + `InMemoryTopicRegistry`).

**Project Type**: single Rust crate — library + thin CLI binary.

**Performance Goals**: none introduced; the registered-topics fold is O(#publishers in the delta) per event; cold-start burst is O(#registered topics); the accept-path checks are O(1) set lookups.

**Constraints**: registry module exercisable with no node event loop (spec FR-019); pure fold + accept-path checks exercisable with no async runtime; **topic-validity invariant** — a node never effectively subscribes to an unregistered topic, for any stream interleaving (spec SC-003); **authorized-publisher invariant** — a message from an unauthorized publisher on a non-open topic is never recorded (SC-005); node strictly read-only toward the registry (SC-009); unbounded channel, no backpressure (ADR 0007); additive — no regression for the registered/subscribed/authorized/valid-signature case (SC-010); the topic-registry projection never touches the config `[[peers]]` field or 008's membership-derived data (SC-009).

**Scale/Scope**: new module `src/topic_registry/` (`mod.rs` + `in_memory.rs`); `src/state.rs` gains the `registered_topics` projection field, `handle_topic_registry_update`, and the two new accept-path checks in `handle_signed_message`; `src/event.rs` gains the `TopicRegistryUpdate` variant; `src/node.rs` gains the third generic registry param + the topic-registry reader producer; `src/error.rs` gains a duplicate-topic-entry + invalid-publisher-key variant on `ConfigError`; `src/crypto/mod.rs` gains `Ord, PartialOrd` derives on `PublicKey` (purely additive, for `BTreeSet<PublicKey>`); `src/main.rs` constructs the topic registry; `src/lib.rs` re-exports the new public items; `src/topic_registry/test_support.rs` adds the `TopicRegistryScript` builder + `TopicRegistryEvent` constructors (constitution v1.2.0 declarative-test-construction standard); `tests/common` gains a topic-registry fixture and existing delivery tests register their topics. One new ADR (0016).

## Constitution Check

*GATE: evaluated before Phase 0; re-evaluated after Phase 1 design — both pass.*

- **I. Correctness Over Optimization** — ✅ Every behavior traces to: spec.md FR-001..019 + SC-001..010 + the 2026-06-11 clarification; the formal model `../formal_spec/topic_registry/` (`Topic.publishers` empty ⇒ open; topic legitimacy; authorised publisher keys) and `../docs/node-lifecycle/{README,topic-creation}.md` (topic registry vs subscription list = two distinct on-chain artifacts; the registry is read so relayers verify signatures against authorised keys); the 008 design it parallels (ADR 0013/0014 — registry-derived state, `Control`-trait write split) and ADR 0016 (this feature, authored with this plan); prior ADRs 0007 (actor-handle pattern this watch mirrors) and 0011/0012 (the 004 pure core this extends); `IMPLEMENTATION_NOTES` N-003 (publisher-authorization slice closed here).
- **II. Test-Driven for Correctness Claims** — ✅ **This feature is critical: the constitution names both "registry interaction" and "message verification" as MUST-TDD areas, and this feature touches both** (a new registry + new accept-path drop conditions). Tests precede implementation. `/speckit-tasks` MUST order, per slice: registry-module tests before the `InMemoryTopicRegistry` impl; pure-fold + accept-path state-machine tests before `handle_topic_registry_update` and the `handle_signed_message` changes; the topic-validity-invariant test (SC-003) and authorized-publisher-invariant test (SC-005) and the no-regression test (SC-010) before/with the node wiring. Property formulations (idempotent upsert SC-006; topic-validity SC-003; authorization SC-005) are natural `proptest` candidates.
- **III. Document Structural Decisions as ADRs** — ✅ One ADR: **ADR 0016** `docs/decisions/0016-topic-registry-interface-and-node-integration.md` (authored with this plan) — the `TopicRegistry`/`TopicRegistryControl` split (parallel to ADR 0014); the **global** (non-node-keyed) watch choice and why it differs from 008; `TopicRegistryEvent`/`TopicRegistryWatch` mirroring ADR 0007; the `Event::TopicRegistryUpdate` seam + `handle_topic_registry_update`; the registered-topics projection in `NodeState` and the **two-independently-folded-sets-ANDed-at-accept-time** model for effective subscriptions (vs a stored derived set); the authorized-publisher accept-path check ordered before signature verification; the `Node::new` third-generic signature change. The additive `Ord` derive on `PublicKey` is tactical (local, additive) — noted in the ADR's consequences, not a separate ADR.
- **IV. Specifications as Ambiguity Detectors** — ✅ The formal model's richer `Topic` (owners/admins/replication/retention/`alive` soft-delete) is **not** silently collapsed: FR-017 explicitly defers governance + soft-delete to 012, and ADR 0016 records that the mock's `remove_topic` is a hard delete (the on-chain `alive`-flag semantics, retaining topic ids forever to prevent reassignment, are a 012 concern). The empty-publishers-⇒-open semantic is taken directly from the formal model (not invented). No new ambiguity arose during planning that is resolved silently.
- **V. Specifications Are Read-Only** — ✅ This plan proposes **no** edits to `pubsub/docs/` or `pubsub/formal_spec/`; those are read as grounding only. `event-loop-and-registry-contract.md` (an agent-editable workstream doc in this crate) needs no change — the topic-registry reader reuses the existing `spawn_producer` seam exactly as 008's reader does; a one-line "second registry reader" note may be added when this lands, no contract change.

**Engineering Standards applied**: logs are operator UX — registry/fold/accept tests assert on `TopicRegistryEvent`s, effective-subscription + candidate snapshots, and `received_messages()`, never on log content; the new drop causes (`topic_not_registered`, `publisher_not_authorized`) follow the existing `message_dropped`/`cause` convention and are not test-anchored. Operator-facing strings stay implementation-neutral. **Parse at the edge** — the topic-registry file (including hex publisher keys) is parsed in `from_file` at the construction boundary; `apply`, `NodeState`, and the trait take already-decoded `TopicId`/`PublicKey` values; on-chain decode/governance types are deferred and will be module-internal (spec FR-003). **Forward-compatible interfaces** — `#[non_exhaustive]` `TopicRegistryEvent` + `TopicRegistryError`; the registry consumed generically as `Arc<T>` at construction (an `async fn`/RPITIT trait is not `dyn`-compatible); RPITIT `watch` with `Send`; all justified by the named 012 consumer (the real on-chain topic-registry feed), not speculative generality. **Reproducible tests** — no wall-clock dependence; the registry is deterministic; publisher keys come from the existing seeded mock crypto. **No new dependencies** (hand-rolled hex decode at the edge). **Declarative test construction** (constitution v1.2.0, new standard) — the multi-step event scripts in this feature's tests (registry write sequences, and `apply` sequences mixing `TopicRegistryUpdate` + `MembershipUpdate`) are built through compact test-only builders beside the type: a new `TopicRegistryScript` + `TopicRegistryEvent` constructors in `src/topic_registry/test_support.rs` (mirroring the merged `MembershipScript` in `src/subscription_registry/test_support.rs`, which the node-side tests reuse for the membership half), not inline struct-literal construction per step.

## Project Structure

### Documentation (this feature)

```text
specs/013-topic-registry/
├── spec.md              # /speckit-specify output (+ Clarifications 2026-06-11)
├── plan.md              # This file
├── research.md          # Phase 0: consolidated design decisions
├── data-model.md        # Phase 1: TopicRegistry / TopicRegistryEvent / registered-topics projection
├── quickstart.md        # Phase 1: drive the registry + a multi-node in-memory network with two registries
├── contracts/
│   └── topic-registry.md  # Phase 1: trait surface + node public-surface + accept-path delta
├── checklists/
│   └── requirements.md  # Spec quality checklist (complete; 0 open markers)
└── tasks.md             # /speckit-tasks output (NOT created by /speckit-plan)
```

ADRs (live outside the feature dir):

```text
docs/decisions/
└── 0016-topic-registry-interface-and-node-integration.md   # authored with this plan
```

### Source Code (repository root)

```text
src/
├── topic_registry/         # NEW — the registry module (independent of the node loop):
│   ├── mod.rs              #   pub trait TopicRegistry { watch }  (read, node-facing; single GLOBAL stream)
│   │                       #   pub trait TopicRegistryControl: TopicRegistry { set_topic, remove_topic }  (write)
│   │                       #   pub enum TopicRegistryEvent { Registered, PublishersChanged, Removed }  (#[non_exhaustive])
│   │                       #   pub struct TopicRegistryWatch  (single-consumer; wraps unbounded rx; not Clone)
│   │                       #   pub enum TopicRegistryError  (#[non_exhaustive])
│   └── in_memory.rs        #   pub struct InMemoryTopicRegistry  (state + subscriber channels; private internals)
│                           #     ::new() / ::from_file(path)  + module-internal TOML topic entry type + hex decode
│                           #   #[cfg(test)] mod tests — write/watch (cold-start burst, idempotency, open-vs-removed)
├── state.rs                # EXTENDED — NodeState gains `registered_topics: HashMap<TopicId, BTreeSet<PublicKey>>`;
│                           #   new private handler `handle_topic_registry_update(&mut NodeState, TopicRegistryEvent) -> Vec<Effect>`;
│                           #   handle_signed_message gains two checks (registered? authorized?) before signature verify;
│                           #   `apply` gains one dispatch line; `effective_subscriptions` snapshot accessor
│                           #   #[cfg(test)] mod tests — fold + intersection (SC-003), authorization (SC-005), no-regression (SC-010)
├── event.rs                # EXTENDED — Event gains `TopicRegistryUpdate(TopicRegistryEvent)` (still #[non_exhaustive])
├── node.rs                 # EXTENDED — Node::new takes the topic registry generically (Arc<T>, T: TopicRegistry) as a
│                           #   third registry param; spawns a node-owned topic-registry reader producer calling watch()
│                           #   (symmetric with the 008 membership reader); new getter `effective_subscriptions`
│                           #   (also: refresh the stale `subscribe`/`unsubscribe` doc-comment left from pre-ADR-0015)
├── error.rs                # CHANGED — ConfigError gains `DuplicateTopicEntry(String)` + `InvalidPublisherKey(String)`
├── crypto/mod.rs           # CHANGED (additive) — add `Ord, PartialOrd` to PublicKey's derives (for BTreeSet<PublicKey>)
├── main.rs                 # CHANGED — construct InMemoryTopicRegistry (from_file), pass to Node::new
├── lib.rs                  # CHANGED — `mod topic_registry;` + pub use of the new public items
└── (network.rs peer.rs received.rs topic.rs message.rs subscription_registry/ — UNCHANGED)

tests/
├── common/mod.rs           # EXTENDED — fixtures construct an InMemoryTopicRegistry; helpers register topics +
│                           #   await effective-subscription / topic-registry convergence before returning
├── (existing 002/003/004/008 integration suites — updated where Node::new's signature change + mandatory
│    topic-registry enforcement force topic registration for delivered topics)
└── topic_registry_*.rs / publisher_authorization_*.rs  # NEW — registry behaviors, topic-validity invariant,
                            #   publisher authorization, multi-node network with two registries
```

**Structure Decision**: the registry is its own module `src/topic_registry/` (a directory: `mod.rs` for the published trait + event + watch + error, `in_memory.rs` for the impl and the module-internal TOML entry type + hex decode), leaving room for an `on_chain.rs` at 012 without reshaping callers (spec FR-003 anti-corruption boundary), exactly as `src/subscription_registry/` is laid out. The registered-topics projection lives in the existing crate-internal `NodeState` (it is folded by a transition, so it is state), written **only** by `handle_topic_registry_update` — kept separate from 008's `subscriptions`/`candidates` fields so each registry's handler owns its own field and the effective subscription set is a *derived* read (intersection at accept time), never a stored field mutated by two handlers. The node consumes the topic registry only through `Event::TopicRegistryUpdate` fed by the node-owned `watch()` reader (the node never imports the impl beyond construction wiring).

## Design Notes (decision-record pointers)

Consolidated in [research.md](./research.md); structural rationale in ADR 0016.

1. **Topic registry is a distinct artifact — no shared trait with 008** — different key (topic id vs node id), payload (authorised publisher keys vs topic set + deposit), and reader (relayers verifying signatures vs subscribers computing candidate sets), per `docs/node-lifecycle/README.md`. `TopicRegistry` + `TopicRegistryControl` parallel 008's split but share nothing with it. (ADR 0016; spec FR-001)
2. **Global watch, not node-keyed** — `watch() → TopicRegistryWatch`; the node folds *all* registered topics (it must validate any subscription-list topic and authorize publishers on any subscribed topic). 008's `watch(node)` is node-keyed because membership is naturally scoped to a node's topics; topic legitimacy is global. Topic-scoping the watch would couple it to the membership stream and is a premature optimization with no ROADMAP consumer. (ADR 0016; spec FR-007)
3. **`empty publishers ⇒ open topic`** — taken directly from `formal_spec/topic_registry/types.qnt`. A registered topic with an empty authorized-publisher set accepts any publisher; an *unregistered* topic is invalid. The empty-but-registered vs absent distinction mirrors 008's empty-topics-vs-unregister. (spec FR-002; Edge Cases)
4. **Effective subscriptions = `subscriptions ∩ registered_topics`, ANDed at accept time** — two independently-folded sets, not a stored derived set: `handle_membership_update` owns `subscriptions` (008, unchanged), `handle_topic_registry_update` owns `registered_topics` (013). The accept-filter checks both; an `effective_subscriptions()` getter computes the intersection for observability/tests. This keeps handlers decoupled, handles arbitrary stream-arrival ordering for free, and makes a later-registered topic become effective with no extra wiring. (ADR 0016; spec FR-014, SC-003/SC-004)
5. **Authorized-publisher enforcement before signature verification** — `handle_signed_message` order: subscribed? → registered? → publisher authorized (open ⇒ any)? → verify signature → record. The three cheap set lookups precede the expensive verification, extending the existing "filter first" ordering; new drop causes `topic_not_registered` + `publisher_not_authorized` follow the `message_dropped`/`cause` convention. (ADR 0016; spec FR-015/FR-016)
6. **Strictly read-only node** — the write API (`set_topic`/`remove_topic`) lives on a **separate** `TopicRegistryControl` trait; `Node` holds the registry generically as `Arc<T>` (`T: TopicRegistry`, an `async fn`/RPITIT trait — not `dyn`-compatible), so it has no write methods in scope. Used by the file loader's equivalent and test harnesses, never the node daemon. (spec FR-001/FR-005; ADR 0016)
7. **Topic-registry file at the edge** — `from_file` parses TOML (`[[topic]]` with `id` + optional hex `publishers`) into the registry's initial state; `new()` builds an empty registry for programmatic tests; duplicate topic id and bad hex are load errors (`ConfigError::DuplicateTopicEntry` / `InvalidPublisherKey`). Governance fields are ignored. (spec FR-004; parse-at-the-edge)
8. **`Node::new` gains a third registry generic** — `Node::new<N: Network, R: SubscriptionRegistry, T: TopicRegistry>(…, Arc<T>)`. The topic registry is mandatory and always enforced; existing delivery tests must register their topics (atomic call-site change, like 008's `subscribed_topics` removal). `BTreeSet<PublicKey>` requires `Ord` on `PublicKey` — added as a purely-additive derive (tactical). (ADR 0016; spec FR-001; SC-010)

9. **Validation lives in the node, not the subscription registry** (design review 2026-06-11) — topic-validity is enforced by the node's fold + accept-path intersection (note 4), not by sanitizing invalid topics inside the subscription registry. The node consumes the topic registry directly for publisher authorization regardless, so validity is a free intersection on the same projection; sanitizing in the subscription registry would couple two independent on-chain artifacts, break the 012 reader swap, and bypass the event-queue fold. (research D11; ADR 0016; spec Clarifications)
10. **Node-facing projection is publishers-only; identity is `PublicKey`** — the formal model's `Topic` carries owners/admins/R/T/`alive`, but the node consumes none of them, so the projection is registered-topics + authorized-publishers (`BTreeSet<PublicKey>`) only; governance is deferred to 012 and the mock's writes are permissionless. Publisher keys are the same identity space the subscription list uses (node pubkey) in the protocol; the mock's `PeerId`/`PublicKey` split unifies at 011 (IMPLEMENTATION_NOTES N-009). (research D8/D12; spec Clarifications/Assumptions; ADR 0016)

## Complexity Tracking

No constitution violations; table omitted.
