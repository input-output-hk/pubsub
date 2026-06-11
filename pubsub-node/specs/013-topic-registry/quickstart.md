# Quickstart: Topic Registry (013)

**Date**: 2026-06-11 | **Plan**: [plan.md](./plan.md)

Three vignettes — the registry alone, the pure fold + accept path, and a multi-node network with both registries. Illustrative (final signatures/names land in code); none assert on log content (constitution).

## 1. Drive the registry in isolation (US1 — no node loop)

```rust
use pubsub_node::{InMemoryTopicRegistry, TopicRegistry, TopicRegistryControl, TopicRegistryEvent};
use std::collections::BTreeSet;

let reg = InMemoryTopicRegistry::new();
reg.set_topic(topic("weather"), pubkeys([&k1])).await?;          // first registration
reg.set_topic(topic("chat"), BTreeSet::new()).await?;            // open topic (no publishers)

// A watcher sees the current state as a cold-start `Registered` burst, then live deltas.
let mut watch = reg.watch().await?;
let burst = drain(&mut watch);                                   // {Registered weather {k1}, Registered chat {}}

reg.set_topic(topic("weather"), pubkeys([&k1, &k2])).await?;     // → PublishersChanged { added: {k2} }
reg.set_topic(topic("weather"), pubkeys([&k1, &k2])).await?;     // identical → NO event (idempotent)
reg.remove_topic(topic("chat")).await?;                          // → Removed { chat }
assert_eq!(drain(&mut watch), vec![
    TopicRegistryEvent::PublishersChanged { topic: topic("weather"), added: pubkeys([&k2]), removed: BTreeSet::new() },
    TopicRegistryEvent::Removed { topic: topic("chat") },
]);
```

From a file (parse-at-the-edge):

```toml
# topic-registry.toml
[[topic]]
id         = "weather"
publishers = ["6b3170...", "a91f02..."]   # lowercase-hex public keys

[[topic]]
id = "chat"                                # no publishers → open
```

```rust
let reg = InMemoryTopicRegistry::from_file(Path::new("tests/fixtures/topic-registry.toml"))?;
// duplicate `id` → ConfigError::DuplicateTopicEntry; bad hex → ConfigError::InvalidPublisherKey
```

## 2. Pure fold + accept path (US2/US3 — synchronous, no async runtime)

```rust
// NodeState for self-id S; topics arrive on TWO streams, folded by `apply`.
let mut state = node_state(peer("S"), verifier());

// Topic registry: only `weather` is registered (open); `ghosttopic` is not.
apply(&mut state, Event::TopicRegistryUpdate(TopicRegistryEvent::Registered {
    topic: topic("weather"), publishers: BTreeSet::new(),
}));
// Subscription list: S declares {weather, ghosttopic}.
apply(&mut state, Event::MembershipUpdate(MembershipEvent::Joined {
    node: peer("S"), topics: topics(["weather", "ghosttopic"]),
}));

// Effective = declared ∩ registered = {weather}. `ghosttopic` is ignored (not registered).
assert_eq!(state.effective_subscriptions_sorted(), vec![topic("weather")]);

// US3 authorization: open topic accepts any publisher; a non-open topic rejects outsiders.
apply(&mut state, Event::TopicRegistryUpdate(TopicRegistryEvent::PublishersChanged {
    topic: topic("weather"), added: pubkeys([&k1]), removed: BTreeSet::new(),   // weather now restricted to {k1}
}));
apply(&mut state, msg_received(peer("relay"), signed_on("weather", &signer_k2))); // k2 not authorized → DROPPED
apply(&mut state, msg_received(peer("relay"), signed_on("weather", &signer_k1))); // k1 authorized + valid → RECORDED
assert_eq!(state.received_snapshot().len(), 1);

// SC-004: registering ghosttopic later makes it effective with no restart.
apply(&mut state, Event::TopicRegistryUpdate(TopicRegistryEvent::Registered {
    topic: topic("ghosttopic"), publishers: BTreeSet::new(),
}));
assert!(state.effective_subscriptions_sorted().contains(&topic("ghosttopic")));

// Every apply returns no effects (Effect uninhabited).
```

## 3. A network of in-memory nodes with both registries (US4)

```rust
// Shared mocked chain: one subscription registry + one topic registry, both via Arc.
let subs = Arc::new(InMemorySubscriptionRegistry::from_file(/* node → topics */)?);
let topics_reg = Arc::new(InMemoryTopicRegistry::from_file(/* topic → publishers */)?);
//   subscription list: node-a → {weather}, node-b → {weather, sports}, node-c → {weather, ghosttopic}
//   topic registry:    weather → {k1},     sports → {} (open)        // ghosttopic NOT registered

let a = Node::new(peer("node-a"), cfg(), net.clone(), verifier(), subs.clone(), topics_reg.clone()).await?;
let b = Node::new(peer("node-b"), cfg(), net.clone(), verifier(), subs.clone(), topics_reg.clone()).await?;
let c = Node::new(peer("node-c"), cfg(), net.clone(), verifier(), subs.clone(), topics_reg.clone()).await?;

// Poll to steady state (both registry bursts drained):
await_effective_subscriptions(&c, &[topic("weather")]).await;   // ghosttopic dropped (not registered) — SC-003
await_effective_subscriptions(&b, &[topic("sports"), topic("weather")]).await;

// A message on `weather` from k1 is accepted by subscribers; from a non-k1 publisher it is dropped — SC-005.
a.send(b.id(), signed_on("weather", &signer_k1)).await?;
await_delivery(&b, /* the k1 message */).await;
a.send(b.id(), signed_on("weather", &signer_k2)).await?;        // k2 unauthorized → b never records it
```

## Test surface checklist

- Registry module (US1): cold-start burst completeness (SC-001), exactly-once delta (SC-002), idempotent no-op (SC-006), open-vs-removed distinction, `from_file` (duplicate id, bad hex). No `Node`.
- Pure core (US2/US3): effective-subscription intersection + topic-validity invariant (SC-003), dynamic register/remove (SC-004), publisher authorization incl. open topics (SC-005), no-regression for the valid path (SC-010), every `apply` → empty `Vec<Effect>`.
- Integration (US4): multi-node convergence over shared `Arc`s with both registries; effective subscriptions per node; accept/drop by publisher; isolation from `peers`/`candidates` (SC-008/SC-009).
- Assertions are on events / `received_messages()` / `effective_subscriptions()` snapshots — never log content.
