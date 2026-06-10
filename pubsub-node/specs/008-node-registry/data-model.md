# Data Model: Subscription Registry (008)

**Date**: 2026-06-10 | **Plan**: [plan.md](./plan.md) | **Research**: [research.md](./research.md)

New **public** types live in module `src/subscription_registry/`. New node-side state is **crate-internal** (`pub(crate)`, `src/state.rs`). Existing public types (`PeerId`, `TopicId`, `Event`, `EventQueue`, `Node`, `NodeConfig`, `NodeError`) are reused; deltas to them are noted.

## SubscriptionRegistry (new, public trait — `src/subscription_registry/mod.rs`)

The **read-only, node-facing** interface / anti-corruption boundary over the subscription-list source. `Send + Sync + 'static`. `Node` consumes this trait **generically** (`Node::new<…, R: SubscriptionRegistry>(…, Arc<R>)`, not `Arc<dyn>` — an `async fn`/RPITIT trait isn't `dyn`-compatible; same as `Network`'s `Arc<N>`), so it has no write methods in scope. The 012 chain reader implements exactly this trait. It has a **single method** — there is no point-read.

| Method | Signature | Semantics |
|---|---|---|
| `watch` | `fn watch(&self, node: PeerId) -> impl Future<Output = Result<MembershipWatch, SubscriptionRegistryError>> + Send` | Open the **node-keyed** stream from which the node derives **all** of its registry state. RPITIT with an explicit `Send` bound (the node-owned reader awaits it in a spawned task — the `Send`-bounded follow-up ADR 0007 flags to `async fn` in traits). The watch is scoped to `node`'s own subscription-list entry; on open it replays a cold-start `Joined` burst — the node's **own** entry first (`Joined { node, own_topics }`, → its subscription set) then the current **members** of those topics (`Joined { member, scoped }`, → candidate sets) — then live deltas. The head `Joined` is the source-of-truth carrier of the node's own topics (ADR 0013), replacing the removed `entry` point-read. |

## SubscriptionRegistryControl (new, public trait — write surface; `: SubscriptionRegistry`)

The **operator/test write surface**, a separate trait extending the read trait. The node never depends on it; `InMemorySubscriptionRegistry` implements it, and test/operator-sim code holds the concrete `Arc<InMemorySubscriptionRegistry>` to drive churn. Keeping these off `SubscriptionRegistry` leaves the node-facing domain interface free of write/test signatures and reflects that the real chain reader (012) has no write method (on-chain writes are transactions).

| Method | Signature | Semantics |
|---|---|---|
| `set_topics` | `async fn set_topics(&self, node: PeerId, topics: BTreeSet<TopicId>) -> Result<(), SubscriptionRegistryError>` | Declarative idempotent upsert. First call → observers see `Joined`; changed set → one `TopicsChanged { added, removed }` (registry-computed diff); unchanged → no-op. **Operator/test/loader only — never the node daemon.** |
| `unregister` | `async fn unregister(&self, node: PeerId) -> Result<(), SubscriptionRegistryError>` | Remove the node entirely → observers of its topics see `Left`. Distinct from `set_topics(node, {})` (empty-topics registration retained). |

## MembershipEvent (new, public — `#[non_exhaustive]`)

One membership delta. **Identity + topics only** — no network address (endpoints are off-chain), no deposit (anti-Sybil deferred).

| Variant | Fields | Meaning |
|---|---|---|
| `Joined` | `{ node: PeerId, topics: BTreeSet<TopicId> }` | `node` is present in `topics` (a watched subset). Emitted during cold-start replay and for live joins. |
| `TopicsChanged` | `{ node: PeerId, added: BTreeSet<TopicId>, removed: BTreeSet<TopicId> }` | `node` changed topics; `added`/`removed` already intersected with the watched set. |
| `Left` | `{ node: PeerId }` | `node` left the registry entirely; drop from every candidate set. |

