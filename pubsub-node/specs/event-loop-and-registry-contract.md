# Event-loop refactor + mock topic registry — shared contract for two parallel features

**Status**: workstream-level design reference (tracked, sibling to `ROADMAP.md` /
`IMPLEMENTATION_NOTES.md`), not a spec. Authored 2026-06-08 as pre-spec input for two features
developed in parallel on separate branches. Each feature still gets its own
`/speckit-specify` → `/speckit-plan` → `/speckit-tasks` → `/speckit-implement` cycle; **both
specs MUST cite this document** for the shared seam so the branches agree on it. The structural
decisions it describes (explicit `NodeState`, pure `apply` returning effects, the event-queue
model) are formalized as ADR(s) during Feature A's `/speckit-plan` (Constitution III); this doc
is the cross-feature scoping + contract, not a substitute for those ADRs.

**Audience**: the two maintainers (project author = Feature A; co-developing architect =
Feature B) and the implementation agent on either branch.

---

## 0. The two features at a glance

- **Feature A — Node event-loop refactor** (author's branch). Make node state an explicit
  `NodeState` struct mutated only by a **pure** state-transition function `apply`, driven by a
  single **event queue** with one consumer and many producers. Connection-orientation
  (ROADMAP ~004) rides along but is secondary; the refactor is the core deliverable.
- **Feature B — InMemory node-membership registry** (architect's branch). A `Registry` trait +
  `InMemoryRegistry` with a declarative write API (`set_interest` / `unregister`) and a **push
  read side** that feeds the node via a node-owned subscription-draining reader task.

**The entire interdependence is one seam: the event queue.** Feature B produces a new kind of
event; Feature A defines the queue, the consumer, and the producer-registration mechanism.
Everything else on each branch is independent.

These map loosely onto ROADMAP entries 004 (connection model) and 008 (registry mock), but
they are being taken **next and in parallel**, out of the roadmap's listed order — see §7.

---

## 1. Feature A — Node event-loop refactor

> **Landed as a seam commit** (branch `common-code-004-008`, to merge to `main`
> before 008 branches). The cross-feature seam is in place: the `Event` enum,
> `EventQueue`, `Node::events()`, `Node::spawn_producer`, a single consumer loop,
> and node-owned producers aborted on drop (the network mailbox is the first such
> producer). What feature 004 still adds is the rest of *this* section — the pure
> `NodeState` + `apply` + `Effect` restructure and connections; the seam
> currently keeps the 003 message-handling logic inline in the consumer loop
> rather than in a pure `apply`. 008 builds against the seam.

### 1.1 Pure core: `NodeState` + `apply`

`NodeState` is a plain struct — **no `Arc`, no channel, no async** — so it is constructible and
drivable in a synchronous unit test. The transition function is **pure**: it mutates state and
**returns a list of effects** (outbound commands) rather than performing I/O itself.

```rust
pub struct NodeState {
    self_id: PeerId,
    subscriptions: HashSet<TopicId>,
    received: Vec<ReceivedDelivery>,
    verifier: Arc<dyn Verifier>,
    // registry-derived state and logical connection/peer metadata land here later
}

/// Everything that can change node state. Queue item type.
#[non_exhaustive]
pub enum Event {
    // Feature A owns this variant. Struct variant with pub field types because
    // the network `RoutingFrame` wrapper is crate-internal (pub(crate)).
    MessageReceived { from: PeerId, message: Message },
    RegistryUpdate(RegistryEvent),   // Feature B owns the variant + payload (see §2)
    // future: ConnectionRequested(...), ConnectionClosed(PeerId), ...
}

/// Outbound commands the shell executes. Lets `apply` stay pure.
#[non_exhaustive]
pub enum Effect {
    // populated when fan-out / connections land (~004):
    //   ForwardTo(PeerId, Message), Dial(PeerId), Close(PeerId), ...
}

/// The single state-transition function. Synchronous, no `.await`, no I/O.
pub fn apply(state: &mut NodeState, event: Event) -> Vec<Effect> {
    match event {
        Event::MessageReceived { from, message } => {
            // 002 topic filter + 003 signature verify; on success push to `received`.
            // returns no effects pre-connection; returns ForwardTo(...) once 004 lands.
            Vec::new()
        }
        Event::RegistryUpdate(_update) => {
            // Feature B / merge-second wires this arm.
            Vec::new()
        }
    }
}
```

Pre-connection, `apply` returns an empty effect list (the node only ingests). The
`-> Vec<Effect>` signature is locked from the start so fan-out slots in without reshaping the
core (Constitution VI: forward-compatible interface, justified by the 004 consumer).

### 1.2 Async shell: the `Node`

The shell owns the queue, the single consumer (event loop), and every producer's `JoinHandle`.

```rust
#[derive(Clone)]
pub struct EventQueue(tokio::sync::mpsc::UnboundedSender<Event>);
impl EventQueue {
    pub fn push(&self, event: Event) { let _ = self.0.send(event); }
}

pub struct Node {
    queue: EventQueue,
    state: Arc<Mutex<NodeState>>,     // event loop is the SOLE writer; getters are readers
    event_loop: JoinHandle<()>,
    producers: Vec<JoinHandle<()>>,   // network adapter, registry reader, ... (singletons)
    // connections: HashMap<PeerId, JoinHandle<()>>,  // added by 004 — see §1.3
}

impl Node {
    pub fn events(&self) -> EventQueue { self.queue.clone() }   // ad-hoc / integration-test feed

    /// Register a node-owned long-lived producer. The node holds its JoinHandle and
    /// aborts it on drop. Used identically for the network adapter and the registry reader.
    pub fn spawn_producer<F, Fut>(&mut self, f: F)
    where F: FnOnce(EventQueue) -> Fut, Fut: Future<Output = ()> + Send + 'static {
        self.producers.push(tokio::spawn(f(self.queue.clone())));
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        self.event_loop.abort();
        for h in &self.producers { h.abort(); }
        // for (_, h) in &self.connections { h.abort(); }  // once 004 adds them
    }
}
```

The event loop is the only writer:

```rust
while let Some(event) = event_rx.recv().await {
    let effects = apply(&mut state.lock().unwrap(), event);
    // shell executes effects (forward over connections, dial, ...) once 004 lands
}
```

The network mailbox becomes the **first producer**, spawned through the same `spawn_producer`
path the registry reader will use — network and registry are symmetric, both node-owned, both
aborted on drop:

```rust
node.spawn_producer(move |q| async move {
    while let Some(frame) = net_rx.recv().await {
        q.push(Event::MessageReceived { from: frame.from, message: frame.message });
    }
});
```

### 1.3 Connections (forward-looking, ~004 — not in the contract)

Connections are **dynamic, keyed producers**: each accepted/dialed `Connection` runs a recv
loop that pushes events, owned in a `HashMap<PeerId, JoinHandle<()>>` on the shell so it can be
torn down **individually** when the connection closes (note: dropping a `JoinHandle` does not
abort it — call `.abort()` on removal). Live connection sinks live on the **shell**, not in
`NodeState`; `apply` decides fan-out as `Effect`s and the shell maps peers → live connections
to execute them. This is what keeps `apply` pure.

### 1.4 Open decisions for Feature A's own spec (not part of the contract)

- **Getter mechanism.** Shown here as `Arc<Mutex<NodeState>>` (event loop = sole writer,
  getters = readers — preserves 003's linearizability with minimal change). Alternative: event
  loop owns `NodeState` outright, getters answered via a query channel. Feature A decides.
- **`subscribe()`/`unsubscribe()`**: stay direct synchronous methods returning
  `SubscribeOutcome`, or become `Event`s funnelled through `apply`? Event-sourcing them is
  purer but makes the result eventually-consistent. Feature A decides.

---

## 2. Feature B — InMemory node-membership registry

> **Amendment (2026-06-09).** This section is revised from its original sketch after a design
> pass. Two changes for the co-developing architect to note: **(a) the read side is push, not
> poll** (the original poll/diff loop is superseded below), and **(b) the first cut is
> node-membership only** — the topic-governance writes (create/delete topic, authorized
> publishers) the original draft listed are deferred to their real consumer (golden discovery,
> ~007) and are *not* added to the trait yet (Constitution VI: no forward-compatible surface
> without a live consumer). **The seam in §3 is unchanged** — only the registry's internal read
> mechanism and write surface are refined.

A `Registry` trait + `InMemoryRegistry` impl. The node manages **its own membership** in the
topics it cares about; "who else is in topic T" flows back to the node as a push stream.

**Write side** — declarative, the node calls one endpoint:

- `set_interest(node, topics)` — idempotent upsert of a node's interest set. First call for a
  node emits `Joined`; a later call with a changed set emits `TopicsChanged { added, removed }`
  (the registry diffs against prior state); an unchanged set is a no-op. The registry owns the
  create-vs-update decision — and, on-chain later (012), which transaction to construct.
- `unregister(node)` — withdraw entirely; emits `Left`. Distinct from `set_interest(node, {})`
  ("registered, interested in nothing").

**Read side — push, not poll.** The registry exposes a subscription that mirrors the
`Network::register → handle{mpsc}` actor-handle idiom (ADR 0006/0007), because that is how the
real backend behaves: a chain indexer publishes accepted state transitions as an event stream,
so the registry is event-driven *at the source*. Polling would reinvent the change-detection
the indexer hands us for free, and adds poll-interval latency.

`subscribe(topics)` returns a single-consumer `RegistryWatch`. On open it replays current
members of the watched topics as a `Joined` burst (cold start — so a late subscriber's candidate
list is not blind to pre-existing membership), then streams live deltas.

```rust
/// Payload of `Event::RegistryUpdate`. Carries identity + interest only — no network
/// address (gossip resolves node id → address later) and no deposit (anti-Sybil is
/// enforced at registration/validation, deferred — see IMPLEMENTATION_NOTES N-003).
pub enum RegistryEvent {
    Joined        { node: PeerId, topics: BTreeSet<TopicId> },
    TopicsChanged { node: PeerId, added: BTreeSet<TopicId>, removed: BTreeSet<TopicId> },
    Left          { node: PeerId },
}

#[allow(async_fn_in_trait)] // mirrors the Network trait's v1 allowance
pub trait Registry: Send + Sync + 'static {
    async fn set_interest(&self, node: PeerId, topics: BTreeSet<TopicId>) -> Result<(), RegistryError>;
    async fn unregister(&self, node: PeerId) -> Result<(), RegistryError>;
    async fn subscribe(&self, topics: BTreeSet<TopicId>) -> Result<RegistryWatch, RegistryError>;
}
```

The node-owned reader drains the watch and forwards each event onto the queue — the seam (§3)
is untouched; only the reader's *source* is a push stream rather than a poll loop:

```rust
node.spawn_producer(move |q| async move {
    let mut watch = registry.subscribe(topics_of_interest).await.expect("subscribe");
    while let Some(ev) = watch.recv().await { q.push(Event::RegistryUpdate(ev)); }
});
```

`apply` folds `Joined` / `TopicsChanged` / `Left` into registry-derived `NodeState` (not a side
structure), filtering the node's own `PeerId` locally so it never sees itself.

**Coupling with local subscriptions (002).** A node's local subscription set (`subscribe` /
`unsubscribe`, the receive-path topic filter) becomes the single source of truth that *drives*
`set_interest`: subscribing to a topic both accepts its messages and announces interest to the
registry. This makes 002's `subscribe`/`unsubscribe` registry-aware — `async` + fallible — which
is a deliberate edit to 002's surface that Feature B's spec calls out. Whether that announce is a
direct call or an `Effect` depends on §1.4's `subscribe`-as-method-vs-event decision (Feature A).

---

## 3. The shared contract (the seam — keep it minimal)

Exactly three items cross the branch boundary:

1. **`EventQueue`** — the cloneable push handle (newtype over an unbounded mpsc sender, with
   `push`). Defined by Feature A.
2. **`Node::spawn_producer(|q: EventQueue| async { … })`** — registers a node-owned producer.
   Defined by Feature A; called by Feature B to attach its reader so the node owns the handle.
3. **`Event::RegistryUpdate(RegistryEvent)`** — a variant on `Event`. The variant and its
   `RegistryEvent` payload are defined by Feature B; the `apply` arm that consumes it is also
   Feature B's.

### Ownership split

| Item | Owner |
|---|---|
| `NodeState`, `Event` enum (+ `MessageReceived`), `Effect`, `apply` skeleton + `MessageReceived` arm | Feature A |
| `EventQueue`, the event loop, `spawn_producer`, `events()`, producer/`JoinHandle` ownership + drop-abort | Feature A |
| Network adapter producer | Feature A |
| `Registry` trait, `InMemoryRegistry`, write API (`set_interest`/`unregister`), `subscribe` + `RegistryWatch`, `RegistryEvent` payload | Feature B |
| `Event::RegistryUpdate` variant + its `apply` arm + the reader producer | Feature B |

Feature A's branch never imports `Registry`. Feature B's branch needs only the three contract
items above.

---

## 4. Interdependence, ordering, merge

- **Refactor lands first.** Feature A merges to `main`; Feature B branches off it. Then
  `Event`, `EventQueue`, `spawn_producer`, and `apply` already exist for Feature B to extend —
  no stubs. Feature B adds the `RegistryUpdate` variant, its `apply` arm, and the reader.
- The `apply` match over a non-trivial `Event` set means **whoever merges second writes the
  `RegistryUpdate` arm** — the compiler's exhaustiveness check enforces that it gets wired
  (keep a real arm, not a catch-all, so the wiring can't be silently skipped).
- If both must branch from today's `main` simultaneously, the alternative is a tiny **seam
  commit** to `main` first landing just `EventQueue` + `Event` (with the two variants) +
  `spawn_producer` signatures. More moving parts; prefer refactor-first.

---

## 5. Testing approach

- **Primary — pure state machine.** Construct a `NodeState`, feed a scripted `Vec<Event>`, and
  after each `apply` assert on **both** the resulting state and the returned `Vec<Effect>`.
  Synchronous, deterministic, "assert after each event." No async, no channel, no tasks.
- **Secondary — queue-level integration.** Build a `Node`, get `node.events()`, push events,
  await, and snapshot the getters (the 003 `await_delivery` polling pattern). Exercises the
  plumbing (loop, producers, ownership).
- Per the constitution: tests assert on state/effects/snapshots, never on log content.

---

## 6. Forward-looking / deferred (do not block the two features)

- Pure `apply` + `Effect` return: **decided** (this doc). Effect variants populate with 004.
- Connections as dynamic keyed producers + shell-executed effects: **~004**, Feature A's later
  scope. The `Event`/`EventQueue`/producer/effect shape already accommodates it.
- Getter mechanism; `subscribe`-as-method-vs-event: open, Feature A's spec (§1.4).
- Registry read model: **decided — push/subscribe** (§2, amended 2026-06-09). Cold-start
  snapshot semantics, backpressure/lag handling (bounded vs the v1 unbounded channel), and
  exact `RegistryWatch` shape: Feature B's spec.

---

## 7. ROADMAP numbering

The feature numbers in `specs/ROADMAP.md` are **identifiers, not a strict implementation
order**. These two features are taken next and in parallel even though they sit at ~004 and
~008 in the list, with several intervening entries not yet built. ROADMAP carries a note to
this effect, and its 004 / 008 entries point at this document.
