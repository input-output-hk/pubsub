# pubsub-network

Protocol implementations for the Cardano PubSub three-layer dissemination stack.

## Purpose

Implements every trait defined in `pubsub-types`. Each module is a self-contained component of the D2 research paper's overlay architecture.

## Components

### `transport` — QUIC transport (`QuicTransport`)

Manages outgoing and incoming QUIC connections using [Quinn](https://github.com/quinn-rs/quinn). Frames messages with a 4-byte big-endian length prefix.

- Testnet: self-signed TLS certificates, server verification skipped
- Connections stored by `NodeId`; `connect(NodeInfo)` must be called before `send`

### `cyclon` — Cyclon peer sampling with eclipse-resistance extensions (`Cyclon`)

Gossip-based partial view maintenance. Vanilla Cyclon (Voulgaris–Gavidia–van Steen, 2005) plus three eclipse-resistance extensions from the Jesi–Montresor–Babaoglu "SecureCyclon" paper (2007):

1. **Signed PeerDescriptors** — every node signs its own descriptor; recipients verify via the public key embedded in the descriptor (self-certifying: `NodeId = Blake2b-256(public_key)`, no registry needed).
2. **Bootstrap diversity** — `is_warm()` returns `true` only after connecting to ≥ `min_seed_diversity` distinct seed origins.
3. **Rate-limited insertion** — `merge_received()` caps new peer insertions at `max_new_per_merge` per call to slow eclipse attacks.

Each cycle selects the oldest peer, sends a signed shuffle buffer, reads the response, and merges verified descriptors.

**Config (`CyclonConfig`):**

| Field | Default | Description |
|-------|---------|-------------|
| `view_size` | 20 | Maximum peers in the local view |
| `shuffle_length` | 10 | Entries exchanged per gossip round |
| `verify_signatures` | `true` | Reject descriptors with missing or invalid signatures |
| `min_seed_diversity` | 2 | Distinct seed origins required before `is_warm()` returns true |
| `max_new_per_merge` | 10 | New peers inserted per merge call (0 = unlimited; default = 50% of view_size) |

### `vicinity` — Topic ring navigation (`Vicinity`)

Maintains exponential finger tables over a 2³²-element topic ring so that any topic can be routed to in O(log T) hops.

**Config (`VicinityConfig`):**

| Field | Default | Description |
|-------|---------|-------------|
| `finger_base` | 2 | Exponential spacing base |
| `max_fingers` | 32 | Maximum fingers per direction |
| `gossip_sample_size` | 10 | Peers evaluated per cycle |

### `dissemination` — Hybrid dissemination (`HybridDisseminator`)

Combines a deterministic neighbor backbone (cyclic NodeId ordering) with random overlay links for fast propagation. Includes a bounded seen-set for deduplication.

**Config (`DisseminationConfig`):**

| Field | Default | Description |
|-------|---------|-------------|
| `fault_tolerance` | 6 | Neighbor connections per direction (`t/2` each way) |
| `fanout` | 3 | Random links per topic |
| `seen_set_capacity` | 10 000 | Max entries before oldest are evicted |

### `codec` — CBOR codec (`CborCodec`)

Encodes/decodes `Message` using [ciborium](https://github.com/enarx/ciborium). Used for both the CLI-to-node wire format and inter-node message forwarding.

### `validator` — Signature validator (`SignatureValidator`)

Verifies Ed25519 signatures and checks publisher authorization against `ChainState`. Rejects messages with invalid signatures or unauthorized publishers.

### `relay_policy` — Relay policy (`DefaultRelayPolicy`)

Phase 1: unconditionally forwards all valid messages. Placeholder for future rate-limiting and BFT checks.

### `store` — Hot cache (`HotCache`)

In-memory DashMap-backed message cache with TTL eviction (default 1 hour). Keyed by `(TopicId, PublisherId, sequence_nr)` — two publishers on the same topic at the same sequence number are stored independently.

**Constructors:**
- `HotCache::new(max_entries)` — custom capacity
- `HotCache::with_defaults()` — 100 000 entries

### `mock_chain` — Mock chain state (`MockChainState`)

In-memory implementation of `ChainState` for testnet. Returns fixed stake (1 000 000 lovelace) for all nodes.

### `mock_registry` — Mock node registry (`MockNodeRegistry`)

In-memory `NodeRegistry` backed by a `DashMap`. Supports loading from a JSON file for testnet peer discovery.

**Constructors:**
- `MockNodeRegistry::new()` — empty
- `MockNodeRegistry::from_nodes(Vec<NodeInfo>)` — pre-populated (tests)
- `MockNodeRegistry::from_file(&Path)` — load from `nodes.json`

**`nodes.json` format:**
```json
{
  "nodes": [
    {
      "addr": "127.0.0.1:9001",
      "public_key": null,
      "subscribed_topics": ["ops/emergency/critical", "gov/drep/test"]
    }
  ]
}
```
`public_key` is optional hex-encoded Ed25519 key. If absent, `NodeId` is derived from the socket address via BLAKE2b.
