# Cardano PubSub Node

Rust implementation of the Cardano PubSub relay node, based on the D2 research paper (AUEB/IOG, 2024).

## Architecture

The node implements the three-layer dissemination protocol from D2 Ch.3:

- **Cyclon** — Gossip-based peer sampling with uniform random views
- **Vicinity** — Topic navigation via finger links on a circular topic ring (O(log T) routing)
- **Hybrid Dissemination** — Harary graph (deterministic delivery) + random links (fast propagation)

Relay nodes join the overlay permissionlessly via Cyclon gossip — no on-chain registration (D2 Ch.3).
Node identity is derived from the Ed25519 public key generated at startup.

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

A permanent seed node runs at `<<seed-host>>:9000`. Connect any local node to it:

```sh
pubsub-node \
  --bind 0.0.0.0:9001 \
  --peers <<seed-host>>:9000 \
  --name my-node \
  --topics ops/emergency/critical
```

> **Note:** Bind to `0.0.0.0` (not `127.0.0.1`) when connecting to a remote seed — QUIC needs
> a routable local address to exchange with the seed.

The seed's HTTP dashboard is at `http://<<seed-host>>:10000`.

## Quick Start (local testnet)

```sh
# Build
cd node
cargo build --release

# Launch 5-node local testnet
./testnet/launch.sh build

# In another terminal, publish a test message
cargo run --release --bin pubsub-cli -- \
  --node 127.0.0.1:9001 \
  publish --topic ops/emergency/critical --message "test alert"
```

## Node Usage

```sh
pubsub-node \
  --bind 0.0.0.0:9001 \
  --peers <<seed-host>>:9000 \
  --name node-1 \
  --topics ops/emergency/critical,gov/drep/test
```

## CLI Usage

```sh
# Publish
pubsub-cli --node 127.0.0.1:9001 publish --topic ops/emergency/critical --message "patch now"
```

## Configuration

| Flag | Default | Description |
|------|---------|-------------|
| `--bind` | `127.0.0.1:9000` | QUIC listen address (use `0.0.0.0` to reach remote peers) |
| `--advertise-addr` | same as `--bind` | Address announced to peers in gossip |
| `--peers` | none | Bootstrap peer addresses (comma-separated) |
| `--name` | `node-0` | Node name for logging |
| `--topics` | none | Topics to subscribe to (comma-separated) |
| `--cyclon-interval` | `5` | Cyclon gossip interval (seconds) |
| `--vicinity-interval` | `10` | Vicinity gossip interval (seconds) |
| `--http-port` | bind+1000 | HTTP dashboard port (0 to disable) |
| `--log-level` | `info` | Log level |

## Deploying the Seed Server

The seed runs as a systemd service on Ubuntu. To update the binary:

```sh
# Build + upload + restart (requires Docker for cross-compilation)
./scripts/deploy-seed.sh

# Upload existing binary only (skip Docker build)
./scripts/deploy-seed.sh --no-build

# Restart service without uploading
./scripts/deploy-seed.sh --restart
```

Server setup: UFW allows SSH (22), QUIC (9000/udp), HTTP dashboard (10000/tcp) only.
SSH key: `~/.ssh/pubsub`. See `scripts/deploy-seed.sh` for full details.

## What's Implemented

- [x] Core types and message envelope (CBOR)
- [x] Cyclon peer sampling (gossip-based, permissionless overlay entry)
- [x] Vicinity topic navigation
- [x] Hybrid Dissemination (Harary + random links)
- [x] QUIC transport (bidirectional streams for gossip, unidirectional for messages)
- [x] Message signing and validation (Ed25519, pool KES keys, DRep credentials)
- [x] In-memory hot cache with TTL eviction
- [x] Mock chain state for testnet
- [x] CLI for publish
- [x] Local testnet launcher (5 nodes)
- [x] Live seed node (<<seed-host>>:9000)
- [x] Cardano contract deployment CLI (`pubsub-admin`)

## What's Next

- [ ] Persistent node identity (load Ed25519 key from file across restarts)
- [ ] gRPC streaming API for real-time subscriptions
- [ ] Topic Registry on-chain integration (read via ogmios / pallas)
- [ ] WebTransport listener for browser clients
- [ ] SDK packages (TypeScript, Python)
- [ ] Replication servers + clique-DHT (D2 Ch.4)
