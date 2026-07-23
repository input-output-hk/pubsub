# Quickstart: running the dissemination models (015)

Every model is a per-node flag combination — no `--model` preset; M1, M2, M3,
and M5 are expressible exactly, M4 as an approximation pending a uniform
selection kind (see its section). Shared
setup (registries, config) is unchanged from the 005 quickstart; only the
strategy flags differ. `--genesis` seeds the epoch nonce (same genesis ⇒ same
topology, reproducible experiments).

## M2 — pull baseline (pre-015 behaviour; the defaults)

```sh
pubsub-node --self-id a \
  --subscription-list subs.toml --topic-registry topics.toml \
  --relay-strategy hash-gated --relay-acceptance-strategy hash-gated-bounded \
  --relay-degree 8
```

No publisher flags: the node dials no publisher links and drops inbound
publisher requests. Behaviour is identical to the pre-015 node.

## M3 — pull + publisher links (the adopted primary model)

```sh
pubsub-node … \
  --relay-strategy hash-gated --relay-acceptance-strategy hash-gated-bounded \
  --relay-degree 8 \
  --publisher-strategy hash-gated --publisher-acceptance-strategy hash-gated-bounded \
  --publisher-degree 4
```

Each node unconditionally establishes ~`--publisher-degree` standing publisher
links (independent hash domain). Locally-published messages go out over relay
downstream **and** active publisher links; relayed messages over relay links
only. M3's exclusivity (publisher links carry only their owner's publications)
is this **sender-side** fan-out default — the receive gate is kind-agnostic
and validates a publisher-link arrival exactly like any message.

Model-parameter mapping: the M3 model's *s* counts the publisher **plus** its
targets, so set `--publisher-degree` to **s − 1** when reproducing the model's
tables.

## Symmetric relay links — the M4 approximation

```sh
pubsub-node … \
  --relay-strategy hash-gated --relay-acceptance-strategy hash-gated \
  --relay-degree 8 --symmetric-edges
```

Relay links are established with the **symmetric handshake** (ADR 0034):
edges are drawn with the unordered-pair predicate under its own domain, and
one accept decision records each link in both directions on both ends —
reciprocity is constructed, not dependent on the two ends' draws agreeing.
Flooding runs over the resulting bidirectional mesh. No publisher flags.

**This is not yet the formal M4** and the recipe deliberately does not claim
the label: hash-gated selection draws a binomial number of edges per node
(expected degree ≈ `--relay-degree`, no minimum-degree floor), whereas M4's
defining property is uniform exactly-RF picks — minimum degree ≥ RF, hence
connectivity w.h.p. at RF ≥ 2 and no muted-publisher mode. A uniform
exactly-RF selection kind (the (B = 1, K = RF) point) is a follow-up feature;
once it lands, `--relay-strategy uniform … --symmetric-edges` will realise M4
exactly.

## M5 — directed k_in/k_out, both classes carry everything

```sh
pubsub-node … \
  --relay-strategy hash-gated --relay-acceptance-strategy hash-gated-bounded \
  --relay-degree 6 \
  --publisher-strategy hash-gated --publisher-acceptance-strategy hash-gated-bounded \
  --publisher-degree 6 \
  --fanout-strategy forward-to-all
```

`forward-to-all` sends every held message over both downstream classes. This
is deliberately the **only** switch that separates M5 from M3: the receive
side is uniform across the models (kind-agnostic gate), so the M3/M5
comparison isolates the fan-out axis.

## M1 — push-only (the M5 `k_in = 0` boundary)

```sh
pubsub-node … \
  --relay-strategy none --relay-acceptance-strategy none \
  --publisher-strategy hash-gated --publisher-acceptance-strategy hash-gated-bounded \
  --publisher-degree 8 \
  --fanout-strategy forward-to-all
```

No relay links at all: every node pushes over its ~`--publisher-degree`
standing out-links, which carry every held message (`forward-to-all`) —
RF-out push gossip. Structurally this is the M5 recipe with the relay seams
switched `none`.

## Observing topology

`Node::upstream_relays()` / `downstream_relays()` / `upstream_publishers()` /
`downstream_publishers()` snapshot the four link classes; the integration
tests (`tests/model_family.rs`, `tests/publisher_links.rs`) show fleet-level
assertions (reciprocity, coverage, sender-side exclusivity).
