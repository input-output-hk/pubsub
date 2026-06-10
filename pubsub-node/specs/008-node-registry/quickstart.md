# Quickstart: Subscription Registry (008)

**Date**: 2026-06-10 | **Plan**: [plan.md](./plan.md)

Three illustrative slices, mirroring the test strategy. Code is indicative (final names/signatures per the contract).

## 1. Exercise the registry alone (no node loop)

```rust
use pubsub_node::{InMemorySubscriptionRegistry, SubscriptionRegistry, SubscriptionEvent};
use std::collections::BTreeSet;

let reg = InMemorySubscriptionRegistry::new();
reg.set_interest(peer("node-a"), topics(["weather"])).await.unwrap();
reg.set_interest(peer("node-b"), topics(["weather", "sports"])).await.unwrap();

// Cold start: a new subscription replays current members of the watched topics.
let mut watch = reg.subscribe(topics(["weather"])).await.unwrap();
// drain the burst: Joined{node-a, {weather}}, Joined{node-b, {weather}} (order: write-application)

// Live deltas:
reg.set_interest(peer("node-b"), topics(["sports"])).await.unwrap(); // → TopicsChanged{node-b, removed:{weather}}
reg.unregister(peer("node-a")).await.unwrap();                       // → Left{node-a}
assert!(matches!(watch.recv().await, Some(SubscriptionEvent::TopicsChanged { .. })));

// Self-lookup (the node's own authoritative interests):
assert_eq!(reg.interests_of(peer("node-b")).await.unwrap(), Some(topics(["sports"])));
assert_eq!(reg.interests_of(peer("ghost")).await.unwrap(), None);
```

## 2. Pure fold into the candidate set (synchronous, no async)

```rust
// state.rs unit test — scripted Vec<Event> through `apply`, asserting candidate sets.
let mut st = NodeState::new(peer("S"), /*subscriptions*/ set!{"T1"}, verifier());

apply(&mut st, Event::SubscriptionUpdate(joined("A", ["T1"])));
apply(&mut st, Event::SubscriptionUpdate(joined("S", ["T1"]))); // self — ignored
apply(&mut st, Event::SubscriptionUpdate(topics_changed("A", added=["T2"], removed=[])));
apply(&mut st, Event::SubscriptionUpdate(left("B")));

assert_eq!(st.candidates_snapshot(&topic("T1")), vec![peer("A")]);     // S excluded
assert_eq!(st.candidates_snapshot(&topic("T2")), vec![peer("A")]);
// every apply returned Vec::<Effect>::new()  (Effect uninhabited)
```

## 3. A self-discovering in-memory network (shared registry)

```rust
// Seed the mocked subscription list, then bring up nodes sharing ONE registry Arc.
let registry = Arc::new(InMemorySubscriptionRegistry::from_file("subscription-list.toml")?);
//   node-a → {T1}, node-b → {T1, T2}, node-c → {T2}
let net = Arc::new(InMemoryNetwork::new());

let a = Node::new(peer("node-a"), cfg(), net.clone(), verifier(), registry.clone()).await?;
let b = Node::new(peer("node-b"), cfg(), net.clone(), verifier(), registry.clone()).await?;
let c = Node::new(peer("node-c"), cfg(), net.clone(), verifier(), registry.clone()).await?;

// Each node sourced its interests from its own subscription-list entry (not config),
// subscribed, and folded the others into its candidate set (self-excluded, topic-scoped).
await_steady(|| a.candidates(&topic("T1")) == vec![peer("node-b")]);
await_steady(|| b.candidates(&topic("T2")) == vec![peer("node-c")]);
assert!(c.candidates(&topic("T1")).is_empty()); // c watches only T2

// Source-of-truth invariant: a node configured as node-a but whose entry says {T1}
// acts on {T1}, regardless of any config value (SC-007).

// Fail-fast: constructing a node whose id has no entry errors out.
assert!(Node::new(peer("ghost"), cfg(), net, verifier(), registry).await.is_err());
```

`cfg()` is a `NodeConfig` carrying identity + bootstrap `[[peers]]` only — **no** `subscribed_topics`. `Node::peers()` still returns the bootstrap list, distinct from `candidates`.

## Notes

- The `subscription-list.toml` file is read only by `InMemorySubscriptionRegistry::from_file` (parse-at-the-edge); the node never reads it.
- Tests assert on `SubscriptionEvent`s, candidate-set snapshots, and `interests_of` — never on log content (constitution).
- Multi-process networks: each process loads the same file into its own registry instance (identical membership); runtime file re-read is deferred to 012.
