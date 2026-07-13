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

A publisher opens **standing initiation links** (the M3 model's s−1 links — always established, regardless of its relay links) so its published messages enter the overlay:

```sh
pubsub-node \
  --self-id publisher-p --config p.toml \
  --subscription-list subs.toml --topic-registry topics.toml \
  --connection-strategy hash-gated --acceptance-strategy hash-gated-bounded \
  --relay-degree 8 \
  --publish-strategy hash-gated --publish-degree 3 \
  --genesis 7
```

On the heartbeat dial tick the node first runs the relay dial diff, then the publish pass: for each subscribed topic it hash-selects `≈ --publish-degree` initiation targets (an independent draw — the publish predicate uses its own hash domain) and dials them with publish-intent requests. Unconditional: initiation links exist whether or not the node holds relay links (`m3/README.md`).

Acceptors admit publish-intent requests through their **publish acceptance strategy** (default `accept-from-all`; give experiment nodes the compound baseline):

```sh
  --publish-acceptance-strategy hash-gated-bounded --publish-degree 3 --cap-buffer 3
```

The publish accept cap is `⌈publish_degree + c·√publish_degree⌉`, counted only against inbound publishing links — relay capacity is untouched.

## What flows where — the fan-out kind is the model knob

Default `--fanout-strategy forward-to-all` — **the M3 semantics** (`m3/README.md`):

- A message the node **publishes** (`Node::publish`) goes to every relay downstream **and** every active outbound initiation link (a forwarder relays every message it holds, own publications included).
- A message the node **relays** goes to relay downstream only — initiation links never carry it (owner-exclusive).

`--fanout-strategy role-scoped` (a strict-partition experiment variant, prescribed by no published model): a local publish goes over initiation links **only** (the relay downstream receive it via the flood); relayed traffic over relay links only. Caution: with `--publish-strategy none` this makes the node a **mute publisher**.

Either way, inbound initiation links admit **only the link peer's own published messages**; anything else is dropped (`relay_over_publish_link`).

## Observing links

```rust
node.links()                    // Vec<(PeerId, TopicId, LinkRole, LinkDirection, LinkState)>
node.upstream_connections()    // Relay/Out view (former upstream)
node.downstream_connections()  // Relay/In view (former downstream)
```

Duplicate delivery (a subscriber reachable over both a publish link and the relay flood) is suppressed by the content-hash dedup — `received_messages()` records one copy.
