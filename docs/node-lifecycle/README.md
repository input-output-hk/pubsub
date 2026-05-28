# Node lifecycle procedures

Procedures a node executes during its lifecycle in the list-based pubsub network.

## On-chain artifacts

Every node runs a chain follower (light client at minimum) — a trustless, Byzantine-resistant deployment cannot delegate this. Chain-event subscription is therefore already available: flows that need real-time reactions to chain state can hook into the existing follower.

| Artifact | Per-entry contents | Read for | Reference |
|----------|-------------------|----------|-----------|
| **Topic registry** | topic identifier, authorised publisher key(s) | relayers verify message signatures | [`formal_spec/topic_registry/`](../../formal_spec/topic_registry/), [topic-creation.md](./topic-creation.md) |
| **Subscription list** | node identity pubkey, topic-interest set, locked deposit | subscribers compute candidate sets | — |

> [!IMPORTANT]
> Network endpoints (IPs/hostnames) are not on-chain. They are exchanged peer-to-peer as signed descriptors and served by bootstrap nodes during [IP discovery](./ip-discovery.md).

## Shared types

- **`SignedDescriptor`** — `(pubkey, endpoint, timestamp, signature)`. Authenticated endpoint binding for a registered node. `pubkey` is the node identity from the subscription list; the signature is produced with the corresponding private key (held only by the daemon — the operator wallet is not used at runtime). Used in [joining](./joining.md), [ip-discovery](./ip-discovery.md), [endpoint-change](./endpoint-change.md), and [leaving](./leaving.md).
  - *Open: single `endpoint` field today; dual-stack (IPv4 + IPv6) or multi-homed nodes would need multiple descriptors or a list-valued field.*

## Overview

Per-procedure docs, in roughly the order a node encounters them. Each row links to the dedicated doc; status indicates the state of the spec.

| # | Procedure | Status |
|---|-----------|--------|
| 1 | [Joining and registering](./joining.md) | Specified |
| 2 | [IP discovery](./ip-discovery.md) | Specified |
| 3 | [Publishing and relaying](./publishing.md) | Specified |
| 4 | [Changing topic subscription](./changing-topic-subscription.md) | Specified |
| 5 | [Endpoint change](./endpoint-change.md) | Specified |
| 6 | [Leaving and unregistering](./leaving.md) | Specified |
| 7 | [Topic creation and registry management](./topic-creation.md) | Formally specified ([Quint](../../formal_spec/topic_registry/)) |
| 8 | [Catch-up / replay](./catch-up.md) | **TBD** — placeholder |

## Configuration parameters

The following knobs are configurable at the node level. Defaults are not yet pinned; the bootstrap-endpoint list is site-specific and lives in the same config.

| Parameter | Description | Suggested default |
|-----------|-------------|-------------------|
| `dissemination_fanout` (`d`) | Random-link peers per topic. Derived dynamically from the current network size `n` (subscription-list cardinality) at each refresh; operating point is `d ≈ ln(n)` plus a safety margin. Publishers use the same outgoing links to inject messages — no separate publication fanout. | dynamic |
| `d_min_warn` | Threshold below which the node's actual fanout triggers a warning log. | `⌈d / 2⌉` |
| `d_min_error` | Threshold below which the node's actual fanout triggers an error log. | `2` (Fenner–Frieze connectivity floor) |
| `subscription_list_poll_interval` | How often to re-read the subscription list for churn. | TBD (seconds–minutes range) |
| `descriptor_staleness_window` | Reject `SignedDescriptor`s older than this. | TBD |
| `endpoint_cache_ttl` | Drop cached endpoints after this time. | TBD |
| `bootstrap_endpoints` | Trusted bootstrap node endpoints (out-of-band). | site-specific |

