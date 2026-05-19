# CLI Contract — `pubsub-node` binary

**Feature**: 001-minimal-node-scaffold
**Source of truth**: `src/main.rs`
**Spec trace**: FR-001 (TOML config loading), FR-012 (CLI parses + parses-at-edge, then constructs Node), US3 (config-file path is the operator entry point)

## Invocation

```text
pubsub-node --self-id <ID> --config <PATH> [--log-level <LEVEL>]
```

| Flag | Required | Type | Description |
|------|----------|------|-------------|
| `--self-id <ID>` | yes | string (PeerId) | This node's own identifier. Non-empty UTF-8, no internal NULs. Maps to `Node::new(self_id, …)`. |
| `--config <PATH>` | yes | path | Filesystem path to a TOML file matching `contracts/peer-list.toml.md`. Read at startup; never re-read while the process runs (FR-008). |
| `--log-level <LEVEL>` | no | enum | `trace` \| `debug` \| `info` (default) \| `warn` \| `error`. Sets the `tracing-subscriber` env filter. |

`--help` and `--version` are provided automatically by `clap`. SC-004 contributor flow expects `pubsub-node --help` to print enough to reproduce the demo.

## Exit codes

| Code | Meaning | Spec trace | When |
|------|---------|-----------|------|
| 0    | Clean exit on signal | None (graceful-shutdown behaviour; not a spec scenario in v1) | Ctrl-C / SIGINT handler fires; recv task aborted; Node dropped |
| 1    | Runtime failure | None (catch-all for non-config failures) | The recv task or signal handler errored; non-config registration failure (e.g., `NetworkError::DuplicateRegistration` per FR-009) |
| 2    | Configuration / identifier error | US3 AS-2 (malformed config); FR-012 (`--self-id` validation failure, symmetry added in CHK023) | `ConfigError::Io / Parse / InvalidPeer` from the loader; OR `PeerIdError` from parsing `--self-id` at CLI entry |
| 64   | Usage error | None (CLI parsing convention from `<sysexits.h>`) | `clap` rejected the arguments (e.g., missing required flag, invalid value type) |

Exit codes 2 and 64 follow `<sysexits.h>` conventions (`EX_DATAERR` / `EX_USAGE`). Operator scripts can distinguish "operator-error" (2, 64) from "node-runtime error" (1). The "Spec trace" column annotates which codes correspond to spec-level scenarios vs which are POSIX-convention best-practice exits.

## Error reporting

When the binary exits non-zero, it prints a single human-readable line to **stderr** followed by the `Display` chain of the underlying error. Format:

```text
pubsub-node: <short cause>
  caused by: <next link>
  caused by: <next link>
```

For US3 AS-2 ("malformed config, clear actionable error"), the first line is the file-level message and subsequent lines reveal the line/column information surfaced by `toml::de::Error`.

Example:

```text
$ pubsub-node --self-id node-a --config peers.toml
pubsub-node: failed to parse TOML config "peers.toml"
  caused by: TOML parse error at line 3, column 5
   |
 3 | id "node-b"
   |     ^
expected = after key
```

## Lifecycle

1. `clap` parses args (exit 64 on failure).
2. `tracing-subscriber` is initialised at the requested level.
3. `config::load_peer_list(--config)` is called. Failure → exit 2.
4. An `Arc<InMemoryNetwork>` is constructed.
5. `Node::new(self_id, parsed_config, network)` is awaited. Failure → exit 1.
6. The binary blocks on a Ctrl-C / SIGINT handler (`tokio::signal::ctrl_c`). On signal, the Node is dropped (which aborts its recv task), the binary logs a shutdown event, and exits 0.

The CLI does **not** offer a "send a Ping" subcommand at this stage — Ping origination is exercised exclusively via integration tests in `tests/`. Adding an interactive or scripted send path is a future iteration (and a candidate for `/speckit-tasks` to enumerate explicitly *not* to implement here).

## Out of scope for v1

- Re-reading config on SIGHUP (FR-008 forbids peer-set mutation after startup).
- Multiple binaries / multi-process orchestration (Single-process scope, spec Assumptions).
- TLS / mTLS / any cryptographic flag (FR-007).
- Metrics endpoint / Prometheus / OpenTelemetry exporter (Engineering Standards mentions structured logs only at this stage).
