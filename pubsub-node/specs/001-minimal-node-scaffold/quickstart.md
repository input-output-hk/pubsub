# Quickstart — Minimal PubSub Node Scaffold

**Feature**: 001-minimal-node-scaffold
**Goal**: Reproduce the two-node Ping demonstration (US1) in under one hour, per SC-004, without consulting any document outside this feature directory.

## Prerequisites

- Rust stable toolchain ≥ 1.75 (`rustup show` should list a stable channel; install via <https://rustup.rs> if absent).
- A POSIX shell (the commands below use bash / fish syntax interchangeably).
- This repository checked out; working directory is `pubsub-node/`.

No other system dependencies. No databases, no message brokers, no Docker.

## 1 — Build

```sh
cargo build
```

First build pulls dependencies (`tokio`, `serde`, `toml`, `tracing`, `tracing-subscriber`, `clap`, `thiserror`); subsequent builds are incremental. Expected duration: 30–90s on a modern laptop the first time, <2s thereafter.

If the build fails, check that the active toolchain is ≥ 1.75:

```sh
rustc --version   # should print 1.75.0 or higher
```

## 2 — Run the two-node Ping integration test (US1)

```sh
cargo test --test two_node_ping
```

Expected output (lines abbreviated):

```text
running 3 tests
test ping_delivered_when_a_lists_b ... ok
test ping_delivered_trust_on_arrival ... ok
test empty_peer_set_cannot_originate ... ok

test result: ok. 3 passed; 0 failed; 0 ignored
```

Each test:
1. Builds an `Arc<InMemoryNetwork>`.
2. Constructs two `Node`s with `PeerId`s `node-a` and `node-b`, each receiving a small in-memory `PeerListConfig` (no TOML file involved at the library boundary — that's FR-012).
3. Calls `a.send(&b.id(), Message::Ping(42)).await`.
4. Calls `await_delivery(&b, a.id(), &Message::Ping(42), Duration::from_secs(1)).await` and asserts `Ok(())`.

The helper code lives in `tests/common/mod.rs`.

## 3 — Run the CLI binary against a TOML peer-list (US3)

Create two TOML files (the schema is documented in `contracts/peer-list.toml.md`):

```sh
mkdir -p /tmp/pubsub-quickstart
cat > /tmp/pubsub-quickstart/node-a.peers.toml <<'EOF'
[[peers]]
id = "node-b"
EOF
cat > /tmp/pubsub-quickstart/node-b.peers.toml <<'EOF'
[[peers]]
id = "node-a"
EOF
```

In one terminal:

```sh
cargo run -- --self-id node-a --config /tmp/pubsub-quickstart/node-a.peers.toml
```

In another:

```sh
cargo run -- --self-id node-b --config /tmp/pubsub-quickstart/node-b.peers.toml
```

Each binary registers on a *separate* in-memory network (Single-process scope assumption — the two CLI processes do NOT exchange messages with each other; the InMemory network is process-local). The CLI exists at this stage to exercise the configuration-loading path (FR-001, US3 AS-1) and to validate the malformed-config error path (US3 AS-2) — *not* to provide a cross-process pubsub layer (that arrives with the first networked transport).

To verify error reporting (US3 AS-2):

```sh
echo '[[peers]
id = "node-b"' > /tmp/pubsub-quickstart/broken.toml
cargo run -- --self-id node-x --config /tmp/pubsub-quickstart/broken.toml
echo "exit code: $?"   # expect 2
```

Expected: a `pubsub-node: failed to parse TOML config …` message on stderr, exit code 2.

## 4 — Verify the 100-send N-intact property (SC-005)

```sh
cargo test --test two_node_ping -- ping_n_intact_across_100_sends --nocapture
```

The test loops 100 times sending `Ping(i)` for `i in 0..100u64`, awaits delivery of each in turn, and asserts that `node-b.received_messages()` contains the full sequence with `N` values preserved.

## 5 — Run the N-node graph test (US2)

```sh
cargo test --test n_node_graph
```

This builds the 4-node star (A connected to B, C, D) described in US2 AS-1 and verifies cross-cutting:
- Each addressed peer receives exactly its Ping.
- No non-addressed peer receives anything.
- A's outbound peer set is irrelevant to whether A *receives* inbound Pings from peers that list A (US2 AS-2).

## 6 — Where things live (mental map)

```text
pubsub-node/
├── src/
│   ├── peer.rs       # PeerId, PeerDescriptor trait, BasicPeerDescriptor
│   ├── message.rs    # Message::Ping(u64)
│   ├── network.rs    # Network trait, InMemoryNetwork, registry
│   ├── node.rs       # Node, receive task, received_messages()
│   ├── received.rs   # ReceivedDelivery
│   ├── config.rs     # PeerListConfig + load_peer_list (TOML)
│   ├── error.rs      # ConfigError, NetworkError, NodeError
│   ├── lib.rs        # Public re-exports
│   └── main.rs       # CLI (clap)
├── tests/
│   ├── two_node_ping.rs
│   ├── n_node_graph.rs
│   ├── config_loading.rs
│   └── common/mod.rs # await_delivery + fixture builders
└── docs/decisions/   # ADRs (per Constitution Principle III)
```

Detailed contracts:
- `contracts/library-api.md` — what the public Rust surface guarantees.
- `contracts/cli.md` — CLI flags, exit codes, error reporting.
- `contracts/peer-list.toml.md` — TOML schema.

Design context: `research.md` (the why behind each plan-level decision). Data shapes: `data-model.md`.

## 7 — Common pitfalls

| Symptom | Likely cause |
|---------|--------------|
| Test fails with `AwaitError::Timeout` after a `send().await` succeeded | Receive task not spawned during `Node::new` (regression on Research §6). Check `Node::new`'s body. |
| `cargo test` deadlocks | The await-on-delivery helper's polling interval can be set too coarse; default 1 ms is the recommended floor. |
| CLI exits 0 immediately | `tokio::signal::ctrl_c` hookup missing in `main.rs`. The binary should park on the signal future. |
| `tracing::warn!` on unknown-peer drop not visible | Default `--log-level info` — re-run with `--log-level warn` (or with the `RUST_LOG` env var). |
| `ConfigError::Parse` lacks line/column in output | `toml` crate too old; `Cargo.toml` requires `toml = "0.8"` or newer. |

## 8 — Budget check (SC-004)

If you got this far in under an hour, SC-004 holds. If you spent more, please leave a note on the PR that introduces the scaffold — the slow step is the signal the scaffold most needs to improve.
