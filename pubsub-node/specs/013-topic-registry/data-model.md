# Data Model: Topic Registry (013)

**Date**: 2026-06-11 | **Plan**: [plan.md](./plan.md) | **Research**: [research.md](./research.md)

> **Superseded by 014 (2026-06-18):** `TopicRegistry::watch()` no longer streams a cold-start `Registered` **burst**; it now returns a `(TopicSnapshot, TopicRegistryWatch)` pair (current state up front, then live deltas). The "cold-start burst" wording throughout this doc reflects 013's original as-built. See ADR 0020 (Amendment 2026-06-18) and `specs/014-registry-consistency/contracts/registry-consistency.md` §A for the current contract.

New **public** types live in module `src/topic_registry/`. New node-side state is **crate-internal** (`pub(crate)`, `src/state.rs`). Existing public types (`TopicId`, `PublicKey`, `PublisherId`, `Event`, `Node`, `NodeError`) are reused; deltas are noted. The shape parallels 008's `src/subscription_registry/` data model.

## Formal-model grounding (`formal_spec/topic_registry/`)

The authoritative Quint `Topic` record (`types.qnt`) is `{ name: str, owners: Set[PublicKey], admins: Set[PublicKey], publishers: Set[PublicKey], replicationFactor: int, retentionPeriod: int, alive: bool, publishedAtEpoch: int }`, with a numeric counter-assigned `TopicID` separate from `name`. The contract's authorization matrix (`topic_registry.qnt`) gates writes by **owner** (deleteTopic; add/remove owner/admin) or **owner-or-admin** (add/remove publisher; set R/T); createTopic is open (sender becomes sole owner); deleteTopic soft-deletes (tombstone, id retained forever). All identities are `PublicKey`.

The 013 mock is the **node-facing projection** of this record, not the full contract:

| Quint `Topic` field | 013 mock |
|---|---|
| `publishers: Set[PublicKey]` (empty ⇒ open) | **carried** — the only node-consumed field; `BTreeSet<PublicKey>` in `TopicRegistryEvent`/`registered_topics` |
| `owners`, `admins` | **deferred to 012** — no node consumer; the mock write surface (`set_topic`/`remove_topic`) is permissionless, not owner/admin-gated |
| `replicationFactor`, `retentionPeriod`, `publishedAtEpoch` | **deferred to 012** — no node consumer |
| `alive` (soft-delete) | **not modelled** — `remove_topic` is a hard delete; the id-reassignment-prevention concern has no analog in the in-memory mock |
| numeric `TopicID` + `name: str` | **collapsed** to the crate's string `TopicId` (the 012 reader maps on-chain numeric id → name → `TopicId`) |

**Identity**: authorized publishers are `PublicKey`s — the same identity space the subscription list keys on (node pubkey) in the protocol. The current mock's `PeerId` (string) vs `PublicKey` (bytes) split is a pre-011 artifact; the unification at 011 is recorded as IMPLEMENTATION_NOTES **N-009** (Principle IV).

## TopicRegistry (new, public trait — `src/topic_registry/mod.rs`)

