# pubsub-types

Shared types and trait definitions for the Cardano PubSub relay network.

## Purpose

Single source of truth for all types and async trait interfaces used across the crate family. Nothing in this crate does I/O — it only defines shapes and contracts.

## Contents

### Types

| Type | Description |
|------|-------------|
| `Message` | Wire envelope: topic, sequence number, timestamp, publisher ID, signature, payload, metadata |
| `TopicId` | 256-bit topic identifier — see [TopicId conventions](#topicid-conventions) |
| `CredentialType` | Tag selecting which on-chain registry validates the publisher key (`Ed25519`, `PoolKes`, `DRepCredential`, `AuthorityKey`) |
| `PublisherCredential` | Typed credential carried in every message (key bytes + type tag + optional auxiliary proof) |
| `PublisherId` | Publisher identity wrapping a `PublisherCredential` |
| `MessageId` | Composite dedup key: `(TopicId, PublisherId, sequence_nr)` |
| `SubscribeRequest` | CBOR control frame on a `SUBSCRIBE` stream: `topic_id`, `since_seq`, `limit` |
| `PublishAck` | CBOR response on a `PUBLISH` stream: `Accepted{topic_id, sequence_nr}` or `Rejected{reason}` |
| `NodeId` | 32-byte node identifier — `Blake2b-256(public_key)` |
| `NodeInfo` | Full node descriptor: ID, socket address, public key, subscribed topics |
| `PeerDescriptor` | `NodeInfo` + age counter + Ed25519 signature; used by Cyclon (SecureCyclon variant) gossip |
| `TopicConfig` | On-chain topic metadata: name, authorized publishers, retention period, replication factor |
| `PubSubError` | Error enum covering transport, codec, validation, and chain-state failures |

### Traits

| Trait | Implemented by |
|-------|---------------|
| `Transport` | `QuicTransport` (uni stream — inter-node app messages) |
| `GossipTransport` | `QuicTransport` (bi one-shot — Cyclon, Vicinity) |
| `SubscribeTransport` | `QuicTransport` (bi streaming response — client subscribe) |
| `PublishTransport` | `QuicTransport` (bi one-shot — client publish with ack) |
| `PeerSampler` | `Cyclon` (with SecureCyclon extensions: signed descriptors, bootstrap diversity, rate-limited insertion) |
| `TopicRouter` | `Vicinity` |
| `Disseminator` | `HybridDisseminator` |
| `Codec` | `CborCodec` |
| `MessageValidator` | `SignatureValidator` |
| `RelayPolicy` | `DefaultRelayPolicy` |
| `MessageStore` | `HotCache` |
| `ChainState` | `MockChainState`, `CardanoChainState` |
| `NodeRegistry` | `MockNodeRegistry` (Phase-1 only — Ch.4 replication-server tier, feature-gated) |
| `SubscriptionManager` | `LocalSubscriptionManager` (in `pubsub-node`) |

## Wire protocol tags

A single byte tag prefixes every bidirectional QUIC stream so the responder can route the request to the correct handler. Defined as `pub const` in `src/traits.rs`.

| Tag | Const | Stream pattern | Direction | Purpose |
|-----|-------|---------------|-----------|---------|
| `0x01` | `GOSSIP_CYCLON` | bi, one-shot | node ↔ node | Cyclon shuffle exchange (SecureCyclon — signed `PeerDescriptor`s) |
| `0x02` | `GOSSIP_VICINITY` | bi, one-shot | node ↔ node | Vicinity T-Man |
| `0x03` | `SUBSCRIBE` | bi, streaming response | client → node | Subscribe (replay then live) |
| `0x04` | `PUBLISH` | bi, one-shot | client → node | Publish with `PublishAck` |

Inter-node application-message forwarding uses **unidirectional** streams with no tag and no ack — `Transport::send` opens a uni stream, frames the CBOR-encoded `Message`, and finishes. The receiving node's accept loop reads the frame and runs the same validate-store-disseminate pipeline as the publish-accept handler.

Frame format on every stream: `[4-byte BE length][payload]`. For tagged bi-streams, the leading byte of the payload is the tag.

## TopicId conventions

`TopicId` is a 32-byte value, but two distinct encodings flow through the system depending on whether the topic is registered on-chain.

- **Hash form** — `Blake2b-256(topic_name)`. Used for off-chain / mock-chain testing where the topic name is the source of truth. CLI default when `--topic <name>` is supplied.
- **On-chain int form** — BE-encoded `u64` in bytes `0..8`, zeros in `8..32`. Used for chain-registered topics where the registry assigns incrementing integer ids. CLI when `--topic-id <u64>` is supplied. The conversion is `pallas_chain::on_chain_int_to_topic_id`. The reverse helper rejects any TopicId whose bytes `8..32` are non-zero (i.e. a hash-form id) — see `crates/pubsub-network/src/pallas_chain.rs:256-261`.

The two encodings collide for `name`s whose Blake2b-256 hash happens to fit the int-form pattern (vanishingly unlikely), but in practice they are disjoint. A publish targeting a chain topic by name will not match the on-chain id and the `SignatureValidator` will reject it as `TopicNotFound`.

## Usage

```toml
[dependencies]
pubsub-types = { path = "../pubsub-types" }
```

All traits are `async` via `async-trait`. Types derive `serde::{Serialize, Deserialize}` where needed for CBOR wire encoding.
