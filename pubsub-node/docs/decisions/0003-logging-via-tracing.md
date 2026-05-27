# ADR 0003: Structured logging — tracing + tracing-subscriber

**Status**: Accepted
**Date**: 2026-05-20
**Feature**: 001-minimal-node-scaffold
**Source**: `specs/001-minimal-node-scaffold/research.md` §3

## Context

FR-010 mandates a *warn-level structured log entry that names the
unregistered identifier* when a send targets an unregistered peer.
Engineering Standards "Observable state transitions" also expects events
for register / send-accepted / recv-applied. Structured fields with named
values are required — string-formatted log lines do not satisfy FR-010.

## Decision

Emit logs via `tracing`; collect them with `tracing-subscriber`:

- The binary initialises a `tracing-subscriber` writing to stderr with an
  env-filter driven by the `--log-level` flag (default `info`, per
  `contracts/cli.md`).
- FR-010's drop event is emitted as
  `tracing::warn!(target = "pubsub_node::network", peer_id = %to,
   "send dropped: unregistered peer id")`.
- FR-006's queryable record is the *normative* observability surface;
  `tracing` events are supplementary.

## Consequences

- Logs are structured by default; downstream consumers can ingest JSON via
  `tracing-subscriber`'s `json` feature.
- The default log level (`info`) surfaces FR-010 warn events without
  explicit operator configuration, satisfying the constraint added to
  FR-012 during CHK040.
- Explicit operator overrides (`--log-level error`) may suppress FR-010
  output; this is operator choice, not an FR-010 violation (the requirement
  governs emission level, not delivery to a human reader — see CHK053).

## Alternatives considered

- **`log` + `env_logger`**: no structured fields without an adapter; cannot
  satisfy FR-010's named-field requirement directly.
- **`slog`**: structurally fine but its momentum has shifted to `tracing`
  since ~2022.
