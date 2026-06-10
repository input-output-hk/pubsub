# Implementation Plan: Subscription Registry (Mock, In-Memory)

**Branch**: `feat/node-registry` (spec dir `008-node-registry`) | **Date**: 2026-06-10 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/008-node-registry/spec.md`; shared seam contract from [`../event-loop-and-registry-contract.md`](../event-loop-and-registry-contract.md) (this feature is **Feature B** of that contract); source-of-truth decision in [ADR 0013](../../docs/decisions/0013-subscription-list-is-authoritative-for-node-interests.md); protocol grounding in `../docs/node-lifecycle/{README,joining}.md`.

## Summary

The mock, in-memory **subscription registry** — the node-membership half of the on-chain *subscription list* (`node pubkey → topic set`), distinct from the topic registry. It produces, per topic, the **candidate set** a future sampler/dialer draws from. Two halves:

- **Registry module** (`src/subscription_registry/`, independent of the node loop): a read-only, node-facing `SubscriptionRegistry` trait (a single node-keyed `watch`) + a separate `SubscriptionRegistryControl: SubscriptionRegistry` write trait (`set_topics`, `unregister`, operator/test surface) + `InMemorySubscriptionRegistry` (implements both; seeded from a TOML *subscription-list file* via `from_file`, or programmatically); `MembershipEvent` (`Joined`/`TopicsChanged`/`Left`, identity + topics only); `MembershipWatch` (single-consumer, push, mirrors `NetworkHandle`/ADR 0007 — node-keyed cold-start `Joined` burst then live deltas over an unbounded channel).
- **Node integration** (on feature 004's merged pure core): a new `Event::MembershipUpdate(MembershipEvent)` variant + a named `handle_membership_update` handler in `apply` folding the stream into a **per-topic candidate set** in `NodeState` (self-excluded) and the node's own subscription set (`node == self_id` events), exposed via a public `Node::candidates` getter; a node-owned reader producer (via `spawn_producer`) draining the `watch(self_id)` stream. The node is **strictly read-only** toward the registry: it learns its own topics from the head `Joined` of the watch cold-start burst (not a startup point-read), seeds an **empty** 002 subscription/filter and converges from the stream — it issues no writes.

This changes `Node::new` (drops `initial_subscriptions`, adds the registry generically as `Arc<R>` where `R: SubscriptionRegistry`) and removes 002's `subscribed_topics` config field — the node's topics now come from the subscription list, not config (ADR 0013, the source-of-truth invariant, spec SC-007).

## Technical Context

**Language/Version**: Rust, edition 2021, `rust-version = 1.75` (unchanged).

**Primary Dependencies**: `tokio` (unbounded mpsc for the watch + reader task ownership — ADR 0001/0007), `tracing` (operator logs — ADR 0003), `serde` + `toml` (subscription-list file parsing — ADR 0002, already direct deps). **No new dependencies.**

**Storage**: in-memory registry state; a TOML *subscription-list file* read once at `from_file` construction (parse-at-the-edge). No persistence.

**Testing**: `cargo test` — (a) registry-module unit tests (write → `watch` → assert the `MembershipEvent` sequence + node-keyed cold-start burst), independent of the node loop; (b) synchronous pure-core tests feeding scripted `Vec<Event>` of `MembershipUpdate` through `apply`, asserting candidate sets + `Vec<Effect>`; (c) integration tests for the multi-node topology over a shared `Arc` (getter polling to steady state, the 003 `await_delivery` pattern). `proptest` available, not required.

**Target Platform**: same as 001–004 (local hosts; in-process `InMemoryNetwork` + `InMemorySubscriptionRegistry`).

**Project Type**: single Rust crate — library + thin CLI binary.

**Performance Goals**: none introduced; the candidate-set fold is O(#watched-topics in the delta) per event; cold-start burst is O(members in watched topics).

**Constraints**: registry module exercisable with no node event loop (spec FR-021); pure fold exercisable with no async runtime (contract §5); **source-of-truth invariant** — a node's effective topics equal its subscription-list entry, never config (spec SC-007); node strictly read-only (spec FR-018, SC-009); unbounded channel, no backpressure (ADR 0007); candidate set distinct from the config `[[peers]]` bootstrap field (spec FR-017, N-007).

**Scale/Scope**: new module `src/subscription_registry/` (`mod.rs` + `in_memory.rs`); `src/state.rs` gains the candidate-set field + `handle_membership_update`; `src/event.rs` gains the `MembershipUpdate` variant; `src/node.rs` gains the generic registry param, the node-owned reader producer (calling `watch(self_id)`), and the `candidates` getter; `src/config.rs` drops `subscribed_topics`; `src/main.rs` constructs the registry; `src/error.rs` is **unchanged** (no new variant — construction never fails on a missing entry). One new ADR (0014) alongside the existing ADR 0013.

## Constitution Check

*GATE: evaluated before Phase 0; re-evaluated after Phase 1 design — both pass.*

- **I. Correctness Over Optimization** — ✅ Every behavior traces to: spec.md FR-001..021 + SC-001..009; `specs/event-loop-and-registry-contract.md` §2/§3/§5 (seam, ownership, test strategy); ADR 0013 (source of truth) and ADR 0014 (interface + node integration, authored with this plan); the protocol artifacts `../docs/node-lifecycle/{README,joining}.md` (subscription list = node membership; endpoints off-chain; node read-only); prior ADRs 0007 (actor-handle pattern this watch mirrors) and 0011/0012 (the 004 pure core this integrates with); `IMPLEMENTATION_NOTES` N-007 (peers placement).
- **II. Test-Driven for Correctness Claims** — ✅ **This feature is critical: the constitution names "registry interaction" as a MUST-TDD area.** Tests precede implementation. `/speckit-tasks` MUST order, per slice: registry-module tests before the `InMemorySubscriptionRegistry` impl; pure-fold state-machine tests before `handle_membership_update`; the source-of-truth-invariant test (SC-007) and multi-node integration test before the node wiring. Property formulations (idempotent upsert SC-004; self-exclusion SC-003; scoping SC-005) are natural `proptest` candidates.
- **III. Document Structural Decisions as ADRs** — ✅ Two ADRs:
  - **ADR 0013** (already merged on this branch) — the subscription list is authoritative for a node's own topics, not config.
  - **ADR 0014** `docs/decisions/0014-subscription-registry-interface-and-node-integration.md` (authored with this plan) — the `SubscriptionRegistry` trait shape (a single node-keyed `watch`, no point-read) and `MembershipEvent`/`MembershipWatch` mirroring ADR 0007; the `Event::MembershipUpdate` seam + `handle_membership_update`; candidate sets in `NodeState` (coexisting with config `peers`, resolving N-007); the `Node::new` signature change (drops `initial_subscriptions`, adds the registry generically as `Arc<R>`, no fail-fast — a node with no entry stays at empty derived state) and the removal of 002's `subscribed_topics`.
- **IV. Specifications as Ambiguity Detectors** — ✅ The one ambiguity encountered — `joining.md`'s config-vs-chain authority — is surfaced, not silently resolved: recorded in ADR 0013 and proposed as a protocol-doc fix in a separate reviewed PR (#52). No new ambiguity arose during planning.
- **V. Specifications Are Read-Only** — ✅ This plan proposes **no** edits to `pubsub/docs/` or `pubsub/formal_spec/`. The `joining.md`/`README` clarification is a separate human-reviewed PR (#52), not part of this feature's code work. `event-loop-and-registry-contract.md` (an agent-editable workstream doc in this crate) needs only the already-flagged seam-variant rename note when this lands.

**Engineering Standards applied**: logs are operator UX — registry/fold tests assert on `MembershipEvent`s and candidate-set snapshots (and, for a node's own topics, the head `Joined` of its `watch` stream), never log content. Operator-facing strings stay implementation-neutral. **Parse at the edge** — the subscription-list file is parsed in `from_file` at the construction boundary; `apply`, `NodeState`, and the trait take already-decoded values; on-chain decode types are deferred and will be module-internal (spec FR-003). **Forward-compatible interfaces** — `#[non_exhaustive]` `MembershipEvent` + `SubscriptionRegistryError`; the registry consumed generically as `Arc<R>` at construction (an `async fn`/RPITIT trait is not `dyn`-compatible); async (RPITIT) trait methods (a real chain reader will need them); all justified by the named 012 consumer, not speculative. **Reproducible tests** — no wall-clock dependence; the registry is deterministic.

