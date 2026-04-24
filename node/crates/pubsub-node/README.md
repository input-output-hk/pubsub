# pubsub-node

Cardano PubSub relay node binary.

## Purpose

Wires all `pubsub-network` components into a running relay node: binds a QUIC transport, bootstraps the peer overlay, subscribes to topics, and processes incoming messages (validate → relay-policy → store → disseminate).

## Usage

```
pubsub-node [OPTIONS]
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--bind` | `127.0.0.1:9000` | Address to listen on |
| `--advertise-addr` | same as `--bind` | Public address advertised to peers (set this to your external IP when behind NAT) |
| `--registry` | — | Path to `nodes.json` registry file for peer discovery |
| `--peers` | — | Comma-separated bootstrap addresses (overrides registry) |
| `--name` | `node-0` | Human-readable name (appears in logs) |
| `--topics` | — | Comma-separated topic names to subscribe to |
| `--cyclon-interval` | `5` | SecureCyclon gossip interval in seconds |
| `--vicinity-interval` | `10` | Vicinity gossip interval in seconds |
| `--log-level` | `info` | Log verbosity: `trace`, `debug`, `info`, `warn`, `error` |

## Bootstrap precedence

1. `--peers` — explicit addresses, used as-is (overrides registry)
2. `--registry path` — load `nodes.json`, connect to all listed nodes except self
3. Neither — start with empty view (useful for the first node in a network)

## Node identity

Each startup generates a fresh Ed25519 keypair. The `NodeId` is the 32-byte public key. For persistent identity across restarts a keyfile path would be added in a future release.

## Example — single node

```bash
pubsub-node --bind 127.0.0.1:9001 --topics ops/emergency/critical
```

## Example — join existing testnet via registry

```bash
pubsub-node \
  --bind 0.0.0.0:9001 \
  --advertise-addr 1.2.3.4:9001 \
  --registry /etc/pubsub/nodes.json \
  --topics ops/emergency/critical,gov/drep/test \
  --log-level info
```
