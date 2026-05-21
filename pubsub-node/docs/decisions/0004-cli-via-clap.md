# ADR 0004: CLI parser — clap derive

**Status**: Accepted
**Date**: 2026-05-20
**Feature**: 001-minimal-node-scaffold
**Source**: `specs/001-minimal-node-scaffold/research.md` §4

## Context

FR-012 requires three CLI flags on the binary: `--self-id`, `--config`,
`--log-level`. SC-004 requires that a contributor be able to figure out
invocation from the binary itself (i.e., `--help` must be auto-generated and
sufficient). The CLI surface is the operator-facing entry point.

## Decision

Use `clap` v4 with the `derive` feature:

- An `Args` struct decorated with `#[derive(clap::Parser)]` describes
  `--self-id`, `--config`, and `--log-level`.
- Argument types do the validation work: `PeerId` (via `FromStr`),
  `PathBuf`, and `tracing::Level` respectively.
- `clap` produces `--help` / `--version` automatically.

## Consequences

- The CLI definition lives next to the struct holding the parsed values —
  no two-place edit when a flag is added.
- An invalid `--self-id` fails before the loader runs, satisfying FR-012's
  symmetric-validation requirement (CHK023).
- `clap`'s default behaviour exits with code `2` on usage errors, which is
  the POSIX convention `contracts/cli.md` documents.

## Alternatives considered

- **`argh`**: lighter but the ergonomics gap on a 3-flag CLI is invisible.
- **`pico-args` / hand-rolled**: saves a dep at the cost of every future
  flag costing ~10 lines.
