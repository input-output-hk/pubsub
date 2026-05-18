# Peer-list TOML schema

**Feature**: 001-minimal-node-scaffold
**Source of truth**: `src/config.rs` (`PeerListConfig`, `PeerEntry`)
**Spec trace**: FR-001 (TOML config file), FR-009 (peer descriptor identity), Clarifications S1-Q2 (TOML), S2-Q1 (parse-at-edge)

## Schema (v1)

```toml
# Each peer this node should be aware of. The node's own id is NOT listed here;
# it is supplied on the command line via --self-id (or as the first argument to
# Node::new in library use).
[[peers]]
id = "node-b"

[[peers]]
id = "node-c"
```

### Fields

| Path | Type | Required | Notes |
|------|------|----------|-------|
| `peers` | array of tables | optional | When omitted or empty, the node starts with an empty peer set (spec Edge Cases bullet 1 — valid, may still receive incoming). |
| `peers[].id` | string | yes | Non-empty UTF-8, no internal NULs. Parses as a `PeerId` (FR-009). Duplicate ids within `peers` are NOT detected at load time (Configuration trust assumption); behaviour if duplicates appear is implementation-defined and not tested. |

### What is intentionally NOT in v1

- **No `[self]` table.** Node identity is supplied externally (Clarifications S2-Q1).
- **No version field.** Schema versioning is deferred until the first breaking schema change; then an ADR documents the migration.
- **No transport-layer fields** (`addr`, `port`, …). These arrive with the first networked transport iteration.
- **No cryptographic fields** (`pubkey`, `key_path`, …). FR-007 forbids them at this stage.
- **No comments / annotations beyond standard TOML `#`**. The parser is read-only; round-tripping is not required.

## Validation pipeline (loader: `config::load_peer_list`)

1. Read file at `path` → `String`. On failure: `ConfigError::Io { path, source }`.
2. `toml::from_str::<PeerListConfig>(&content)`. On failure: `ConfigError::Parse { path, source }`. The error's `Display` includes line/column, satisfying US3 AS-2.
3. For each `PeerEntry`, re-validate `id` through `PeerId::from_str` (catches empty strings that slipped past `serde`). On failure: `ConfigError::InvalidPeer(reason)`.
4. Return `PeerListConfig`.

## Examples

### Two-node peer list (used in US1 / two-node integration test)

```toml
# config/node-a.peers.toml
[[peers]]
id = "node-b"
```

```toml
# config/node-b.peers.toml
[[peers]]
id = "node-a"
```

### Star graph (US2 P2 — centre node "A" with peers B/C/D)

```toml
# config/node-a.peers.toml
[[peers]]
id = "node-b"

[[peers]]
id = "node-c"

[[peers]]
id = "node-d"
```

```toml
# config/node-b.peers.toml
# (B, C, D start with empty peer sets per the US2 AS-1 scenario)
```

### Empty peer set (Edge Cases bullet 1)

```toml
# config/observer.peers.toml
# An empty file is valid. The node cannot originate sends but may still receive.
```

(The `peers` key is `#[serde(default)]`; an absent or empty `peers` array is accepted.)

### Malformed (US3 AS-2 negative path)

```toml
# Will fail with ConfigError::Parse
[[peers]
id = "node-b"
```

```toml
# Will fail with ConfigError::InvalidPeer (empty id)
[[peers]]
id = ""
```

## Forward-compatibility note

Because each peer is its own table (`[[peers]]`), v2 can add per-peer fields (`addr`, `pubkey`, etc.) without changing the v1 reader's parse path. Unknown fields will be **rejected** by `serde` unless the v1 reader is amended to add `#[serde(deny_unknown_fields)] -> #[serde(default)]` semantics on the relevant struct. The v1 default is strict (`deny_unknown_fields` on `PeerEntry`) so that operators see a clear error if they configure something the running binary does not understand.
