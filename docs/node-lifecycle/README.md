# Node lifecycle procedures

This directory documents the procedures a node executes during its lifecycle in the list-based pubsub network: joining, finding peers, publishing/relaying, changing its topic subscriptions, endpoint updates, and leaving. Each procedure is a self-contained doc with a short intro, steps, a mermaid sequence diagram, and (when relevant) type definitions for the on-the-wire structures it uses.

## On-chain artifacts

- **Topic registry** — per-topic entry containing the topic identifier and the authorised publisher key(s) for that topic. Read by relayers to verify message signatures. Full formal specification (operations, role-based access control, invariants) lives under [`formal_spec/topic_registry/`](../../formal_spec/topic_registry/). See [topic-creation.md](./topic-creation.md) for the cross-reference.
- **Subscription list** — per-subscriber entry containing the operator's public key, the subscribed topic-interest set, and the locked deposit. Read to determine who participates in dissemination for each topic.

> [!IMPORTANT]
> Network endpoints (IPs/hostnames) are not on-chain. They are exchanged peer-to-peer as signed descriptors and served by bootstrap nodes during [IP discovery](./ip-discovery.md).

## Chain access

Every node runs a chain follower (light client at minimum). This is not optional: relayers must read the topic registry to verify message signatures against the authorised publisher key (see [publishing.md](./publishing.md)), and subscribers must read the subscription list to compute their candidate sets. A trustless, Byzantine-resistant deployment cannot delegate this to a third party.

A useful consequence: chain-event subscription is already available — flows that benefit from real-time chain reactions (e.g., reacting to subscription-list updates without polling) can hook into the existing follower rather than adding new infrastructure.

## Shared types

- **`SignedDescriptor`** — `(pubkey, endpoint, timestamp, signature)`. Authenticated endpoint binding for a registered node. The signature is produced with the operator's private key and covers `(pubkey, endpoint, timestamp)`. Used in [joining](./joining.md), [ip-discovery](./ip-discovery.md), [endpoint-change](./endpoint-change.md), and [leaving](./leaving.md).
  - *Open: single `endpoint` field today; dual-stack (IPv4 + IPv6) or multi-homed nodes would need multiple descriptors or a list-valued field.*

## Overview

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
| `subscription_list_poll_interval` | How often to re-read the subscription list for churn. | TBD (seconds–minutes range) |
| `descriptor_staleness_window` | Reject `SignedDescriptor`s older than this. | TBD |
| `endpoint_cache_ttl` | Drop cached endpoints after this time. | TBD |
| `bootstrap_endpoints` | Trusted bootstrap node endpoints (out-of-band). | site-specific |

