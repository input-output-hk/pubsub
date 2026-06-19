# Quickstart — Publishing and Fan-out

A walkthrough of the new surface on top of the 004 connection model. Illustrative; not a test (logs shown are operator UX, never asserted).

## Construct a node with a fan-out strategy

`Node::new` gains a final `fanout_strategy` parameter, mirroring the connection `strategy`:

```rust
let node = Node::new(
    self_id,
    config,
    network,
    signer,
    verifier,
    subscription_registry,
    topic_registry,
    Arc::new(ConnectToAllCandidates), // connection strategy (004)
    Arc::new(ForwardToAll),           // fan-out strategy (006)  ← NEW
).await?;
```

## Publish a message

The caller builds and signs the `SignedMessage` (parse-at-the-edge: the node consumes a ready value, mints nothing, consults no clock), then publishes fire-and-forget:

```rust
let plain = PlainMessage {
    topic: weather.clone(),
    publisher_id: publisher_signer.public_key().into(),
    parent_hash: None,
    sequence: 0,
    timestamp: Timestamp::from_millis(now_ms),
    payload: MessagePayload::Ping(42),
};
let signed = SignedMessage { signature: publisher_signer.sign(&plain.signed_bytes()), plain };

node.publish(signed); // returns () immediately
```

If `node` is subscribed to `weather`, the topic is registered, the publisher is authorized, and the signature verifies, the node records the message (`origin = Local`) and forwards it to every downstream peer it holds on `weather`. Otherwise it is dropped — observe via logs (`event=message_dropped cause=…`) and the absence from `received_messages()`.

**Proxy publishing**: `publisher_signer` need not be the node's own key. A node can inject any validly-signed, topic-authorized message — useful for a relay forwarding an external publisher's pre-signed message.

## Observe deliveries and their origin

`received_messages()` now returns deliveries tagged with `Origin`:

```rust
for d in node.received_messages() {
    match d.origin {
        Origin::Local      => { /* this node published it */ }
        Origin::Peer(peer) => { /* `peer` forwarded it to us */ }
    }
}
```

The publisher is always in `d.message` (`publisher_id`), independent of origin.

## Relay across the mesh

A node that receives a message over an Active upstream — once it passes the connection gate, subscription, registration, authorization, and signature checks — records it (`origin = Peer(sender)`) and forwards it to its other downstream peers, **excluding the sender** (split-horizon). In a connected mesh, one publish reaches every member.

## Loop suppression

The node remembers the content hash of every message it accepts. A second copy — whether relayed around a cycle or re-published — is dropped (`cause=duplicate`), not recorded again and not forwarded again. This is what makes relay safe in the cyclic full-mesh that 004 builds.

## Building a partial topology in tests

The full mesh (the default 004 topology) means every member also receives a direct copy, so relay never becomes the *sole* delivery path. To test relay in isolation, script the handshake directly (as `connections.rs` does) to build, e.g., a line `A→B→C` where A and C are not connected:

```rust
// B requests A (B's upstream); C requests B (C's upstream); no A–C edge.
// publish at A → B relays to C → C records via B only.
```

Connection-lifecycle integration suites pass the public `ForwardToAll`; fan-out does not perturb their assertions. (There is no test-only no-op strategy — a `#[cfg(test)]` one would be invisible to integration crates anyway; empty-downstream covers no-fan-out unit assertions.)
