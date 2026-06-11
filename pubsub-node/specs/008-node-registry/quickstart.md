# Quickstart: Subscription Registry (008)

**Date**: 2026-06-10 | **Plan**: [plan.md](./plan.md)

Three illustrative slices, mirroring the test strategy. Code is indicative (final names/signatures per the contract).

## 1. Exercise the registry alone (no node loop)

```rust
use pubsub_node::{InMemorySubscriptionRegistry, SubscriptionRegistry, MembershipEvent};
use std::collections::BTreeSet;

let reg = InMemorySubscriptionRegistry::new();
reg.set_topics(peer("node-a"), topics(["weather"])).await.unwrap();
reg.set_topics(peer("node-b"), topics(["weather", "sports"])).await.unwrap();

// Node-keyed cold start: watch AS node-a. The burst is node-a's OWN entry first
// (its id + topics — the source of truth for its subscription set), then the
// members of node-a's topics scoped to them.
let mut watch = reg.watch(peer("node-a")).await.unwrap();
// drain the burst: Joined{node-a, {weather}} (self), then Joined{node-b, {weather}} (member, scoped)

// Live deltas, scoped to node-a's watched topics ({weather}):
reg.set_topics(peer("node-b"), topics(["sports"])).await.unwrap(); // node-b leaves weather → TopicsChanged{node-b, removed:{weather}}
reg.unregister(peer("node-a")).await.unwrap();                       // → Left{node-a}
assert!(matches!(watch.recv().await, Some(MembershipEvent::TopicsChanged { .. })));

// There is NO point-read: a node learns its own id + topics from the head
// `Joined` of its own watch's cold-start burst (the removed `entry`'s role).
```

## 2. Pure fold into the candidate set (synchronous, no async)

```rust
// state.rs unit test — scripted Vec<Event> through `apply`. The node starts EMPTY
// and derives BOTH its own subscription set and its candidate sets from the stream.
let mut st = NodeState::new(peer("S"), /*subscriptions*/ HashSet::new(), verifier());

apply(&mut st, Event::MembershipUpdate(joined("A", ["T1"])));
apply(&mut st, Event::MembershipUpdate(joined("S", ["T1"]))); // self → sets S's OWN subscriptions (not a candidate)
apply(&mut st, Event::MembershipUpdate(topics_changed("A", added=["T2"], removed=[])));
apply(&mut st, Event::MembershipUpdate(left("B")));

assert_eq!(st.subscriptions, HashSet::from([topic("T1")]));            // own set (crate-internal field), derived from the self `Joined`
assert_eq!(st.candidates_snapshot(&topic("T1")), vec![peer("A")]);     // S excluded (its own events fed subscriptions)
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

// Each node starts EMPTY and derives its state from the head of its own watch
// stream: its OWN entry sets its subscription set (not config), then the members
// of its topics fold into its candidate set (self-excluded, topic-scoped). State
// converges asynchronously, so wait for steady state.
await_steady(|| a.subscriptions() == vec![topic("T1")]);  // derived, not configured
await_steady(|| a.candidates(&topic("T1")) == vec![peer("node-b")]);
await_steady(|| b.candidates(&topic("T2")) == vec![peer("node-c")]);
assert!(c.candidates(&topic("T1")).is_empty()); // c watches only T2

// Source-of-truth invariant: a node configured as node-a but whose entry says {T1}
// acts on {T1}, regardless of any config value (SC-007).

// No fail-fast: a node whose id has no entry constructs cleanly and simply stays
// at empty derived state (the "registered but not yet present" / initializing posture).
let ghost = Node::new(peer("ghost"), cfg(), net, verifier(), registry).await?;
await_steady(|| ghost.subscriptions().is_empty() && ghost.candidates(&topic("T1")).is_empty());
```

`cfg()` is a `NodeConfig` carrying identity + bootstrap `[[peers]]` only — **no** `subscribed_topics`. `Node::peers()` still returns the bootstrap list, distinct from `candidates`.

## Notes

- The `subscription-list.toml` file is read only by `InMemorySubscriptionRegistry::from_file` (parse-at-the-edge); the node never reads it.
- Tests assert on `MembershipEvent`s (including a watch's head `Joined`), subscription/candidate-set snapshots — never on log content (constitution).
- Multi-process networks: each process loads the same file into its own registry instance (identical membership); runtime file re-read is deferred to 012.
