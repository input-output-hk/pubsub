# Data Model: Node Event-Loop Refactor (004)

**Date**: 2026-06-09 | **Plan**: [plan.md](./plan.md) | **Research**: [research.md](./research.md)

All types below are **crate-internal** (`pub(crate)`, module `src/state.rs`) unless noted as
existing public surface. Existing public types (`Event`, `EventQueue`, `Message`, `PeerId`,
`TopicId`, `ReceivedDelivery`, `SubscribeOutcome`, `UnsubscribeOutcome`) are unchanged.

## NodeState (new, crate-internal)

The node's full mutable state as one plain struct — no `Arc`, no channel, no async inside;
constructible and drivable in a synchronous unit test.

| Field | Type | Notes |
|---|---|---|
| `self_id` | `PeerId` | The node's own identity; used by handlers for log context. Moves from the loop-captured `self_id_for_task`. |
| `subscriptions` | `HashSet<TopicId>` | The topic-subscription set. Replaces the standalone `Arc<Mutex<HashSet<TopicId>>>` field on `Node`. |
| `received` | `Vec<ReceivedDelivery>` | Accepted deliveries in receive order. Replaces the standalone `Arc<Mutex<Vec<ReceivedDelivery>>>` field on `Node`. |
| `verifier` | `Arc<dyn Verifier>` | Consulted by the message-received handler. Canonical owner (the duplicate `Node.verifier` field is removed). The `Arc` is a shared *immutable* service handle, not shared mutable state — purity is unaffected. |

**Methods** (logic lives here so it is synchronously testable; inline `tracing` per the
ambient-effect carve-out, ADR 0011):

- `subscribe(&mut self, topic: TopicId) -> SubscribeOutcome` — insert; `Added` /
  `AlreadyPresent` semantics identical to the current `Node::subscribe` (ADR 0008).
- `unsubscribe(&mut self, topic: TopicId) -> UnsubscribeOutcome` — remove; `Removed` /
  `NotSubscribed` semantics identical to the current `Node::unsubscribe`.
- Snapshot accessors used by the shell's getters (clone-out of `received` /
  `subscriptions`).

**Invariants**:

- Mutated only (a) by `apply` (event-driven transitions, sole call site: the event loop) and
  (b) by its own `subscribe`/`unsubscribe` methods (control-plane, called by the shell's
  public methods) — both under the shell's single mutex.
- `received` is append-only in this feature; entries appear in event-processing order.

## Event (existing, public — unchanged)

`#[non_exhaustive] pub enum Event` in `src/event.rs`, exactly as on `main`:

- `MessageReceived { from: PeerId, message: Message }` — owned by this feature; its `apply`
  arm dispatches to `handle_message_received`.
- *(reserved, not added here)* `RegistryUpdate(RegistryEvent)` — owned by feature 008 per
  the seam contract §3; 008 adds the variant, payload, and handler.

## Effect (new, crate-internal, uninhabited)

```rust
#[non_exhaustive]
pub(crate) enum Effect {}
```

Outbound commands the shell executes on the transition's behalf. **No variants in this
feature** — the node only ingests. First inhabitants (`ForwardTo`, `Dial`, `Close`) arrive
with 004-connections (ROADMAP); 008's registry arm is state-only and adds none. The type
exists now solely to lock `apply`'s `-> Vec<Effect>` contract (research R2).

## Transition function (new, crate-internal)

```rust
pub(crate) fn apply(state: &mut NodeState, event: Event) -> Vec<Effect>
```

Pure w.r.t. state and protocol effects: synchronous, no `.await`, no protocol I/O; inline
`tracing` permitted (ADR 0011). Thin dispatch to named per-variant handlers (research R7):

| Event variant | Handler | State change | Effects | Log (ambient) |
|---|---|---|---|---|
| `MessageReceived { from, message }`, topic ∉ `subscriptions` | `handle_message_received` → `handle_signed_message` | none | `[]` | `message_dropped`, `cause = "topic_not_subscribed"` |
| `MessageReceived { from, message }`, topic ∈ `subscriptions`, signature invalid | `handle_message_received` → `handle_signed_message` | none | `[]` | `message_dropped`, `cause = "invalid_signature"` |
| `MessageReceived { from, message }`, topic ∈ `subscriptions`, signature valid | `handle_message_received` → `handle_signed_message` | push `ReceivedDelivery { from, message }` onto `received` | `[]` | debug-level `recv` |

(`handle_message_received` is the per-event dispatcher — it emits the debug `recv` and
matches on message kind; the signed-message logic lives in `handle_signed_message`. Future
message kinds get sibling handlers behind the same dispatcher.)

Order of checks is preserved from 003: topic filter first (cheap), then signature
verification — off-topic traffic never pays the verification cost.

## Node (existing, public — internals reshaped, surface unchanged)

| Field | Type | Change |
|---|---|---|
| `handle` | `NetworkHandle` | kept (shell-owned I/O) |
| `peers` | `Vec<BasicPeerDescriptor>` | kept (static; not part of `NodeState` — nothing transitions it) |
| `state` | `Arc<Mutex<NodeState>>` | **new** — replaces the separate `received` + `subscriptions` mutex fields and the duplicate `verifier` field |
| `events` | `EventQueue` | kept |
| `event_loop` | `JoinHandle<()>` | kept; body becomes `recv → apply → execute effects (match effect {})` |
| `producers` | `Vec<JoinHandle<()>>` | kept; network producer becomes the named `network_mailbox_loop` |

Public methods — **signatures and observable semantics unchanged** (`new`, `send`, `id`,
`peers`, `events`, `spawn_producer`, `received_messages`, `subscriptions`, `subscribe`,
`unsubscribe`); `subscribe`/`unsubscribe`/getters become thin lock-takers delegating to
`NodeState`. `Drop` unchanged: abort loop + producers.

## Relationships

```text
producers (network_mailbox_loop, …)
        │ push(Event)
        ▼
   EventQueue ──► event loop (sole event-driven writer)
                      │ lock; apply(&mut NodeState, event) -> Vec<Effect>
                      ▼
              Arc<Mutex<NodeState>> ◄── lock-and-clone ── public getters
                      ▲                                   (received_messages, subscriptions)
                      └── lock; NodeState::subscribe/unsubscribe ── public mutators
                                                       (sync, return outcomes)
shell executes returned effects: match effect {}   (vacuous until 004-connections)
```

## Validation rules

Unchanged from 002/003 and relocated, not rewritten: topic membership check against
`subscriptions`; signature verification via `verifier.verify(publisher_key, signed_bytes,
signature)`. No new validation introduced (chain-integrity etc. remain deferred per
IMPLEMENTATION_NOTES N-003).
