# Cardano PubSub Node

Rust implementation of the Cardano PubSub relay node, based on the D2 research paper (AUEB/IOG, 2024).

## Architecture

The node implements the three-layer dissemination protocol from D2 Ch.3:

- **Cyclon with eclipse-resistance extensions** — Gossip-based peer sampling. Vanilla Cyclon (Voulgaris–Gavidia–van Steen, 2005) plus three extensions from the Jesi–Montresor–Babaoglu "SecureCyclon" paper (2007): signed PeerDescriptors, bootstrap diversity, rate-limited insertion
- **Vicinity** — Topic navigation via finger links on a circular topic ring (O(log T) routing)
- **Hybrid Dissemination** — Harary graph (deterministic delivery) + random links (fast propagation)

Relay nodes join the overlay permissionlessly via gossip — no on-chain registration (D2 Ch.3).
Identity is self-certifying: `NodeId = Blake2b-256(public_key)`; the key is carried in every `PeerDescriptor` so recipients verify signatures without any registry lookup.

Every component is behind a trait interface for modularity. See `crates/pubsub-types/src/traits.rs`.

## Crates

| Crate | Description |
|-------|-------------|
| `pubsub-types` | Core types, message envelope, trait definitions |
| `pubsub-network` | Protocol implementations (Cyclon, Vicinity, Hybrid Dissemination, QUIC transport) |
| `pubsub-node` | Main node binary |
| `pubsub-cli` | CLI tool for publishing and subscribing |
| `pubsub-admin` | Cardano contract deployment CLI |

## Live Seed Node (preprod)

A permanent seed node runs on preprod at `116.202.30.232:9000`.
Connect any local node to it:

```sh
pubsub-node \
  --bind 0.0.0.0:9001 \
  --peers 116.202.30.232:9000 \
  --name my-node \
  --config node/config.preprod.toml
```

> **Note:** Bind to `0.0.0.0` (not `127.0.0.1`) when connecting to a remote seed — QUIC needs
> a routable local interface to send outbound packets.

The seed's HTTP dashboard is at `http://116.202.30.232:10000`.

`node/config.preprod.toml` in this repo contains the live contract addresses. Add your
`blockfrost_key` to a local copy to enable on-chain topic discovery.

## Quick Start (local testnet)

```sh
# Build
cd node
cargo build --release

# Launch 5-node local testnet
./testnet/launch.sh --build

# In another terminal, publish a test message
cargo run --release --bin pubsub-cli -- \
  --node 127.0.0.1:9001 \
  publish --topic ops/emergency/critical --message "test alert"
```

## Node Usage

```sh
pubsub-node \
  --bind 0.0.0.0:9001 \
  --peers <seed-host>:9000 \
  --name node-1 \
  --topics iog/spo/alerts
```

## CLI Usage

```sh
# Publish (off-chain — TopicId = Blake2b-256(name))
pubsub-cli --node 127.0.0.1:9001 publish --topic ops/emergency/critical --message "patch now"

# Publish to a chain-registered topic (TopicId = on-chain integer id, e.g. 0)
pubsub-cli --node 127.0.0.1:9001 publish --topic-id 0 --message "patch now"

# Subscribe — replay-from-cache then live, Ctrl-C to exit
pubsub-cli --node 127.0.0.1:9001 subscribe --topic-id 0
```

Exactly one of `--topic <name>` / `--topic-id <u64>` is required (mutually exclusive). Publish is bidirectional — the node returns a `PublishAck::Accepted{topic_id, sequence_nr}` on success or `Rejected{reason}` on validation failure (CLI exits non-zero with the reason). See `crates/pubsub-cli/README.md` for full details.

## TopicId conventions

`TopicId` is a 32-byte value with two encodings depending on origin:

| Form | Encoding | Source | CLI flag |
|------|----------|--------|----------|
| **Hash** | `Blake2b-256(topic_name)` | Off-chain / mock-chain testing | `--topic <name>` |
| **On-chain int** | BE-encoded `u64` in bytes 0..8, zeros in 8..32 | Chain-registered topics (assigned by the on-chain registry) | `--topic-id <u64>` |

The two forms are disjoint in practice. Targeting a chain topic by name will produce a hash-form id that won't match the on-chain integer id — the validator will reject the message as `TopicNotFound`. See `crates/pubsub-types/README.md` for the canonical contract.

## Subscribe / publish surface

