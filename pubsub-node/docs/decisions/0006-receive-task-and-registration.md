# ADR 0006: Receive-task model and registration timing

**Status**: Accepted
**Date**: 2026-05-20
**Feature**: 001-minimal-node-scaffold
**Source**: `specs/001-minimal-node-scaffold/research.md` §6 + §8

## Context

FR-013 decouples `send().await` resolution from recipient observability —
the recipient processes deliveries into its `received_messages()` record
*subsequently*. Something must drive that processing on each node, and
that something must be alive by the time `Node::new` returns or the
acceptance scenarios race against startup.

These two questions — *who drives recv?* and *when does registration
complete?* — are conjoined: the recv driver cannot be started before the
node is registered with the network, and registration is meaningless
without a receiver to drain the mailbox.

## Decision

- **Receive driver**: each `Node` spawns a background `tokio::task` during
  `Node::new`. The task owns the receiver moved out of `NetworkHandle` via
  `take_receiver()` (see ADR 0007), loops `rx.recv().await`, and appends
  each delivered envelope into the node's `ReceivedDelivery` record under
  a `Mutex`.
- **Registration timing**: `Network::register(id)` is invoked by `Node::new`
  during construction. `Node::new` does not return until registration is
  complete *and* the recv task is spawned. The `JoinHandle` is retained on
  the `Node` and aborted in `impl Drop` for clean shutdown.

## Consequences

- Callers (tests, the CLI) cannot accidentally send before the recv loop is
  alive — the constructor's resolution is the readiness signal.
- The `Node` owns a complete lifecycle (register → spawn → run → abort on
  drop), no `start()` / `attach()` ceremony at every call site.
- `Drop` semantics are deliberately a plan-level concern, not a spec
  requirement (CHK042 resolution): when failure handling arrives, an
  explicit shutdown API may join this ADR.

## Alternatives considered

- **Caller-driven `node.poll()` loop**: shifts complexity to every test.
- **Synchronous delivery inside `Network::send`** (mutating the receiver's
  record directly): contradicts FR-013.
- **Lazy spawn on first send/recv**: opaque lifecycle; races with the
  `await_delivery` helper.
- **Two-step `Node::new(...)` + `node.attach(network).await`**: extra
  ceremony for every test.
