# ADR 0002: Config parsing — serde + toml

**Status**: Accepted
**Date**: 2026-05-20
**Feature**: 001-minimal-node-scaffold
**Source**: `specs/001-minimal-node-scaffold/research.md` §2

## Context

FR-001 requires loading the peer set from a TOML file. US3 AS-2 requires that
a malformed config yield a clear, actionable startup error (see also
`contracts/cli.md` exit code `2`). The loader is the parse-at-the-edge
boundary that keeps the Node constructor filesystem-free (FR-012).

## Decision

Use `serde` (derive) plus the `toml` crate (v0.8+) for config parsing:

- `PeerListConfig` and `PeerEntry` derive `serde::Deserialize`.
- `PeerEntry` carries `#[serde(deny_unknown_fields)]` per
  `contracts/peer-list.toml.md` Forward-compatibility.
- `PeerListConfig` carries `#[serde(default)]` on `peers` so an empty list is
  representable.
- A `load_peer_list(path) -> Result<PeerListConfig, ConfigError>` function
  performs the three-stage pipeline (read → parse → re-validate each
  `PeerId`).

## Consequences

- Line/column-aware parse errors flow through `toml::de::Error` into
  `ConfigError::Parse`, satisfying US3 AS-2's actionability requirement.
- Schema extensions (`addr`, `pubkey`) become non-breaking field additions
  on `PeerEntry`.
- No new dependency to swap out later — `serde` is the universal Rust
  de/serialization framework.

## Alternatives considered

- **`toml_edit`**: supports round-tripping comments and formatting, which a
  read-only loader does not need.
- **Hand-rolled parser**: reinvents an audited dependency for no gain;
  line/column information would have to be hand-built.