## Project Structure

### Documentation (this feature)

```text
specs/008-node-registry/
├── spec.md              # /speckit-specify output (+ clarifications)
├── plan.md              # This file
├── research.md          # Phase 0: consolidated design decisions
├── data-model.md        # Phase 1: SubscriptionRegistry / MembershipEvent / candidate-set model
├── quickstart.md        # Phase 1: drive the registry + a multi-node in-memory network
├── contracts/
│   └── subscription-registry.md  # Phase 1: trait surface + node public-surface delta
├── checklists/
│   └── requirements.md  # Spec quality checklist (complete; 0 open markers)
└── tasks.md             # /speckit-tasks output (NOT created by /speckit-plan)
```

ADRs (live outside the feature dir):

```text
docs/decisions/
├── 0013-subscription-list-is-authoritative-for-node-interests.md   # merged on this branch
└── 0014-subscription-registry-interface-and-node-integration.md    # authored with this plan
```

### Source Code (repository root)

```text
src/
├── subscription_registry/   # NEW — the registry module (independent of the node loop):
│   ├── mod.rs               #   pub trait SubscriptionRegistry { watch }  (read, node-facing; single node-keyed stream)
│   │                        #   pub trait SubscriptionRegistryControl: SubscriptionRegistry { set_topics, unregister }  (write)
│   │                        #   pub enum MembershipEvent { Joined, TopicsChanged, Left }   (#[non_exhaustive])
│   │                        #   pub struct MembershipWatch  (single-consumer; wraps unbounded rx; not Clone)
│   │                        #   pub enum SubscriptionRegistryError  (#[non_exhaustive])
│   └── in_memory.rs         #   pub struct InMemorySubscriptionRegistry  (state + subscriber channels; private internals)
│                            #     ::new() / ::from_file(path)  + the TOML subscription-list entry type (module-internal)
│                            #   #[cfg(test)] mod tests — write/watch (node-keyed cold-start, scoping, idempotency)
├── state.rs                 # EXTENDED — NodeState gains `candidates: HashMap<TopicId, HashSet<PeerId>>`;
│                            #   new private handler `handle_membership_update(&mut NodeState, MembershipEvent) -> Vec<Effect>`
│                            #   (self-excluded fold); `apply` gains one dispatch line; candidate snapshot accessor
│                            #   #[cfg(test)] mod tests — scripted MembershipUpdate fold + self-exclusion (SC-003)
├── event.rs                 # EXTENDED — Event gains `MembershipUpdate(MembershipEvent)` (still #[non_exhaustive])
├── node.rs                  # EXTENDED — Node::new takes the registry generically (Arc<R>, R: SubscriptionRegistry),
│                            #   drops initial_subscriptions; seeds NodeState with an empty subscription set, then
│                            #   spawns a node-owned reader producer calling watch(self_id) (network-symmetric);
│                            #   topics + candidates converge as the cold-start burst drains (no startup point-read);
│                            #   new getter `candidates(&self, &TopicId) -> Vec<PeerId>`
├── config.rs                # CHANGED — remove `subscribed_topics`; NodeConfig keeps node identity + bootstrap [[peers]]
├── error.rs                 # UNCHANGED — no new variant; construction never fails on a missing entry (FR-018 relaxed)
├── main.rs                  # CHANGED — construct InMemorySubscriptionRegistry (from_file), pass to Node::new
├── lib.rs                   # CHANGED — `mod subscription_registry;` + pub use of the new public items
└── (crypto/ message.rs network.rs peer.rs received.rs topic.rs — UNCHANGED)

tests/
├── (existing 002/003/004 integration suites — updated only where Node::new's signature change forces it)
└── subscription_registry_*.rs / candidate_set_*.rs  # NEW — registry behaviors, source-of-truth invariant, multi-node network
```

