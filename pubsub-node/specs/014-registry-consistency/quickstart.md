# Quickstart: Cross-Registry Consistency (014)

**Date**: 2026-06-15 | **Plan**: [plan.md](./plan.md)

Three vignettes — the maintained invariant + strict drop, the atomic cascade + defensive fold, and a cold-start multi-node convergence. Illustrative (final names land in code); none assert on log content (Constitution).

## 1. Strict drop + the maintained invariant (pure, synchronous)

```rust
let mut state = node_state(peer("S"));

// Topic registry warms first (readiness gate): only `weather` registered (open).
apply(&mut state, Event::TopicRegistryUpdate(TopicRegistryEvent::Registered {
    topic: topic("weather"), publishers: BTreeSet::new(),
}));
apply(&mut state, Event::TopicRegistryUpdate(TopicRegistryEvent::SnapshotComplete)); // boundary

// Self subscribes to {weather, ghost}; `ghost` is NOT registered → strict-dropped.
apply(&mut state, Event::MembershipUpdate(MembershipEvent::joined("S", ["weather", "ghost"])));
assert_eq!(sorted(state.subscriptions_snapshot()), vec![topic("weather")]);   // ghost never entered

// No auto-promotion: registering ghost later does NOT add it back.
apply(&mut state, Event::TopicRegistryUpdate(TopicRegistryEvent::Registered {
    topic: topic("ghost"), publishers: BTreeSet::new(),
}));
assert!(!state.subscriptions_snapshot().contains(&topic("ghost")));            // strict drop, no promotion

// A fresh membership event AFTER registration does add it (the supported path).
apply(&mut state, Event::MembershipUpdate(MembershipEvent::topics_changed("S", ["ghost"], [])));
assert!(state.subscriptions_snapshot().contains(&topic("ghost")));

// INV-1 held at every step: subscriptions ⊆ registered_topics.keys().
```

## 2. Atomic cascade + defensive fold (pure, synchronous)

```rust
// weather registered + restricted to {k1}; S subscribed; another node is a weather candidate.
let mut state = registered_and_subscribed("S", "weather", [&k1]);
apply(&mut state, Event::MembershipUpdate(MembershipEvent::joined("B", ["weather"])));
assert_eq!(state.candidates_snapshot(&topic("weather")), vec![peer("B")]);

// Removal cascades atomically: subscription, candidate, and projection all drop in one fold.
apply(&mut state, Event::TopicRegistryUpdate(TopicRegistryEvent::Removed { topic: topic("weather") }));
assert!(state.subscriptions_snapshot().is_empty());
assert!(state.candidates_snapshot(&topic("weather")).is_empty());
// a subsequent message on weather is dropped (not registered / not subscribed).

// Defensive fold: PublishersChanged for an unregistered topic does NOT create it.
apply(&mut state, Event::TopicRegistryUpdate(TopicRegistryEvent::PublishersChanged {
    topic: topic("ghost"), added: pubkeys([&k1]), removed: BTreeSet::new(),
}));
assert!(!state.is_registered(&topic("ghost")));     // no or_default create; dropped + logged

// Candidate gating: B on an unregistered topic is not recorded.
apply(&mut state, Event::MembershipUpdate(MembershipEvent::joined("B", ["ghost"])));
assert!(state.candidates_snapshot(&topic("ghost")).is_empty());
```

## 3. Cold-start multi-node convergence (the readiness gate)

```rust
// Shared mocked chain: one subscription registry + one topic registry, both pre-populated.
let subs = Arc::new(InMemorySubscriptionRegistry::from_file(/* node → topics */)?);
let topics_reg = Arc::new(InMemoryTopicRegistry::from_file(/* topic → publishers */)?);
//   topics:  weather → {k1}, sports → {} (open)         // ghost NOT registered
//   subs:    node-a → {weather}, node-b → {weather, sports}, node-c → {weather, ghost}

// Node::new is non-blocking: an in-node oneshot holds the membership reader until
// the topic reader has enqueued its SnapshotComplete (registered_topics warm), so
// strict drop never fires spuriously on a cold-start race. (Signatures illustrative.)
let a = Node::new(peer("node-a"), cfg(), net.clone(), verifier(), subs.clone(), topics_reg.clone()).await?;
let b = Node::new(peer("node-b"), cfg(), net.clone(), verifier(), subs.clone(), topics_reg.clone()).await?;
let c = Node::new(peer("node-c"), cfg(), net.clone(), verifier(), subs.clone(), topics_reg.clone()).await?;

// Deterministic convergence (no timing flakiness): each node's subscriptions = registered ∩ its entry.
await_subscriptions(&c, &[topic("weather")]).await;            // ghost dropped (strict, never recovered)
await_subscriptions(&b, &[topic("sports"), topic("weather")]).await;

// Steady-state acceptance is byte-identical to 013: a k1 message on weather is accepted; non-k1 dropped.
```

## Test surface checklist

- Pure core: INV-1 + INV-2 after every step (SC-001, `proptest`); strict drop + no auto-promotion (SC-008); candidate gating (SC-008); defensive fold no-create (SC-010); atomic cascade leaves no residue (SC-002/003); 013 accept/drop matrix unchanged (SC-004); every `apply` → empty `Vec<Effect>`.
- `TopicEntry` unit: `is_open` / `is_publisher_authorized` (open + restricted).
- Integration: cold-start multi-node deterministic convergence, no spurious drop (SC-009); two registries stay separate (SC-006).
- Assertions on snapshots / `received_messages()` / events — never log content.
