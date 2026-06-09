# Quickstart: Node Event-Loop Refactor (004)

**Plan**: [plan.md](./plan.md) | **Data model**: [data-model.md](./data-model.md)

## Verify parity (the acceptance gate)

The whole existing suite is the regression net — it must pass unmodified:

```sh
cargo build && cargo clippy --all-targets && cargo test
```

No integration test under `tests/` is edited by this feature (spec SC-001).

## Exercise the pure core (new, in-crate unit tests)

The pure core lives in `src/state.rs` and is tested in-module — synchronous, no async
runtime, no channels, no tasks (spec SC-002). The pattern (contract doc §5, "primary"):

```rust
// in src/state.rs  #[cfg(test)] mod tests
let verifier: Arc<dyn Verifier> = Arc::new(TestVerifier::accepting());
let mut state = NodeState::new(self_id, [topic_t.clone()].into(), verifier);

// scripted events, assert state (and effects) after each apply
let effects = apply(&mut state, Event::MessageReceived { from: peer_a, message: on_topic_t });
assert!(effects.is_empty());                       // no effects pre-connection — always
assert_eq!(state.received_snapshot().len(), 1);    // accepted: subscribed + valid signature

let effects = apply(&mut state, Event::MessageReceived { from: peer_a, message: on_topic_u });
assert!(effects.is_empty());
assert_eq!(state.received_snapshot().len(), 1);    // dropped: not subscribed — state unchanged
```

Subscription logic is testable the same way, no `Node` required:

```rust
assert_eq!(state.subscribe(topic_u.clone()), SubscribeOutcome::Added);
assert_eq!(state.subscribe(topic_u.clone()), SubscribeOutcome::AlreadyPresent);
assert_eq!(state.unsubscribe(topic_t), UnsubscribeOutcome::Removed);
```

Tests assert on **state and returned effects only** — never on log output (constitution:
logs are operator UX).

## Exercise the plumbing (existing queue-level pattern, unchanged)

Integration tests keep using the seam exactly as on `main` (contract doc §5, "secondary"):

```rust
let node = Node::new(id, config, subs, network, verifier).await?;
node.events().push(Event::MessageReceived { from, message });   // ad-hoc feed
// 003 await_delivery polling pattern, then:
assert_eq!(node.received_messages().len(), 1);                  // sync snapshot getter
```

## Where things live after the refactor

| Concern | Location |
|---|---|
| `NodeState`, `Effect`, `apply`, handlers, sync unit tests | `src/state.rs` (crate-internal) |
| `Node` shell: queue, event loop, producers, public API | `src/node.rs` |
| `Event`, `EventQueue` (public seam, unchanged) | `src/event.rs` |
| Structural rationale | `docs/decisions/0011-…`, `0012-…` |

## Pre-commit sweep (every checkpoint)

```sh
cargo fmt && cargo build && cargo clippy --all-targets && cargo test
```