The **read-only, node-facing** interface / anti-corruption boundary over the topic-registry source. `Send + Sync + 'static`. `Node` consumes this trait **generically** (`Node::new<…, T: TopicRegistry>(…, Arc<T>)`, not `Arc<dyn>` — an `async fn`/RPITIT trait isn't `dyn`-compatible; same as `Network`/`SubscriptionRegistry`), so it has no write methods in scope. The 012 chain reader implements exactly this trait. It has a **single method**, and unlike 008's `SubscriptionRegistry::watch(node)` it is **global** (no scoping argument).

| Method | Signature | Semantics |
|---|---|---|
| `watch` | `fn watch(&self) -> impl Future<Output = Result<TopicRegistryWatch, TopicRegistryError>> + Send` | Open the **global** topic-registry stream. RPITIT with an explicit `Send` bound (the node-owned reader awaits it in a spawned task). On open it replays a cold-start burst — one `Registered { topic, publishers }` per currently-registered topic — then streams live deltas (`Registered`/`PublishersChanged`/`Removed`). The burst + live deltas are one gap-free, duplicate-free sequence (snapshot capture + subscriber registration atomic under the lock). |

## TopicRegistryControl (new, public trait — write surface; `: TopicRegistry`)

The **operator/test write surface**, a separate trait extending the read trait. The node never depends on it; `InMemoryTopicRegistry` implements it, and the file loader's equivalent + test/operator-sim code hold the concrete `Arc<InMemoryTopicRegistry>`. Keeping these off `TopicRegistry` leaves the node-facing domain interface free of write/test signatures and reflects that the real chain reader (012) has no write method (on-chain governance writes are transactions).

| Method | Signature | Semantics |
|---|---|---|
| `set_topic` | `async fn set_topic(&self, topic: TopicId, publishers: BTreeSet<PublicKey>) -> Result<(), TopicRegistryError>` | Declarative idempotent upsert. First call → observers see `Registered`; changed publisher set → one `PublishersChanged { added, removed }` (registry-computed diff); unchanged → no-op. An empty `publishers` set registers the topic **open**. **Operator/test/loader only — never the node daemon.** |
| `remove_topic` | `async fn remove_topic(&self, topic: TopicId) -> Result<(), TopicRegistryError>` | Remove the topic entirely → observers see `Removed`. Distinct from `set_topic(topic, {})` (empty-publishers registration retained as *open*). Hard delete (the on-chain `alive` soft-delete is deferred to 012). |

## TopicRegistryEvent (new, public — `#[non_exhaustive]`)

One topic-registry delta. **Topic id + authorized publisher keys only** — no `replicationFactor`, `retentionPeriod`, owners, or admins (on-chain governance fields the node does not consume).

| Variant | Fields | Meaning |
|---|---|---|
| `Registered` | `{ topic: TopicId, publishers: BTreeSet<PublicKey> }` | `topic` is a legitimate topic; `publishers` are its authorized keys (**empty ⇒ open** — any publisher). Emitted during cold-start replay and for live registrations. |
| `PublishersChanged` | `{ topic: TopicId, added: BTreeSet<PublicKey>, removed: BTreeSet<PublicKey> }` | `topic`'s authorized-publisher set changed by the given diff. |
| `Removed` | `{ topic: TopicId }` | `topic` is no longer a registered topic; drop it from the registered-topics projection. |

`#[non_exhaustive]` reserves room for a future warmth/lag signal (010/012 consumers) without a breaking change.

## TopicRegistryWatch (new, public)

Single-consumer subscription handle; mirrors `MembershipWatch`/`NetworkHandle` (ADR 0007).

| Field | Type | Notes |
|---|---|---|
| `rx` | `tokio::sync::mpsc::UnboundedReceiver<TopicRegistryEvent>` | private; owns the receive half. |

- **Not `Clone`.** Dropping it ends the subscription cleanly (no effect on registry state or other watches).
- Exposes `recv().await -> Option<TopicRegistryEvent>` (the reader producer loops on it); a `#[cfg(test)] try_next` non-blocking drain mirrors 008.
- **Ordering invariant**: events arrive in write-application order; the cold-start burst and live deltas form one gap-free, duplicate-free sequence.

## TopicRegistryError (new, public — `#[non_exhaustive]`)

Typed error for the fallible methods; `std::error::Error` + `Debug` + `Display` (crate convention; `thiserror`). Minimal now (in-memory methods do not fail under normal operation); grows with the on-chain backend (012). File-load failures surface through `ConfigError`, not this enum. A minimal `Backend(String)` variant mirrors `SubscriptionRegistryError`.

## InMemoryTopicRegistry (new, public struct; private internals)

The v1 concrete impl. Shareable across `Node`s via `Arc` (the multi-node topology).

| Internal (private) | Type | Notes |
|---|---|---|
| topics | `Mutex<HashMap<TopicId, BTreeSet<PublicKey>>>` | topic → authorized publishers; the truth. |
| subscribers | `Mutex<Vec<UnboundedSender<TopicRegistryEvent>>>` | live watches; closed senders pruned on send. (No per-watcher topic filter — the watch is global, unlike 008's per-subscriber filter.) |

(As in 008, both maps may live behind one `Mutex<Inner>` rather than two — a tactical choice for the impl.)

- `::new() -> Self` — empty registry (programmatic tests).
- `::from_file(path) -> Result<Self, ConfigError>` — parse a TOML topic-registry file into the initial state (parse-at-the-edge). Module-internal entry type:

```toml
# topic-registry.toml — the mocked on-chain topic registry
[[topic]]
id         = "weather"
publishers = ["6b317...", "a91f0..."]   # lowercase-hex public keys; absent or [] = open topic

[[topic]]
id = "chat"                              # no publishers key = open topic
```

  The mock entry type has **only** `id` + optional `publishers` — the registered topics and their authorized publishers, all the node consumes. Strict `deny_unknown_fields` (per 001) applies **uniformly**: governance fields (`owners`/`admins`/`replication_factor`/`retention_period`) are **not** part of the mock format (they are 012's on-chain domain), so a field outside `id`/`publishers` is a load error — no accepted-but-ignored fields (resolves analyze F1 by simplification: the mock file is our own minimal config, not a faithful on-chain dump, so the "ignore governance" clause is dropped rather than reconciled). Duplicate `id` → `ConfigError::DuplicateTopicEntry`. Malformed hex in `publishers` → `ConfigError::InvalidPublisherKey`.
- `set_topic`/`remove_topic` mutate `topics` under the lock, compute the diff against prior state, and fan out the resulting `TopicRegistryEvent` to every subscriber. `watch()` captures, **atomically** under the lock, the current topics as the `Registered` cold-start burst, registers the sender, then returns the `TopicRegistryWatch`.

## NodeState (existing crate-internal — extended)

| Field | Type | Change |
|---|---|---|
| `self_id` | `PeerId` | unchanged |
| `subscriptions` | `HashSet<TopicId>` | **unchanged** — still the membership-derived set written only by `handle_membership_update` (008). NOT made the intersection. |
| `received` | `Vec<ReceivedDelivery>` | unchanged |
| `verifier` | `Arc<dyn Verifier>` | unchanged |
| `candidates` | `HashMap<TopicId, HashSet<PeerId>>` | unchanged (008) |
| `registered_topics` | `HashMap<TopicId, BTreeSet<PublicKey>>` | **new** — the topic-registry projection: registered topic → authorized publishers (empty = open). Written **only** by `handle_topic_registry_update`. |

New methods:
- `subscriptions_snapshot(&self) -> Vec<TopicId>` — the intersection `subscriptions ∩ registered_topics.keys()` (the actual accept-filter), cloned out for the public `Node::subscriptions` getter and tests. (Post-implementation collapse, 2026-06-12: this single snapshot replaced the earlier declared `subscriptions_snapshot` + a separate `effective_subscriptions_snapshot`.)
- (private accept-path helper) the registered? + authorized? checks consult `registered_topics`.

## Event (existing public — extended)

`#[non_exhaustive] pub enum Event` in `src/event.rs` gains:

- `TopicRegistryUpdate(TopicRegistryEvent)` — owned by this feature; its `apply` arm dispatches to `handle_topic_registry_update`. (Sibling to 008's `MembershipUpdate(MembershipEvent)`.)

## Transition: handle_topic_registry_update (new, private — `src/state.rs`)

`apply` gains one dispatch line; the handler is pure (no I/O, no `.await`), returns an empty `Vec<Effect>` (`Effect` uninhabited). It folds **only** `registered_topics` — it does not touch `subscriptions` or `candidates`.

| Input variant | Effect on `registered_topics` | Effects |
|---|---|---|
| `Registered { topic, publishers }` | insert/replace `registered_topics[topic] = publishers` | `[]` |
| `PublishersChanged { topic, added, removed }` | for the entry at `topic`: insert `added`, remove `removed` (create the entry if absent — defensive) | `[]` |
| `Removed { topic }` | remove `registered_topics[topic]` | `[]` |

## Transition: handle_signed_message (existing, private — extended)

The accept path gains two checks; existing checks/causes are retained. New behavior is purely additive (SC-010).

| Step | Condition | On failure |
|---|---|---|
| 1 (existing) | `subscriptions.contains(topic)` | drop, cause `topic_not_subscribed` |
| 2 (**new**) | `registered_topics.contains_key(topic)` | drop, cause `topic_not_registered` |
| 3 (**new**) | topic open (`registered_topics[topic].is_empty()`) **or** `registered_topics[topic].contains(publisher_key)` | drop, cause `publisher_not_authorized` |
| 4 (existing) | signature verifies | drop, cause `invalid_signature` |
| 5 (existing) | — | record the delivery |

`publisher_key` is `signed.plain.publisher_id.as_public_key()`. Steps 1–3 are O(1) lookups and precede the expensive step 4 (FR-015).

## Node (existing public — surface delta)

| Item | Change |
|---|---|
| `Node::new` | **signature change**: adds a generic `topic_registry: Arc<T>` where `T: TopicRegistry` (third registry param after `network: Arc<N>` and `subscription_registry: Arc<R>`, the 008 param renamed from `registry` for symmetry). Spawns a node-owned reader that calls `topic_registry.watch()` and pushes one `Event::TopicRegistryUpdate` per delta; `registered_topics` converges as the burst drains. No fail-fast — an empty registry simply yields no registered topics (and thus no effective subscriptions until topics register). |
| `Node::subscriptions` | **changed semantics** (single getter): `fn subscriptions(&self) -> Vec<TopicId>` now returns the **effective accept-filter** `subscriptions ∩ registered_topics` (declared ∩ registered). The declared set + registered-topics projection stay internal-only. Supersedes 008's declared-set semantics; no separate `effective_subscriptions` getter (collapsed post-implementation, 2026-06-12). |
| `Node::candidates` / `Node::peers` | **unchanged** (008 / config bootstrap). |
| `NodeError` | **unchanged** — no new variant; construction never fails on an empty/absent topic registry. |

## Relationships

```text
topic-registry.toml ──from_file──► InMemoryTopicRegistry  (shared via Arc across nodes)
                                       ▲          │ watch() → TopicRegistryWatch
        operator/test set_topic ─────┘          │ (Registered burst, then live deltas — GLOBAL)
                                                   ▼
                           node-owned reader producer (spawn_producer)
                                       │ q.push(Event::TopicRegistryUpdate(ev))
                                       ▼
                                  EventQueue ──► event loop ──► apply(&mut NodeState, ev)
                                                                   │ handle_topic_registry_update
                                                                   ▼
                                              NodeState.registered_topics  (topic → authorized pubs)
                                                                   │
   subscription registry (008) ──► NodeState.subscriptions ────────┤
                                                                   ▼  (intersect at accept time)
                          handle_signed_message: subscribed? registered? authorized? verify? → record
                          Node::subscriptions          ◄── lock-and-clone ── subscriptions ∩ registered_topics
```

## Validation rules

- **Topic validity**: a node's effective topics ⊆ registered topics; an unregistered subscription-list topic never enters the accept-filter and its traffic is dropped (`topic_not_registered`) — SC-003.
- **Dynamic convergence**: registering a previously-unregistered subscribed topic makes it effective; removing a topic makes it ineffective — no restart (SC-004).
- **Authorization**: a non-open topic accepts only publishers in its authorized set; an open topic (empty set) accepts any — SC-005.
- **Idempotency**: `set_topic` with an unchanged publisher set (including unchanged-empty) emits no event (SC-006).
- **Open vs removed**: `set_topic(t, {})` ≠ `remove_topic(t)`.
- **No regression**: registered + subscribed + authorized + valid-signature ⇒ recorded exactly as pre-013 (SC-010).
- **Isolation**: the topic-registry projection never mutates `peers`, `candidates`, or `subscriptions` (SC-009).
- No new chain-integrity validation beyond publisher authorization; equivocation / sequence / deposit remain deferred (N-003).
