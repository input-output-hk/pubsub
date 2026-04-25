# pubsub-node

Cardano PubSub relay node binary.

## Purpose

Wires all `pubsub-network` components into a running relay node: binds a QUIC transport,
bootstraps the peer overlay, subscribes to topics, and processes incoming messages
(validate → relay-policy → store → disseminate).

## Usage

```
pubsub-node [OPTIONS]
```

## Options

**Instance flags** (set per node on the CLI):

| Flag | Default | Description |
|------|---------|-------------|
| `--bind` | `0.0.0.0:9000` | QUIC listen address |
| `--advertise-addr` | same as `--bind` | Public address announced to peers (set to external IP when behind NAT) |
| `--peers` | — | Bootstrap peer addresses (comma-separated) |
| `--name` | `node-0` | Human-readable name (appears in logs) |
| `--key-file` | — (ephemeral) | Path to 32-byte Ed25519 key file; created on first run if absent |
| `--http-port` | bind+1000 | HTTP dashboard port (0 to disable) |
| `--config` | — | Path to TOML config file (see below) |

**Network / chain flags** (settable via CLI or `--config` file; CLI wins):

| Flag | Default | Description |
|------|---------|-------------|
| `--topics` | — | Topic names to subscribe to at startup (comma-separated) |
| `--cyclon-interval` | `5` | Cyclon gossip interval in seconds |
| `--vicinity-interval` | `10` | Vicinity gossip interval in seconds |
| `--topic-refresh-interval` | `300` | Chain topic poll interval in seconds (0 to disable) |
| `--log-level` | `info` | Log verbosity: `trace`, `debug`, `info`, `warn`, `error` |
| `--blockfrost-key` | — | Blockfrost project ID (enables on-chain chain backend) |
| `--blockfrost-url` | preprod URL | Blockfrost REST API base URL |
| `--ogmios-url` | — | Ogmios JSON-RPC URL (alternative chain backend) |
| `--topic-validator-addr` | — | Bech32 address of the topic validator contract |
| `--publisher-vault-addr` | — | Bech32 address of the publisher vault contract |
| `--node-registry-addr` | — | Bech32 address of the node registry contract |
| `--registry-policy-id` | — | Hex minting policy ID (56 chars) |

## Config file (`--config`)

Network and chain flags can be placed in a TOML file instead of the CLI, keeping per-instance
flags separate from deployment config. Generated automatically by `pubsub-admin bootstrap`.

```sh
pubsub-node \
  --bind 0.0.0.0:9000 \
  --name seed-0 \
  --key-file /opt/pubsub/data/node.sk \
  --config local/config.preprod.toml
```

For the preprod network, `node/config.preprod.toml` in this repo contains the live contract
addresses. Add your `blockfrost_key` to a local copy and point `--config` at it.

See `node/config.example.toml` for a documented template.

## Bootstrap precedence

1. `--peers <addr,...>` — explicit bootstrap addresses (passed directly to Cyclon)
2. Neither — start with empty view (useful for the first/seed node)

## Node identity

Node identity is derived from a 32-byte Ed25519 key. With `--key-file`, the key is persisted
to disk and reloaded on restart (stable `NodeId`). Without it, a fresh ephemeral key is
generated each run.

## Example — single node (local testnet, no chain backend)

```bash
pubsub-node --bind 127.0.0.1:9001 --topics ops/emergency/critical
```

## Example — join preprod via seed node with on-chain topics

```bash
pubsub-node \
  --bind 0.0.0.0:9000 \
  --advertise-addr <your-ip>:9000 \
  --peers 116.202.30.232:9000 \
  --key-file ~/.pubsub/node.sk \
  --config local/config.preprod.toml
```
