# Quickstart: 004-connections

A guided tour of the connection lifecycle, written against the post-feature public
surface. Identities are mock-stage aliases ("a", "b") — readable in, readable out.

## 1. Construct two connectable nodes

```rust
use std::sync::Arc;
use std::str::FromStr;
use pubsub_node::{
    ConnectToAllCandidates, InMemoryNetwork, InMemorySubscriptionRegistry,
    InMemoryTopicRegistry, MockCryptoScheme, Node, NodeConfig, PeerId,
    SubscriptionRegistryControl, TopicId, TopicRegistryControl,
};

let network  = Arc::new(InMemoryNetwork::new());
let registry = Arc::new(InMemorySubscriptionRegistry::new());
let topics   = Arc::new(InMemoryTopicRegistry::new());
let scheme   = MockCryptoScheme::with_seed([0u8; 32]);

// Alias identities: PeerId::from_str("a") and keypair_from_alias("a") agree by
// construction, so the identity/signer coherence check passes.
let kp_a = scheme.keypair_from_alias("a");
let kp_b = scheme.keypair_from_alias("b");
let id_a = PeerId::from_str("a")?;
let id_b = PeerId::from_str("b")?;

// Topic t registered in the topic registry (open topic: empty publisher set), and
// both nodes registered for it in the subscription registry (the source of truth).
let t = TopicId::from_str("t")?;
topics.set_topic(t.clone(), Default::default()).await?;
registry.set_topics(id_a.clone(), [t.clone()].into()).await?;
registry.set_topics(id_b.clone(), [t.clone()].into()).await?;

let node_a = Node::new(
    id_a, NodeConfig::default(), Arc::clone(&network),
    Arc::new(scheme.signer(kp_a.private)),         // the node's signing identity
    Arc::new(scheme.verifier()),
    Arc::clone(&registry),
    Arc::clone(&topics),                           // topic registry (013)
    Arc::new(ConnectToAllCandidates),              // v1 selection policy
).await?;
let node_b = Node::new(
    id_b, NodeConfig::default(), Arc::clone(&network),
    Arc::new(scheme.signer(kp_b.private)),
    Arc::new(scheme.verifier()),
    Arc::clone(&registry),
    Arc::clone(&topics),
    Arc::new(ConnectToAllCandidates),
).await?;
```

With `NodeConfig::default()` the setup delay is unset: **neither node dials on its
own**. They still accept incoming requests, and their registry views converge in the
background.

## 2. Trigger establishment deterministically (the test path)

```rust
use pubsub_node::Event;

// Wait until each node's candidate view knows the other (tests/common helpers),
// then inject the setup event through the public intake — no timers involved.
node_a.events().push(Event::ConnectionSetup);
node_b.events().push(Event::ConnectionSetup);

// Await convergence (tests/common: await_connection-style helper), then observe:
// each node holds the other as an Active upstream AND as a downstream for t.
assert_eq!(node_a.upstream_connections().len(), 1);   // (b, t) Active
assert_eq!(node_a.downstream_connections().len(), 1); // (b, t)
```

The autonomous path is the same flow with configuration instead of injection:

```toml
# node config TOML — opt-in autonomy
connection_setup_delay_ms = 500
```

## 3. Connection-gated delivery

```rust
// b is an Active upstream of a for t — a validly signed message from b is recorded.
node_b.send(node_a.id(), signed_payload_on(&t)).await?;
// ... await_delivery: node_a.received_messages() now contains it.

// An unconnected third node sending the same valid message is dropped:
// message_dropped, cause = not_connected — never recorded.
```

## 4. Misbehavior severs silently

```rust
// One tampered (invalid-signature) message from b over the Active connection:
node_b.send(node_a.id(), tampered_payload_on(&t)).await?;
// → node_a removes upstream (b, t) and logs connection_severed; nothing is sent to b.

// b's subsequent VALID messages on t are now dropped as not_connected:
assert!(node_a.upstream_connections().is_empty());
```

## 5. Graceful shutdown vs. abrupt drop

```rust
node_b.shutdown().await;       // sends Terminated for every held entry, both roles;
                               // node_a's matching entries are removed.
// Plain `drop(node_b)` instead: no notices — node_a keeps stale entries (harmless:
// they admit nothing). If b restarts and re-dials, a re-accepts idempotently.
```

## 6. Synchronous state-machine testing (the bulk of the coverage)

Crate-internal tests drive `apply` directly with a declarative script — no runtime,
no timers (constitution v1.2.0):

```rust
// state.rs tests, sketch: ConnectionScript chains events one step per line.
let script = ConnectionScript::new()
    .member_joined("b", ["t"])        // candidates converge
    .setup()                          // strategy dials b
    .accepted_from("b", "t")          // AwaitingAccept -> Active
    .tampered_payload_from("b", "t")  // misbehavior: severed, Effect::Misbehaved
    .shutdown();                      // clears state, Terminated effects
for event in script { /* apply + assert state/effects per step */ }
```
