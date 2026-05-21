# ADR 0005: Error model — thiserror in the library, no anyhow

**Status**: Accepted
**Date**: 2026-05-20
**Feature**: 001-minimal-node-scaffold
**Source**: `specs/001-minimal-node-scaffold/research.md` §5

## Context

Integration tests need to match on error variants (e.g., distinguishing
`ConfigError::Parse` from `ConfigError::InvalidPeer` per US3 AS-2's
sub-cases). The CLI binary needs to render an error chain on failure with
exit-code mapping per `contracts/cli.md`. The choice of error model
propagates into every `Result` signature in the crate.

## Decision

- Library code uses `thiserror`-derived enums: `ConfigError`,
  `NetworkError`, `NodeError`. `PeerIdError` lives in `src/peer.rs`
  co-located with `PeerId` (the `std::num::ParseIntError` pattern).
- The binary uses `Result<(), Box<dyn std::error::Error>>` and prints the
  source chain via `Display`.
- `anyhow` is NOT a dependency. The library never collapses variants into
  opaque strings.

## Consequences

- Callers can match on the specific failure cause; the CLI's exit-code
  mapping (`Io`/`Parse`/`InvalidPeer` → exit `2`, network/node startup
  failures → exit `1`) is a single pattern match.
- One fewer crate in the dependency tree.

## Alternatives considered

- **`anyhow` in the library**: collapses error variants into opaque
  strings, defeats US3 AS-2's per-cause assertions.
- **Hand-rolled error enums with manual `Display` / `Error` impls**: works
  but `thiserror` is the lowest-overhead expression of the same shape.
