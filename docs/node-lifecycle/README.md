# Node lifecycle procedures

This directory documents the procedures a node executes during its lifecycle in the list-based pubsub network: joining, finding peers, publishing/relaying, changing its topic subscriptions, and leaving. Each procedure is a self-contained doc with steps, a mermaid sequence diagram, and (when relevant) type definitions for the on-the-wire structures it uses.

## On-chain artifacts

- **Topic registry** — per-topic entry containing the topic identifier and the authorised publisher key(s) for that topic. Read by relayers to verify message signatures.
- **Subscription list** — per-subscriber entry containing the operator's public key, the subscribed topic-interest set, and the locked deposit. Read to determine who participates in dissemination for each topic.

Network endpoints (IPs/hostnames) are not on-chain. They are exchanged peer-to-peer as signed descriptors and served by bootstrap nodes during [IP discovery](./ip-discovery.md).

## Shared types

- **`SignedDescriptor`** — `(pubkey, endpoint, timestamp, signature)`. Authenticated endpoint binding for a registered node. The signature is produced with the operator's private key and covers `(pubkey, endpoint, timestamp)`. Used in [joining](./joining.md), [ip-discovery](./ip-discovery.md), and [leaving](./leaving.md).

## Overview

| # | Procedure | Purpose |
|---|-----------|---------|
| 1 | [Joining and registering](./joining.md) | Operator subscribes to the network and becomes discoverable. |
| 2 | [IP discovery](./ip-discovery.md) | Resolve endpoints for working-set peers and establish dissemination links. |
| 3 | [Publishing and relaying](./publishing.md) | Inject a signed message into the topic and forward it through the overlay. |
| 4 | [Changing topic subscription](./changing-topic-subscription.md) | Add or remove a topic without leaving the network. |
| 5 | [Leaving and unregistering](./leaving.md) | Exit the network entirely and reclaim the deposit. |