`#[non_exhaustive]` reserves room for a future `SnapshotComplete` (010 warmth signal) or `Lagged` (bounded-channel backpressure) without a breaking change.

## MembershipWatch (new, public)

Single-consumer subscription handle; mirrors `NetworkHandle` (ADR 0007).

| Field | Type | Notes |
|---|---|---|
| `rx` | `tokio::sync::mpsc::UnboundedReceiver<MembershipEvent>` | private; owns the receive half. |

- **Not `Clone`.** Dropping it ends the subscription cleanly (no effect on registry state or other watches).
- Exposes a `recv().await -> Option<MembershipEvent>` style drain (the reader producer loops on it).
- **Ordering invariant**: events arrive in write-application order; the cold-start burst and subsequent live deltas form one gap-free, duplicate-free sequence (snapshot capture + subscriber registration are atomic).

## SubscriptionRegistryError (new, public — `#[non_exhaustive]`)

Typed error for the fallible methods; `std::error::Error` + `Debug` + `Display` (crate convention, ADR 0005). Minimal now — in-memory methods do not fail under normal operation; grows with the on-chain backend (012, e.g. backend-unavailable). File-load failures surface through the existing config/IO error path, not this enum.

## InMemorySubscriptionRegistry (new, public struct; private internals)

The v1 concrete impl. Shareable across `Node`s via `Arc` (the multi-node topology, as `InMemoryNetwork` is shared).

| Internal (private) | Type | Notes |
|---|---|---|
| membership | `Mutex<HashMap<PeerId, BTreeSet<TopicId>>>` | node → topic set; the truth. |
| subscribers | `Mutex<Vec<(BTreeSet<TopicId>, UnboundedSender<MembershipEvent>)>>` | live watches and their watched-topic filters; closed senders are pruned on send. |

- `::new() -> Self` — empty registry (programmatic tests).
- `::from_file(path) -> Result<Self, ...>` — parse a TOML subscription-list file into the initial membership (parse-at-the-edge). Module-internal entry type:

```toml
# subscription-list.toml — the mocked on-chain subscription list
[[entry]]
node_id = "node-a"
topics  = ["weather", "sports"]
```

  Strict unknown-field rejection (per 001). Duplicate `node_id` is a load error. Any `deposit` field, if present, is ignored (out of scope).
- `set_topics`/`unregister` mutate `membership` under the lock, compute the diff against prior state, and fan out the resulting `MembershipEvent` (scoped + intersected) to each matching subscriber. `watch(node)` captures, **atomically** under the lock, the node's own entry then the current members of its topics as the `Joined` cold-start burst, registers the sender (filtered to the node's topics), then returns the `MembershipWatch`.

## NodeState (existing crate-internal — extended)

| Field | Type | Change |
|---|---|---|
| `self_id` | `PeerId` | unchanged; used for self-exclusion in the fold |
| `subscriptions` | `HashSet<TopicId>` | unchanged mechanism, but now **starts empty and is derived** from the head `Joined { self_id, .. }` of the `watch(self_id)` stream (folded by `handle_membership_update`), not from the removed config `subscribed_topics` |
| `received` | `Vec<ReceivedDelivery>` | unchanged |
| `verifier` | `Arc<dyn Verifier>` | unchanged |
| `candidates` | `HashMap<TopicId, HashSet<PeerId>>` | **new** — per-topic topic-derived candidate set, folded from `MembershipEvent`s, self-excluded |

New method: `candidates_snapshot(&self, topic: &TopicId) -> Vec<PeerId>` (clone-out for the public getter).

## Event (existing public — extended)

`#[non_exhaustive] pub enum Event` in `src/event.rs` gains:

- `MembershipUpdate(MembershipEvent)` — owned by this feature; its `apply` arm dispatches to `handle_membership_update`. (Renamed from the `RegistryUpdate` placeholder ADR 0011/CLAUDE.md anticipate.)

## Transition: handle_membership_update (new, private — `src/state.rs`)

