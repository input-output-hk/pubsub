# ADR 0037: `serde_json` as an optional feature-tied dependency

**Status**: Accepted
**Date**: 2026-07-19
**Feature**: 016-experiments-framework
**Source**: `specs/016-experiments-framework/research.md` R4

## Context

The experiments framework (016) emits three output artifacts — a manifest,
a run-records JSONL stream, and an aggregates file — with a byte-identical
reproducibility requirement (016-FR-024, 016-SC-001): the same sweep
configuration and master seed must produce identical bytes at any worker
count. JSON encoding is therefore on the determinism-critical path: float
formatting and string escaping are exactly where byte-reproducibility bugs
live if hand-rolled.

The framework is entirely feature-gated (`experiments` cargo feature); the
Justified Dependencies engineering standard requires that a new dependency
be recorded with its justification and must not burden consumers who never
use it.

## Decision

Add `serde_json` (v1) as an **optional** dependency activated only by the
`experiments` cargo feature:

```toml
[features]
experiments = ["dep:serde_json"]

[dependencies]
serde_json = { version = "1", optional = true }
```

- Record and aggregate types derive `serde::Serialize` (the `serde` derive
  dependency already exists — ADR 0002) and contain only order-stable
  containers (`Vec`, `BTreeMap` — never a `HashMap` field), so encoding a
  value is a pure function of its content.
- The default build never compiles `serde_json`; the `experiments` binary
  carries `required-features = ["experiments"]` so no default target can
  reach it.

## Consequences

- Value-level determinism composed with deterministic encoding (`serde_json`
  uses ryu shortest-form float output, stable across platforms) yields
  byte-identical artifacts, so the file-level test surface shrinks to one
  or two byte-diff anchors; the bulk of determinism testing stays at value
  level.
- Consumers of the default library and node binary see no new dependency in
  their build graph.
- The order-stable-containers rule becomes a review obligation on every
  serialized record type: a `HashMap` field would silently break the
  byte-identity guarantee.

## Alternatives considered

- **Hand-rolled JSON writers**: relocates the byte-reproducibility risk into
  our own escaping/float-formatting code — the exact failure mode the
  decision avoids — while reimplementing an audited, universally-used
  encoder.
- **Non-optional dependency**: would compile `serde_json` for every user of
  the default build, violating the gating requirement that the framework be
  invisible when the feature is off (016-FR-001).
