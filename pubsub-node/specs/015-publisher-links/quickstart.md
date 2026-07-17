# Quickstart: running the dissemination models (015)

Every model is a per-node flag combination — no `--model` preset. Shared
setup (registries, config) is unchanged from the 005 quickstart; only the
strategy flags differ. `--genesis` seeds the epoch nonce (same genesis ⇒ same
topology, reproducible experiments).

## M2 — pull baseline (pre-015 behaviour; the defaults)

```sh
pubsub-node --self-id a --config node.toml \
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
only; a publisher link admits only its owner's publications.

Model-parameter mapping: the M3 model's *s* counts the publisher **plus** its
targets, so set `--publisher-degree` to **s − 1** when reproducing the model's
tables.

## M4 — bidirectional relay links

```sh
pubsub-node … \
  --relay-strategy hash-gated --relay-acceptance-strategy hash-gated \
  --relay-degree 8 --symmetric-edges
```

The symmetric predicate (unordered pair, own domain) makes both ends of a
valid edge dial each other; every link materialises as a reciprocal pair and
`forward-to-all` floods all incident links. No publisher flags. Expected
degree ≈ `--relay-degree` with binomial variance (no min-degree guarantee —
the documented approximation of the models' exactly-k picks).

## M5 — directed k_in/k_out, both classes carry everything

```sh
pubsub-node … \
  --relay-strategy hash-gated --relay-acceptance-strategy hash-gated-bounded \
  --relay-degree 6 \
  --publisher-strategy hash-gated --publisher-acceptance-strategy hash-gated-bounded \
  --publisher-degree 6 \
  --fanout-strategy all-links --publisher-admission any-verified
```

`all-links` sends every held message over both downstream classes;
`any-verified` admits foreign publishers' messages over inbound publisher
links. **Pair the two network-wide** — `all-links` senders against
`owner-only` receivers lose every publisher-link hop.

## Observing topology

`Node::upstream_relays()` / `downstream_relays()` / `upstream_publishers()` /
`downstream_publishers()` snapshot the four link classes; the integration
tests (`tests/model_family.rs`, `tests/publisher_links.rs`) show fleet-level
assertions (reciprocity, coverage, owner-binding).
