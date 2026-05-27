# ADR 0001: Async runtime — tokio

**Status**: Accepted
**Date**: 2026-05-20
**Feature**: 001-minimal-node-scaffold
**Source**: `specs/001-minimal-node-scaffold/research.md` §1

## Context

FR-011 mandates an async send/receive API. The scaffold's substrate, the test
harness, and every realistic future networked transport (TCP, QUIC, libp2p)
need a Rust async runtime. The choice is structural per Constitution
Principle III: it propagates into every `async fn` signature in the crate and
into `#[tokio::test]` attributes throughout the integration test suite.

## Decision

Use `tokio` as the async runtime:

- Multi-thread runtime by default (`#[tokio::main]` for the binary).
- `#[tokio::test]` for integration tests with the default flavour.
- Sync primitives (`mpsc`, `RwLock`, `Mutex`) and `time::timeout` taken from
  `tokio::sync` / `tokio::time` rather than from competing ecosystems.

Required features: `macros`, `rt-multi-thread`, `sync`, `time`, `signal`.

## Consequences

- The substrate is shape-compatible with future networked transports — every
  Cardano-adjacent Rust networking stack we might integrate against (sigp's
  Lighthouse, Parity's `sc_network`, libp2p-rust) is tokio-native.
- `#[tokio::test]` provides the integration-test ergonomics SC-001 depends on
  (whole-suite under 30 seconds with no test-runner gymnastics).
- The crate's MSRV is pinned at Rust 1.75 to use native `async fn` in traits
  without `async-trait` macro overhead.

## Alternatives considered

- **`async-std`**: viable but smaller ecosystem; would lock us out of
  tokio-only crates later.
- **`smol`**: lightweight but the saved binary size is irrelevant for a node
  binary.
- **No runtime / hand-rolled poll loop**: contradicts FR-011's intent (use
  the async ecosystem to surface integration-test patterns).
