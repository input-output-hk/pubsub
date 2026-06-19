# ADR 0022: Async test synchronization — barriers vs. bounded-negative checks

**Status**: Accepted
**Date**: 2026-06-19
**Feature**: Workstream-wide (test strategy); surfaced during 006-fanout-policy
**Source**: `.specify/memory/constitution.md` (Engineering Standard: *reproducible tests / no wall-clock assertions*); `tests/common` (`await_*`, `assert_no_new_deliveries`); 006-fanout-policy dissemination suite; PR #67 review.

## Context

A node is an async, single-consumer event loop: a producer pushes an `Event`, the loop drains it via `apply`, then executes the returned effects (including network sends to other nodes). Any state a test observes — a recorded delivery, a connection becoming `Active` — happens *after* the relevant event drains **and** its effects run, possibly several hops away. Tests therefore cannot read state immediately after acting; they must synchronize on the asynchronous outcome.

The constitution forbids wall-clock assertions, but a raw `tokio::time::sleep(…)` settle had crept in for two distinct needs, and they have different correct answers:

- **(P) Assert a positive outcome eventually happens** — e.g. a published message is recorded at every downstream.
- **(N) Assert a non-event** — e.g. an off-topic publish is *not* recorded, or a deduped copy is *not* re-recorded. The correct outcome is that nothing observable changes, so there is nothing to await.

A blanket `sleep` is flaky-prone (too short → false failure; too long → slow) and intention-opaque (the reader cannot tell P from N). This ADR records when to use which mechanism, and — importantly — what each can and cannot prove.

## Decision

**1. Positive outcomes (P) — synchronize on an observable happens-after, never on wall-clock.**
Await a real state change with the `await_*` poll helpers (`await_delivery`, `await_downstream`, `await_upstream_active`, …). To prove that a *no-op* event was processed, do **not** sleep — enqueue a **later real observable event** and await *it*: the single FIFO consumer guarantees the earlier event was already drained. (006-fanout-policy: the off-topic-publish test publishes the off-topic message first, then a valid one, and awaits the valid one's delivery — the valid publish is the barrier.)

**2. Genuine non-events (N) with no observable outcome — use a bounded-negative helper, not a raw sleep.**
When the event's correct result is to leave *no trace* (a deduped/dropped copy in a cyclic flood, where every "would-be" delivery is suppressed), use `tests/common::assert_no_new_deliveries(&[…nodes], window)`: it snapshots each node's record count and polls across `window`, **failing fast** the instant any count grows, and treating a clean window as quiescent. It reads as the negative assertion it is, and it lives in `tests/common` — **no test-only artifact is added to the node's production surface.**

**3. Never assert on the wall clock directly.** This is the constitution's *no-wall-clock* standard made concrete: the poll helpers' internal millisecond tick is an implementation detail of waiting on an observable condition, not an assertion about elapsed time.

## Consequences

**What a barrier (mechanism 1) fixes:** observing one real event proves an earlier event on the same FIFO path was fully processed — including its effects and the resulting downstream propagation — a true happens-after guarantee. This is the strongest tool and is preferred wherever the topology yields an observable downstream event.

**What a barrier cannot fix:**
- It orders events only along a **single** producer→consumer path. Across **independent** channels/hops it gives no guarantee: in a cyclic multi-hop mesh a later *directly-sent* message can overtake an earlier *relayed* copy, so a barrier cannot establish cyclic-flood quiescence. (This is why 006-fanout-policy's triangle test does **not** use a barrier.)

**What the bounded-negative check (mechanism 2) cannot fix:**
- It cannot prove "never" — a straggler arriving *after* `window` is missed. It is bounded confidence, not a guarantee. Therefore **the correctness of a no-event property must be owned by deterministic state-machine unit tests**, with the integration-level negative check serving only as a regression window. (006-fanout-policy: dedup exactly-once is proven by the `006-T010` state tests — `already_seen_…`, `republish_…`; the `006-T012` triangle's `assert_no_new_deliveries` window is the integration-level regression guard, not the proof.)

**Selection rule.** Positive outcome → `await_*` (or a real-event barrier for a processed no-op). No-trace non-event → `assert_no_new_deliveries`, backed by a deterministic unit test for the actual guarantee. Raw settle-`sleep`s are disallowed.

**Scope / migration.** 006-fanout-policy's dissemination suite follows this (zero settle-sleeps). Pre-existing suites still carry settle-sleeps; converting them is a deferred test-hygiene sweep (IMPLEMENTATION_NOTES N-026), not a refactor required by this ADR.
