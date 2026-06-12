# ADR 0019: Graceful shutdown lifecycle (amends ADR 0012's teardown story)

**Status**: Accepted
**Date**: 2026-06-12
**Feature**: 004-connections
**Source**: `specs/004-connections/{spec,research}.md` (FR-020/021, R8); ADR 0011 (pure core), ADR 0012 (spawn-in-constructor + drop-abort).

## Context

ADR 0012 gave the node exactly one teardown path: `Drop` aborts the event loop and
every producer — adequate while teardown had no protocol obligations. 004-connections
introduces one: a gracefully departing node must send a `Terminated` notice per held
connection entry (both roles, any state) *before* its tasks die, which `Drop` cannot
do (synchronous, cannot await sends). The teardown story is structural: it touches the
event loop's lifetime, the public API, and every counterpart's state.

## Decision

### 1. A consuming, awaitable `shutdown` beside the unchanged abrupt `Drop`

`pub async fn shutdown(mut self)`:

1. push `Event::Shutdown` onto the node's own queue (events already queued drain
   first — orderly quiescence);
2. await the event loop's completion via `(&mut self.event_loop).await`
   (`JoinHandle` is `Unpin`; `Node` has a `Drop` impl, so the handle is awaited by
   reference, not moved out); a `JoinError` is logged and ignored;
3. let `self` drop — `Drop` runs as today and aborts the producers (aborting the
   already-finished loop is a no-op).

Consuming `self` makes use-after-shutdown unrepresentable. Plain `drop` without
calling `shutdown` remains the abrupt, no-notice path — deliberately without ordering
guarantees (ADR 0012's posture, unchanged).

### 2. Teardown is a state transition; the Shutdown event is the loop's terminal marker

`handle_shutdown` (pure, in `apply`) clears both connection structures and returns one
`Effect::Send { Terminated }` per held entry — teardown decisions stay synchronously
testable like every other transition. The event loop, after executing a `Shutdown`
event's effects, `break`s: loop termination *is* the completion signal `shutdown`
awaits, which guarantees the notices were handed to the network first. No oneshot
ack channel is needed.

**Carve-out, recorded**: the loop inspects the event kind (`matches!(event,
Event::Shutdown)`) to know when to break. This is loop *lifecycle*, not event
*semantics* — the handling of the event (clear + notices) still lives in `apply`;
the shell only decides to stop iterating. This is the single sanctioned exception to
"new event variants get their handling in `state::apply`, not here".

### 3. The executor gains the network send half

Effects now perform wire sends, so the loop task captures a clone of the handle's
crate-internal `NetworkSender` (exposed via a `pub(crate)` accessor on
`NetworkHandle`; the sender is `Clone` by the ADR 0007 actor shape). Effect execution
failures are logged only (spec FR-005); execution happens outside the state lock,
preserving the lock→apply→unlock→execute order ADR 0011/0012 established.

## Consequences

- Graceful shutdown guarantees: queued-events-first, notices-before-teardown,
  idempotent-by-construction (consuming `self`).
- `Drop`-only teardown keeps its meaning: abrupt, silent, stale entries on
  counterparts (spec-documented stale states; healed per the staleness catalog).
- The terminal-marker pattern generalizes: any future "stop the loop" semantics
  reuse the same shape rather than adding side channels.
- After `shutdown` begins, producers may still push briefly; events behind
  `Shutdown` in the queue are never processed (the loop broke) — acceptable: the
  node is gone, and `EventQueue::push` already treats a closed queue as a silent
  no-op for exactly this teardown window.

## Alternatives considered

- **Notices from `Drop`**: impossible — `Drop` is synchronous and cannot await the
  network; a fire-and-forget spawn from `Drop` loses the completion guarantee.
- **A oneshot ack carried in the Shutdown event**: rejected — the loop's natural
  termination already signals completion; an ack adds a channel for no information.
- **A `Shutdown` effect that stops the loop**: rejected — would make effect
  execution order load-bearing inside `apply`'s output; the shell-side break on the
  event kind is simpler and keeps `Effect` purely "external work".
- **Abort-ordering changes in `Drop`** (producers first): rejected — `abort()` does
  not wait, so no ordering provides a guarantee; the push-to-closed-queue contract
  already absorbs the race.

## Sources

- `specs/004-connections/spec.md` — FR-005, FR-020, FR-021, US4; Clarifications
  (notice scope incl. AwaitingAccept; shutdown wait mechanism discussion).
- `specs/004-connections/research.md` — R8; ADR 0007 / 0011 / 0012.
