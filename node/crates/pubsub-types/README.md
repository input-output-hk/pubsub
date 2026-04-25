# pubsub-types

Shared types and trait definitions for the Cardano PubSub relay network.

## Purpose

Single source of truth for all types and async trait interfaces used across the crate family. Nothing in this crate does I/O — it only defines shapes and contracts.

## Contents

### Types

| Type | Description |
|------|-------------|
| `Message` | Wire envelope: topic, sequence number, timestamp, publisher ID, signature, payload, metadata |
| `TopicId` | 256-bit topic identifier (BLAKE2b hash of the topic name) |
| `PublisherId` | Publisher identity (Ed25519 public key bytes) |
| `MessageId` | Composite dedup key: `(TopicId, PublisherId, sequence_nr)` |
| `NodeId` | 32-byte node identifier — `Blake2b-256(public_key)` |
| `NodeInfo` | Full node descriptor: ID, socket address, public key, subscribed topics |
| `PeerDescriptor` | `NodeInfo` + age counter + Ed25519 signature; used by SecureCyclon gossip |
| `TopicConfig` | On-chain topic metadata: name, authorized publishers, retention period, replication factor |
| `PubSubError` | Error enum covering transport, codec, validation, and chain-state failures |

### Traits

| Trait | Implemented by |
|-------|---------------|
| `Transport` | `QuicTransport` |
| `PeerSampler` | `Cyclon` |
| `TopicRouter` | `Vicinity` |
| `Disseminator` | `HybridDisseminator` |
| `Codec` | `CborCodec` |
| `MessageValidator` | `SignatureValidator` |
| `RelayPolicy` | `DefaultRelayPolicy` |
| `MessageStore` | `HotCache` |
| `ChainState` | `MockChainState` |
| `NodeRegistry` | `MockNodeRegistry` |
| `SubscriptionManager` | `LocalSubscriptionManager` (in `pubsub-node`) |

## Usage

```toml
[dependencies]
pubsub-types = { path = "../pubsub-types" }
```

All traits are `async` via `async-trait`. Types derive `serde::{Serialize, Deserialize}` where needed for CBOR wire encoding.
