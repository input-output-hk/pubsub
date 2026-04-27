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

## Reachability and NAT

`--advertise-addr` is the address every other peer will dial to reach you. It's gossiped via Cyclon as part of your `NodeInfo`; downstream peers `endpoint.connect()` it directly. Defaults to `--bind`.

For a public host with a routable IP, set `--advertise-addr <public-ip>:9000` and bind to `0.0.0.0:9000`. For a node behind a home router:

1. Forward **UDP** (not TCP) on the router: `WAN udp/<port> → LAN <node-host-ip>:<port>`. Same number on both sides keeps it simple.
2. Bind locally to `0.0.0.0:<port>`.
3. Set `--advertise-addr <your-WAN-ip>:<port>` (find via `curl ifconfig.me` or the router's admin page).

Do **not** forward the HTTP API port (`--http-port`, default `bind+1000`). It's the dashboard/control plane and has no auth.

QUIC's keep-alive (`15s`, `transport/tls.rs`) holds the outbound NAT pinhole open so the seed can push gossip back to you on the original connection. Other peers opening *new* connections to you still need the forward to exist.

Behind carrier-grade NAT or a corp firewall with no port-forwarding, you can still publish and subscribe (outbound-only), but you cannot operate as a relay — peers cannot dial you back.

Common footgun: advertising a private IP (`192.168.x.x`, `10.x.x.x`, `172.16-31.x.x`, `169.254.x.x`, loopback). Silently fails outside your LAN. The startup warning at `main.rs:458` only catches `0.0.0.0` advertise-addr; private-IP detection is on the to-do list.

## Troubleshooting

**My node connects to the seed but I don't show up in the seed's topology.** Almost always wrong `--advertise-addr`. Your `NodeInfo` reached the seed; the seed gossiped it on; a third peer tried to dial you and failed. Check the dashboard at the seed's `:10000` — if your NodeId appears with the stale flag, advertise-addr is unreachable. Logs on your node should show *inbound* connection traces; if only outbound ever appears, port forwarding isn't set up.

**Zero peers, ever.** Bootstrap handshake failing. Check `--peers` reachable on UDP. TLS / NodeId-mismatch lines mean the seed's cert pubkey doesn't match what gossip claimed for it.

**Connections drop after ~30s of idle.** NAT pinhole expired despite the 15s keep-alive — some carrier-grade NATs drop UDP keep-alives. No workaround in-protocol today.

**`subscribe` returns history then goes silent.** HotCache replay worked but the live broadcast subscription dropped. Check the client log for `subscribe stream ended`; look at the node log for the matching disconnect.

**Publish returns `Rejected: TopicId bytes 8-31 are non-zero; not a registry-originated topic`.** You used `--topic <name>` (hash-form id) against a node configured for an on-chain topic registry. Use `--topic-id <int>` instead — query `/api/topics` to discover registered ids — or register the topic via `pubsub-admin create-topic`.

**Local 5-node testnet works but joining preprod doesn't.** Localhost has no NAT. Once you bind on a routable interface, advertise-addr + port-forwarding are live concerns.

## HTTP API

The node exposes a small HTTP surface for the dashboard, browser/curl clients, and ops tooling. Default port is `bind_port + 1000` (override with `--http-port`, set `0` to disable). All routes return JSON unless noted; SSE routes use `text/event-stream`. See `src/api.rs` for the canonical handlers.

| Method | Path | Response | Notes |
|--------|------|----------|-------|
| GET | `/api/status` | `StatusResponse` | Node id (hex + bech32), network, uptime, peer/topic/message counts |
| GET | `/api/peers` | `[PeerEntry]` | Connected peers with hex + bech32 ids and addresses |
| GET | `/api/topics` | `[TopicEntry]` | Topics known to this node (hex id, name, subscribed flag) |
| GET | `/api/messages?topic=<prefix>&limit=<N>` | `[StoredMessage]` | Last N entries from the dashboard ringbuffer; optional topic-hex prefix filter; default `limit=20` |
| GET | `/api/topology` | `TopologyResponse` | Self id + peers with `stale` flag (15s threshold) |
| GET | `/api/topics/{topic_hex}/stream?since=<n>&limit=<m>` | SSE `StreamMessage` | Per-topic replay-from-HotCache then live broadcast hits |
| GET | `/events` | SSE `NodeEvent` | Global event firehose (`peer_connected`, `message_received`) |

Examples:

```sh
# Snapshot node state
curl -s http://127.0.0.1:10001/api/status

# Tail a single topic, full TTL replay then live (Ctrl-C to exit)
TOPIC=0000000000000000000000000000000000000000000000000000000000000000
curl -N "http://127.0.0.1:10001/api/topics/$TOPIC/stream?since=0"

# Dashboard event firehose
curl -N http://127.0.0.1:10001/events
```

Response struct shapes are defined in `crates/pubsub-node/src/api.rs` (`StatusResponse`, `PeerEntry`, `TopicEntry`, `StoredMessage`, `TopologyResponse`, `StreamMessage`, `NodeEvent`).

## Subscribe wires

Three transports today, all reading from the same `subscriber_tx: broadcast::Sender<Message>` fan-out point in `main.rs` (set right after store + dedup in the receive-loop handler task):

| Wire | Consumer | Replay? | Per-topic? | Carries |
|------|----------|---------|-----------|---------|
| QUIC `SUBSCRIBE` (tag `0x03`) | `pubsub-cli`, native clients | yes (HotCache `get_since`) | yes | full `Message` (CBOR — incl. signature) |
| HTTP `/api/topics/{hex}/stream` | browsers, curl | yes (HotCache `get_since`) | yes | `StreamMessage` JSON (no signature) |
| HTTP `/events` | dashboard | no | no — global | `NodeEvent` JSON metadata |

Replay-then-live: the QUIC and per-topic SSE paths first drain `HotCache::get_since(topic, since_seq, limit)` and then attach to the broadcast for live frames. The seam between replay and live is best-effort — messages broadcast during the cache snapshot can be missed and are not de-duplicated against the replay batch.

## Message retention

Two caches sit side by side and have different eviction rules. Don't conflate them.

| Store | Location | Eviction | Used for |
|-------|----------|----------|----------|
| `HotCache` | `crates/pubsub-network/src/store.rs` (`DEFAULT_TTL = 3600s`, 100k entry cap) | Time-based: 1h TTL with periodic eviction every 60s. Capacity-based: oldest entry dropped when at cap. | Replay (`get_since`) for both QUIC and SSE subscribers |
| Dashboard `recent_messages` | `crates/pubsub-node/src/api.rs` (200-entry FIFO) | Capacity-only: oldest dropped when 201st arrives. **No time eviction.** | `/api/messages` JSON polling and dashboard preview |

The `recent_messages` ringbuffer is why a quiet node will display yesterday's messages on the dashboard even though `HotCache` evicted them an hour after arrival. Treat it as a debug preview, not an authoritative recent feed.
