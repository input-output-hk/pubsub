# Quickstart — publishing links & the unified link model (015)

## Relay-only (unchanged behaviour)

The defaults are behaviour-preserving; the only visible change is the degree flag's name:

```sh
pubsub-node \
  --self-id node-a --config node-a.toml \
  --subscription-list subs.toml --topic-registry topics.toml \
  --connection-strategy hash-gated --acceptance-strategy hash-gated-bounded \
  --relay-degree 8 --genesis 7
```

(`--relay-degree` is the former `--target-degree` — same semantics: the fixed target relay connection degree `RF`.)

## A publishing node

A node that may end up with no relay downstream (nobody hash-selects it as their upstream) forms **publishing links** so its published messages still enter the overlay:

```sh
pubsub-node \
  --self-id publisher-p --config p.toml \
  --subscription-list subs.toml --topic-registry topics.toml \
  --connection-strategy hash-gated --acceptance-strategy hash-gated-bounded \
  --relay-degree 8 \
  --publish-strategy hash-gated --publish-degree 3 \
  --genesis 7
```

On the heartbeat dial tick the node first runs the relay dial diff, then the publish pass: for each subscribed topic it checks the **M3 trigger** — would any candidate select this node as an upstream under the current epoch nonce? Only if **no** expected relay downstream exists does it hash-select `≈ --publish-degree` targets (an independent draw: the publish predicate uses its own hash domain) and dial them with publish-intent requests.

Acceptors admit publish-intent requests through their **publish acceptance strategy** (default `accept-from-all`; give experiment nodes the compound baseline):

```sh
  --publish-acceptance-strategy hash-gated-bounded --publish-degree 3 --cap-buffer 3
```

The publish accept cap is `⌈publish_degree + c·√publish_degree⌉`, counted only against inbound publishing links — relay capacity is untouched.

## What flows where

- A message the node **publishes** (`Node::publish`) goes to every relay downstream **and** every active outbound publishing link.
- A message the node **relays** goes to relay downstream only — publishing links never carry it.
- Inbound, a publishing link admits **only the link peer's own published messages**; anything else is dropped (`relay_over_publish_link`).

## Observing links

```rust
node.links()                    // Vec<(PeerId, TopicId, LinkRole, LinkDirection, LinkState)>
node.upstream_connections()    // Relay/Out view (former upstream)
node.downstream_connections()  // Relay/In view (former downstream)
```

Duplicate delivery (a subscriber reachable over both a publish link and the relay flood) is suppressed by the content-hash dedup — `received_messages()` records one copy.