| Wire | Consumer | Replay | Per-topic | Carries |
|------|----------|--------|-----------|---------|
| QUIC `SUBSCRIBE` (tag `0x03`) | `pubsub-cli`, native clients | yes (HotCache) | yes | full `Message` (CBOR) |
| HTTP `/api/topics/{hex}/stream` | browsers, curl | yes (HotCache) | yes | `StreamMessage` JSON (no signature) |
| HTTP `/events` | dashboard | no | no — global | `NodeEvent` JSON metadata |
| QUIC `PUBLISH` (tag `0x04`) | `pubsub-cli`, native clients | n/a | n/a | one `Message` → `PublishAck` |

See `crates/pubsub-node/README.md` for HTTP endpoint details and retention semantics; `crates/pubsub-types/README.md` for wire-protocol tags; `crates/pubsub-network/README.md` for the QUIC transport stream patterns.

## Configuration

Instance flags (CLI only):

| Flag | Default | Description |
|------|---------|-------------|
| `--bind` | `0.0.0.0:9000` | QUIC listen address |
| `--advertise-addr` | same as `--bind` | Address announced to peers in gossip |
| `--peers` | none | Bootstrap peer addresses (comma-separated) |
| `--name` | `node-0` | Node name for logging |
| `--key-file` | none (ephemeral) | Path to 32-byte Ed25519 key file; created on first run if absent |
| `--http-port` | bind+1000 | HTTP dashboard port (0 to disable) |
| `--config` | none | Path to TOML config file (see `node/config.preprod.toml`) |

Network / chain flags (settable via CLI or `--config`; CLI wins):

| Flag | Default | Description |
|------|---------|-------------|
| `--topics` | none | Topics to subscribe to at startup (comma-separated) |
| `--cyclon-interval` | `5` | Cyclon gossip interval (seconds) |
| `--vicinity-interval` | `10` | Vicinity gossip interval (seconds) |
| `--log-level` | `info` | Log level |
| `--blockfrost-key` | none | Blockfrost project ID (enables on-chain topic discovery) |
| `--blockfrost-url` | preprod URL | Blockfrost REST API base URL |
| `--topic-validator-addr` | none | Bech32 address of the topic validator contract |
| `--publisher-vault-addr` | none | Bech32 address of the publisher vault contract |
| `--registry-policy-id` | none | Hex minting policy ID (56 chars) |

## What's Implemented

- [x] Core types and message envelope (CBOR)
- [x] Cyclon peer sampling (gossip-based, permissionless overlay entry)
- [x] Vicinity topic navigation (per-topic finger tables)
- [x] Hybrid Dissemination (Harary + random links)
- [x] QUIC transport: uni (app msgs), bi-oneshot (gossip + publish), bi-streaming (subscribe)
- [x] Message signing and validation (Ed25519, pool KES keys, DRep credentials)
- [x] CLI publish with server-side ack (`PublishAck::Accepted{topic_id, seq}` / `Rejected{reason}`)
- [x] CLI subscribe with replay from HotCache, then live broadcast
- [x] Per-topic SSE endpoint for browser clients (`/api/topics/{hex}/stream`)
- [x] In-memory HotCache with 1h TTL eviction
- [x] Ogmios + Blockfrost chain state backends (reads topic registry from chain)
- [x] Local testnet launcher (configurable node count via `--nodes N`)
- [x] TOML config file support (`--config`; generated by `pubsub-admin bootstrap`)
- [x] Live seed node (preprod, `116.202.30.232:9000`)
- [x] Cardano contract deployment CLI (`pubsub-admin bootstrap`, `publish-scripts`, `create-topic`)
- [x] Persistent node identity via `--key-file` (Ed25519 key survives restarts)
- [x] TLS peer verification (NodeId derived from cert public key; mismatch drops connection)
- [x] HTTP dashboard with peer topology and message feed

## What's Next

- [ ] WebTransport listener for browser clients (currently SSE; WebTransport gives bidirectional streams)
- [ ] SDK packages (TypeScript, Python) wrapping the QUIC + HTTP wire formats
- [ ] CardanoChainState completion — `get_pool_kes_keys` / `get_drep_keys` / `get_authority_keys` are `todo!()` stubs; `pool-kes` / `drep` / `authority` credentials only work against `MockChainState`
- [ ] Replication servers + clique-DHT (D2 Ch.4) — durable storage tier with on-chain registration, locked stake, and slashable storage commitments. Relay nodes (Ch.3) remain permissionless.
