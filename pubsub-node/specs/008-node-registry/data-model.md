# Data Model: Subscription Registry (008)

**Date**: 2026-06-10 | **Plan**: [plan.md](./plan.md) | **Research**: [research.md](./research.md)

New **public** types live in module `src/subscription_registry/`. New node-side state is **crate-internal** (`pub(crate)`, `src/state.rs`). Existing public types (`PeerId`, `TopicId`, `Event`, `EventQueue`, `Node`, `NodeConfig`, `NodeError`) are reused; deltas to them are noted.

## SubscriptionRegistry (new, public trait — `src/subscription_registry/mod.rs`)

The published interface / anti-corruption boundary over the subscription-list source. `Send + Sync + 'static`, async methods (mirrors `Network`, incl. `#[allow(async_fn_in_trait)]` + its tracked `Send`-bound follow-up).

| Method | Signature | Semantics |
|---|---|---|
| `set_interest` | `async fn set_interest(&self, node: PeerId, topics: BTreeSet<TopicId>) -> Result<(), SubscriptionRegistryError>` | Declarative idempotent upsert. First call → observers see `Joined`; changed set → one `TopicsChanged { added, removed }` (registry-computed diff); unchanged → no-op. **Operator/test/loader only — never the node daemon.** |
| `unregister` | `async fn unregister(&self, node: PeerId) -> Result<(), SubscriptionRegistryError>` | Remove the node entirely → observers of its topics see `Left`. Distinct from `set_interest(node, {})` (empty-interest registration retained). |
| `subscribe` | `async fn subscribe(&self, topics: BTreeSet<TopicId>) -> Result<SubscriptionWatch, SubscriptionRegistryError>` | Open a topic-scoped watch: replay current members as a `Joined` cold-start burst, then stream live deltas. Takes no subscriber id (self-filtering is the consumer's job). |
| `interests_of` | `async fn interests_of(&self, node: PeerId) -> Result<Option<BTreeSet<TopicId>>, SubscriptionRegistryError>` | Self-lookup: a node's registered interest set, `None` if not registered. The node calls `interests_of(self_id)` at startup to learn its **own** authoritative interests (source-of-truth enforcement, ADR 0013). |

## SubscriptionEvent (new, public — `#[non_exhaustive]`)

One membership delta. **Identity + interest only** — no network address (endpoints are off-chain), no deposit (anti-Sybil deferred).

| Variant | Fields | Meaning |
|---|---|---|
| `Joined` | `{ node: PeerId, topics: BTreeSet<TopicId> }` | `node` is present in `topics` (a watched subset). Emitted during cold-start replay and for live joins. |
| `TopicsChanged` | `{ node: PeerId, added: BTreeSet<TopicId>, removed: BTreeSet<TopicId> }` | `node` changed interests; `added`/`removed` already intersected with the watched set. |
| `Left` | `{ node: PeerId }` | `node` left the registry entirely; drop from every candidate set. |

`#[non_exhaustive]` reserves room for a future `SnapshotComplete` (010 warmth signal) or `Lagged` (bounded-channel backpressure) without a breaking change.

## SubscriptionWatch (new, public)

Single-consumer subscription handle; mirrors `NetworkHandle` (ADR 0007).

| Field | Type | Notes |
|---|---|---|
| `rx` | `tokio::sync::mpsc::UnboundedReceiver<SubscriptionEvent>` | private; owns the receive half. |

- **Not `Clone`.** Dropping it ends the subscription cleanly (no effect on registry state or other watches).
- Exposes a `recv().await -> Option<SubscriptionEvent>` style drain (the reader producer loops on it).
- **Ordering invariant**: events arrive in write-application order; the cold-start burst and subsequent live deltas form one gap-free, duplicate-free sequence (snapshot capture + subscriber registration are atomic).

## SubscriptionRegistryError (new, public — `#[non_exhaustive]`)

Typed error for the fallible methods; `std::error::Error` + `Debug` + `Display` (crate convention, ADR 0005). Minimal now — in-memory methods do not fail under normal operation; grows with the on-chain backend (012, e.g. backend-unavailable). File-load failures surface through the existing config/IO error path, not this enum.

## InMemorySubscriptionRegistry (new, public struct; private internals)

The v1 concrete impl. Shareable across `Node`s via `Arc` (the multi-node topology, as `InMemoryNetwork` is shared).

| Internal (private) | Type | Notes |
|---|---|---|
| membership | `Mutex<HashMap<PeerId, BTreeSet<TopicId>>>` | node → interest set; the truth. |
| subscribers | `Mutex<Vec<(BTreeSet<TopicId>, UnboundedSender<SubscriptionEvent>)>>` | live watches and their watched-topic filters; closed senders are pruned on send. |

- `::new() -> Self` — empty registry (programmatic tests).
- `::from_file(path) -> Result<Self, ...>` — parse a TOML subscription-list file into the initial membership (parse-at-the-edge). Module-internal entry type:

```toml
# subscription-list.toml — the mocked on-chain subscription list
[[entry]]
node_id = "node-a"
topics  = ["weather", "sports"]
```

  Strict unknown-field rejection (per 001). Duplicate `node_id` is a load error. Any `deposit` field, if present, is ignored (out of scope).
- `set_interest`/`unregister` mutate `membership` under the lock, compute the diff against prior state, and fan out the resulting `SubscriptionEvent` (scoped + intersected) to each matching subscriber. `subscribe` captures the current matching members as the `Joined` burst and registers the sender **atomically** under the lock, then returns the `SubscriptionWatch`.

## NodeState (existing crate-internal — extended)

| Field | Type | Change |
|---|---|---|
| `self_id` | `PeerId` | unchanged; used for self-exclusion in the fold |
| `subscriptions` | `HashSet<TopicId>` | unchanged mechanism, but now **seeded from `interests_of(self_id)`** at construction, not from the removed config `subscribed_topics` |
| `received` | `Vec<ReceivedDelivery>` | unchanged |
| `verifier` | `Arc<dyn Verifier>` | unchanged |
| `candidates` | `HashMap<TopicId, HashSet<PeerId>>` | **new** — per-topic interest-derived candidate set, folded from `SubscriptionEvent`s, self-excluded |

New method: `candidates_snapshot(&self, topic: &TopicId) -> Vec<PeerId>` (clone-out for the public getter).

## Event (existing public — extended)

`#[non_exhaustive] pub enum Event` in `src/event.rs` gains:

- `SubscriptionUpdate(SubscriptionEvent)` — owned by this feature; its `apply` arm dispatches to `handle_subscription_update`. (Renamed from the `RegistryUpdate` placeholder ADR 0011/CLAUDE.md anticipate.)

## Transition: handle_subscription_update (new, private — `src/state.rs`)

`apply` gains one dispatch line; the handler is pure (no I/O, no `.await`), returns an empty `Vec<Effect>` (`Effect` uninhabited).

| Input variant | State change (on `candidates`) | Self-handling | Effects |
|---|---|---|---|
| `Joined { node, topics }` | for each `t ∈ topics`: insert `node` into `candidates[t]` | if `node == self_id`: skip entirely | `[]` |
| `TopicsChanged { node, added, removed }` | insert `node` into each `added` topic; remove from each `removed` topic | if `node == self_id`: skip | `[]` |
| `Left { node }` | remove `node` from every topic set | (no-op if `node == self_id`) | `[]` |

Self-exclusion is applied **here** (locally), not in the registry — `subscribe` never learns who is asking (spec FR-014/FR-016).

## Node (existing public — surface delta)

| Item | Change |
|---|---|
| `Node::new` | **signature change**: drops `initial_subscriptions: HashSet<TopicId>`; adds `registry: Arc<dyn SubscriptionRegistry>`. Startup: `interests = registry.interests_of(self_id).await?` → `None` ⇒ fail fast (`NodeError` registration-not-found); seed `NodeState.subscriptions` from `interests`; `registry.subscribe(interests)` → spawn the reader producer (drains the watch, pushes `Event::SubscriptionUpdate`). |
| `Node::candidates` | **new** public getter: `fn candidates(&self, topic: &TopicId) -> Vec<PeerId>` — sync lock-and-clone snapshot of the per-topic candidate set. |
| `Node::peers` | **unchanged** — still returns the config `[[peers]]` bootstrap list; distinct from `candidates`. |
| `NodeError` | gains a registration-not-found variant (fail-fast, spec FR-018). |

## Relationships

```text
subscription-list.toml ──from_file──► InMemorySubscriptionRegistry  (shared via Arc across nodes)
                                          ▲          │ subscribe(topics) → SubscriptionWatch
            operator/test set_interest ───┘          │ (cold-start Joined burst, then live deltas)
                                                      ▼
                              node-owned reader producer (spawn_producer)
                                          │ q.push(Event::SubscriptionUpdate(ev))
                                          ▼
                                     EventQueue ──► event loop ──► apply(&mut NodeState, ev)
                                                                      │ handle_subscription_update
                                                                      ▼  (self-excluded fold)
                          Node::candidates ◄── lock-and-clone ── NodeState.candidates
                          Node::peers      ◄────────────────── Node.peers (config bootstrap; untouched)
startup: registry.interests_of(self_id) ──► seed NodeState.subscriptions  (source of truth, ADR 0013)
```

## Validation rules

- **Source-of-truth**: a node's effective interests equal its subscription-list entry; config cannot widen them (SC-007). Absent entry at startup ⇒ fail fast.
- **Self-exclusion**: the node's own `PeerId` never appears in any candidate set (SC-003), enforced in the fold.
- **Scoping**: a watch receives only events for its watched topics, with `added`/`removed` intersected (SC-005).
- **Idempotency**: `set_interest` with an unchanged set emits no event (SC-004).
- **Empty vs withdrawal**: `set_interest(node, {})` ≠ `unregister(node)`.
- No new message validation; chain-integrity / publisher-authorization remain deferred (IMPLEMENTATION_NOTES N-003).