`apply` gains one dispatch line; the handler is pure (no I/O, no `.await`), returns an empty `Vec<Effect>` (`Effect` uninhabited).

The handler **branches on `node == self_id`**: own-id events drive the node's `subscriptions` (its accept-filter); all other events drive `candidates`. This single branch is what lets the one `watch` stream feed both kinds of state, and is also what keeps the node out of its own candidate sets.

| Input variant | If `node == self_id` (own subscription set) | Else (other node → `candidates`) | Effects |
|---|---|---|---|
| `Joined { node, topics }` | replace `subscriptions` with `topics` | for each `t ∈ topics`: insert `node` into `candidates[t]` | `[]` |
| `TopicsChanged { node, added, removed }` | insert `added`, remove `removed` from `subscriptions` (and drop `candidates[removed]` — no longer watched) | insert `node` into each `added` topic; remove from each `removed` topic | `[]` |
| `Left { node }` | clear `subscriptions` and `candidates` | remove `node` from every topic set | `[]` |

Self-exclusion is a consequence of this branch (own-id events never touch `candidates`), applied **locally**, not in the registry — `watch` carries the node's own entry as data, but the fold decides routing (spec FR-014/FR-016).

## Node (existing public — surface delta)

| Item | Change |
|---|---|
| `Node::new` | **signature change**: drops `initial_subscriptions: HashSet<TopicId>`; adds a generic `registry: Arc<R>` where `R: SubscriptionRegistry` (not `Arc<dyn>` — see the trait note above). Seeds `NodeState` with an **empty** subscription set and spawns a node-owned reader that calls `registry.watch(self_id)` and pushes one `Event::MembershipUpdate` per delta; subscriptions + candidates converge as the cold-start burst drains. **No startup point-read and no fail-fast** — a node with no entry constructs cleanly and stays at empty derived state. |
| `Node::candidates` | **new** public getter: `fn candidates(&self, topic: &TopicId) -> Vec<PeerId>` — sync lock-and-clone snapshot of the per-topic candidate set. |
| `Node::peers` | **unchanged** — still returns the config `[[peers]]` bootstrap list; distinct from `candidates`. |
| `NodeError` | **unchanged** — no new variant; construction no longer fails on a missing entry (FR-018 relaxed). |

## Relationships

```text
subscription-list.toml ──from_file──► InMemorySubscriptionRegistry  (shared via Arc across nodes)
                                          ▲          │ watch(self_id) → MembershipWatch
            operator/test set_topics ───┘          │ (own entry first, then scoped members, then live deltas)
                                                      ▼
                              node-owned reader producer (spawn_producer)
                                          │ q.push(Event::MembershipUpdate(ev))
                                          ▼
                                     EventQueue ──► event loop ──► apply(&mut NodeState, ev)
                                                                      │ handle_membership_update
                                                                      ▼  (branch on node == self_id)
                  self → NodeState.subscriptions  ◄────────────────────┤  (source of truth, ADR 0013)
                  others → NodeState.candidates (self-excluded)         │
                          Node::candidates ◄── lock-and-clone ── NodeState.candidates
                          Node::peers      ◄────────────────── Node.peers (config bootstrap; untouched)
```

## Validation rules

- **Source-of-truth**: a node's effective topics equal its subscription-list entry (the head `Joined` of its `watch` stream); config cannot widen them (SC-007). Absent entry ⇒ empty derived state (no fail-fast); the node converges from the stream.
- **Self-exclusion**: the node's own `PeerId` never appears in any candidate set (SC-003) — its own-id events are routed to `subscriptions`, not `candidates`, by the fold.
- **Scoping**: a watch receives only events for the watched node's topics, with `added`/`removed` intersected (SC-005).
- **Idempotency**: `set_topics` with an unchanged set emits no event (SC-004).
- **Empty vs withdrawal**: `set_topics(node, {})` ≠ `unregister(node)`.
- No new message validation; chain-integrity / publisher-authorization remain deferred (IMPLEMENTATION_NOTES N-003).
