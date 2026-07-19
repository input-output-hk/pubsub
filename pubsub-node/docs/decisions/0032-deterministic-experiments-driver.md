# ADR 0032: Deterministic experiments driver

**Status**: Accepted
**Date**: 2026-07-19
**Feature**: 016-experiments-framework
**Source**: `specs/016-experiments-framework/research.md` R1–R3; plan.md

## Context

The experiments framework is a measurement instrument: its coverage, depth,
miss-cause, and message-cost numbers must be right, and any run must be
exactly replayable from its recorded seed (016-FR-024). Measuring the real
node's behaviour by driving real async `Node`s over `InMemoryNetwork` cannot
provide that: task interleaving is not reproducible, and the readiness gate
makes the terminal topology timing-dependent. At the same time, the
instrument must measure *this* protocol, not an approximation of it.

## Decision

A synchronous **wavefront driver** (`experiments::driver`) over the crate's
real pure core:

- **Real cores, no shell.** The driver owns one `NodeState` per participant
  and calls the crate-internal `apply` directly — the same transition
  function, strategy seams, and message vocabulary the node runs. No tokio,
  channels, or `InMemoryNetwork` anywhere in the measurement path; fidelity
  is inherited, not approximated.
- **Round-based wavefront.** Round r is the set of in-flight deliveries;
  applying them yields the sends forming round r+1; a round producing no new
  sends is quiescence — detected exactly, no polling or timeouts. The wave
  index is the synchronous-round hop count the analytical models use, so
  depth is a topology property, directly comparable. Content-hash dedup +
  fire-once + a static topology make the receiver set interleaving-invariant,
  so the wavefront is a canonical order among equivalent ones.
- **Driver-owned canonicalisation.** Before routing, each wave is stably
  sorted by a canonical content key (sender, addressee, message identity
  bytes); all driver-side tally and extraction structures are ordered or
  index-keyed. Byte-determinism therefore never rests on the core's
  hash-based collection iteration order (that ordered-collection conversion
  is delegated to the in-flight connection-link work); the sort is
  O(W log W) per wave and makes any future core ordering change
  output-invariant for recorded experiments.
- **Participant model.** A population is a `BTreeMap<PeerId, Participant>`;
  each participant carries a class (honest / adversarial), a churn mark
  (`down`), its keys, and its node core. Adversaries are Level-1: the honest
  transition with a hostile strategy bundle. The driver routes all message
  kinds identically and never branches on class when delivering events or
  collecting sends — class appears only in tallies and denominators; `down`
  only gates stepping (a send to a down node is tallied sent-to-down and
  never enqueued). Severance effects (`Misbehaved`) are consumed and tallied.
- **Phase orchestration.** Strictly ordered phases per run: registration
  (faithful folds or direct pre-population; in faithful mode all registry
  folds land before any readiness event and all `Synced` events are injected
  as one wave, so every dial lands on a synced acceptor) → dial drain to
  quiescence (single-epoch: the nonce stays at genesis) → seeded churn draw
  (no events, no drain) → seeded publisher draw → publish drain ×
  publishes-per-run (fresh message each, no state reset).
- **Runs as pure functions.** A run performs no I/O and shares no state with
  other runs; all its randomness enters through pre-derived seeds. This is
  what makes run-granularity parallelism (a worker pool folding records in
  canonical run-index order) output-invariant at any worker count.

## Consequences

- Determinism tests can be value-level (run twice, compare observations);
  file-level byte diffs shrink to a couple of contract anchors.
- Depth/coverage numbers are directly comparable with the formal folder's
  round-based models — no scheduling noise term to explain.
- Realistic timing skew is out of scope by construction; if latency realism
  is ever needed, a discrete-event scheduler with per-edge delays is a
  driver-local extension that would refine latency metrics only (set-valued
  metrics are interleaving-invariant already).
- The driver holds every participant's state in memory; population size per
  in-flight run is bounded by memory, which is why the worker count doubles
  as the memory bound.

## Alternatives considered

- **Driving real async `Node`s on `InMemoryNetwork`**: non-reproducible
  interleaving; the readiness gate makes the terminal topology
  timing-dependent; wall-clock quiescence detection (sleeps) in the suite.
- **Seeded-random sequential dispatch**: destroys the round unit — depth
  becomes scheduling noise matching neither the models nor real time, while
  adding no realism to set-valued metrics.
- **Converting the core's connection collections to ordered forms here**:
  collides with the in-flight connection-link strategies PR that reshapes
  exactly those collections, and would rest the guarantee on an invariant
  owned by another module.