**Structure Decision**: the registry is its own module `src/subscription_registry/` (a directory: `mod.rs` for the published trait + event + watch + error, `in_memory.rs` for the impl and the module-internal TOML entry type), leaving room for an `on_chain.rs` at 012 without reshaping callers (spec FR-003 anti-corruption boundary). The candidate set lives in the existing crate-internal `NodeState` (it is folded by a transition, so it is state — per N-007 it enters `NodeState` now that 008 consumes peer data), kept **distinct** from the `Node.peers` bootstrap shell field (spec FR-017). The node consumes the registry only through `Event::MembershipUpdate` fed by the node-owned `watch(self_id)` reader (contract §3 ownership; node never imports the impl beyond construction wiring).

## Design Notes (decision-record pointers)

Consolidated in [research.md](./research.md); structural rationale in ADR 0013 / ADR 0014.

1. **Source of truth = subscription list, not config** — node learns its topics from the head `Joined` of its `watch(self_id)` stream; `subscribed_topics` removed; absent entry → empty derived state (no fail-fast), converges from the stream. (ADR 0013; spec FR-018, SC-007)
2. **Trait + push watch mirror the Network actor-handle** — `watch(node) → MembershipWatch{unbounded rx}`, node-keyed cold-start burst (own entry first, then scoped members) then deltas; not `Clone`; drop ends the subscription. (ADR 0014, extending ADR 0007)
3. **Read model is push, not poll** — the registry emits deltas on write; the in-memory impl fans out to subscriber channels. The protocol's authoritative periodic chain re-read is a 012 concern. (spec FR-006; contract §2)
4. **Seam variant `Event::MembershipUpdate`** — replaces the `RegistryUpdate` placeholder anticipated by ADR 0011/CLAUDE.md; one dispatch line in `apply` + `handle_membership_update`. Needs a heads-up to the 004 author + one-line updates to ADR 0011's comment and the CLAUDE.md SpecKit block when landing. (ADR 0014)
5. **Candidate set in `NodeState`, distinct from bootstrap `peers`** — `HashMap<TopicId, HashSet<PeerId>>`, self-excluded; `Node::candidates` getter; the config `[[peers]]` field is untouched. (ADR 0014; spec FR-015/FR-017; resolves N-007)
6. **Strictly read-only node** — the write API (`set_topics`/`unregister`) lives on a **separate** `SubscriptionRegistryControl` trait, not the node-facing `SubscriptionRegistry`; `Node` holds the registry generically as `Arc<R>` (`R: SubscriptionRegistry`, an `async fn`/RPITIT trait — not `dyn`-compatible), so it has no write methods in scope. Used by the file loader's equivalent and test harnesses, never the node daemon. (spec FR-001/FR-005/FR-018; analyze F3)
7. **Subscription-list file at the edge** — `from_file` parses TOML into the registry's initial membership; `new()` builds an empty registry for programmatic tests. (spec FR-004; parse-at-the-edge)
8. **`subscribe`/`unsubscribe` stay sync** (ADR 0012) — unchanged by this feature; the node's topic set is fixed at `watch(self_id)` time (spec Clarifications), so runtime own-topic changes are out of scope (deferred to 012).

## Complexity Tracking

No constitution violations; table omitted.
