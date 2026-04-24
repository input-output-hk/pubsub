# Cardano PubSub Node

Rust implementation of the Cardano PubSub relay node, based on the D2 research paper (AUEB/IOG, 2024).

## Architecture

The node implements the three-layer dissemination protocol:

- **SecureCyclon** — Eclipse-resistant peer sampling with uniform random views
- **Vicinity** — Topic navigation via finger links on a circular topic ring (O(log T) routing)
- **Hybrid Dissemination** — Harary graph (deterministic delivery) + random links (fast propagation)

Every component is behind a trait interface for modularity. See `crates/pubsub-types/src/traits.rs`.

## Crates

| Crate | Description |
|-------|-------------|
| `pubsub-types` | Core types, message envelope, trait definitions |
| `pubsub-network` | Protocol implementations (SecureCyclon, Vicinity, Hybrid Dissemination, QUIC transport) |
| `pubsub-node` | Main node binary |
| `pubsub-cli` | CLI tool for publishing and subscribing |

## Quick Start

```bash
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

```bash
pubsub-node \
  --bind 127.0.0.1:9001 \
  --peers 127.0.0.1:9002,127.0.0.1:9003 \
  --name node-1 \
  --topics ops/emergency/critical,gov/drep/test
```

## CLI Usage

```bash
# Publish
pubsub-cli --node 127.0.0.1:9001 publish --topic ops/emergency/critical --message "patch now"

# Subscribe (streaming — requires gRPC, coming soon)
pubsub-cli --node 127.0.0.1:9001 subscribe --topic ops/emergency/critical
```

## Configuration

| Flag | Default | Description |
|------|---------|-------------|
| `--bind` | `127.0.0.1:9000` | Node bind address |
| `--peers` | none | Bootstrap peer addresses |
| `--name` | `node-0` | Node name for logging |
| `--topics` | none | Topics to subscribe to |
| `--cyclon-interval` | `5` | SecureCyclon gossip interval (seconds) |
| `--vicinity-interval` | `10` | Vicinity gossip interval (seconds) |
| `--log-level` | `info` | Log level |

## What's Implemented

- [x] Core types and message envelope (CBOR)
- [x] SecureCyclon peer sampling
- [x] Vicinity topic navigation
- [x] Hybrid Dissemination (Harary + random links)
- [x] QUIC transport
- [x] Message signing and validation (Ed25519)
- [x] In-memory hot cache with TTL eviction
- [x] Mock chain state for testnet
- [x] CLI for publish
- [x] Local testnet launcher (5 nodes)

## What's Next

- [ ] gRPC streaming API for real-time subscriptions
- [ ] Persistent node identity (load keys from file)
- [ ] Proper peer discovery via SecureCyclon bootstrapping
- [ ] Topic Registry on-chain integration (ogmios/cardano-node)
- [ ] WebTransport listener for browser clients
- [ ] SDK packages (TypeScript, Python)
