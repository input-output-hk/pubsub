# ADR 0015: The node has no local subscription mutators; its subscription set is registry-derived

**Status**: Accepted
**Date**: 2026-06-11
**Feature**: 008-node-registry
**Source**: ADR 0013 (subscription list is authoritative), ADR 0014 (registry interface + node integration); supersedes the runtime-mutator surface of ADR 0008 and the sync-mutator retention of ADR 0012 §2; `specs/008-node-registry/spec.md` FR-016/FR-018.

## Context

Feature 002 gave the node a runtime API to mutate its own subscription set — `Node::subscribe(topic)` / `Node::unsubscribe(topic)` returning `SubscribeOutcome` / `UnsubscribeOutcome` (ADR 0008), kept as synchronous lock-takers when 004 unified the state behind one mutex (ADR 0012 §2). At that time the subscription set was the node's own locally-owned state: config seeded it (`subscribed_topics`) and the API edited it.

Feature 008 changed what that set *is*. ADR 0013 made the **subscription list the single source of truth** for a node's topics (config cannot define them — otherwise an operator could make the node participate beyond its registered, deposited commitment). ADR 0014 then had the node **derive** its subscription set by folding the head `Joined { self_id, .. }` of its `watch(self_id)` stream: `handle_membership_update` writes `NodeState.subscriptions` on every own-id event.

That left **two writers to one field**. `NodeState.subscriptions` — the set that gates message acceptance (`handle_signed_message`) — was written both by the registry fold (authoritative, from the stream) and by the local `subscribe`/`unsubscribe` mutators (imperative, node-local, bypassing the event loop under the mutex). The mutators are the **same accountability hole ADR 0013 closed for config, through a different door**: a caller could `subscribe` the node to a topic it is not registered for, and a subsequent self-event from the registry (`TopicsChanged { self }` / `Left { self }`) would silently clobber it — a split-brain accept-filter. `plan.md` §8 had deferred the question ("subscribe/unsubscribe stay sync, unchanged by this feature; runtime own-topic changes out of scope"), but deferral left the contradictory writer live.

This is structural per Principle III: it removes public API (`Node::subscribe`/`unsubscribe`, `SubscribeOutcome`/`UnsubscribeOutcome`) and reverses prior ADRs.

## Decision

**The node has no API to mutate its own subscription set. The set is derived solely from the subscription registry (the node's own entry, via the `watch` stream).**

Concretely:

1. **`Node::subscribe` / `Node::unsubscribe` are removed**, along with `SubscribeOutcome` / `UnsubscribeOutcome` and the `NodeState::subscribe` / `unsubscribe` methods. `NodeState.subscriptions` is now written **only** by `handle_membership_update` (the own-id branch). The read-only `Node::subscriptions()` snapshot getter stays.
2. **Runtime topic changes are operator/registry actions**, not node actions: an operator edits the subscription list (in the mock, `SubscriptionRegistryControl::set_topics`; in production, an on-chain transaction), and the change flows back to the node as a `TopicsChanged { self }` / `Left { self }` on its `watch` stream. There is no node-initiated path, now or at 012.
3. **The write surface stays where it belongs** — `SubscriptionRegistryControl::set_topics` / `unregister`, used by the file loader, test harnesses, and operator stand-ins (never the node). Tests drive runtime subscription changes through it (no fixture file required).
4. **Runtime *narrowing* works today; *expansion* is deferred to 012.** The `watch(self_id)` stream is scoped to the node's topics at watch time (ADR 0014), so a self-`TopicsChanged` that *removes* a watched topic is delivered and folded, but one that *adds* a topic outside the original scope is not. Dynamic re-scoping (re-opening/widening the watch on own-entry growth) is feature 012's concern.

## Consequences

- The split-brain accept-filter is eliminated: there is one writer to `subscriptions`, and it is the authoritative registry stream. The source-of-truth invariant (ADR 0013, SC-007) now holds at runtime, not only at startup.
- **Public API shrinks** (breaking): `Node::subscribe`/`unsubscribe` and the two outcome enums are gone from `lib.rs`. No production caller existed (only tests). The 002-era `tests/topic_runtime.rs` is rewritten — its runtime-mutation scenarios that relied on the local mutators are replaced by one registry-driven *narrowing* test (`set_topics` → `watch` → fold → accept-filter converges); the orthogonal initial-filter and decoupled-emission tests are retained.
- The `state.rs` pure-core test for mutator outcomes is removed; the "subscription change affects subsequent transitions" test is rewritten to drive the change via `Event::MembershipUpdate(Joined { self, .. })` (the canonical path) instead of a local mutator.
- Clean 012 story: runtime own-entry changes (including expansion / re-scoping) are designed there as registry/stream behaviour, with no node-side mutator to reconcile.

## Alternatives considered

- **Keep the mutators, make them non-authoritative / guarded** (e.g. no-op against the registry-derived set, or test-only): rejected — leaves dead or confusing surface and still invites the divergence; the registry write-domain already gives tests the control they need.
- **Keep the mutators and have them write the registry** (node-initiated `set_topics`): rejected — makes the node a registry writer, contradicting the strictly-read-only role (ADR 0013); registration is an operator action.
- **Defer removal to 012**: rejected — the contradictory second writer is a live correctness hazard now, not unfinished polish; closing it is cheap and local.

## Sources

- ADR 0008 (002 subscription-mutator shape — runtime surface superseded here), ADR 0012 §2 (sync-mutator retention — superseded here), ADR 0013 (subscription list authoritative), ADR 0014 (node-keyed `watch`, fold writes `subscriptions`).
- `specs/008-node-registry/spec.md` — FR-016 (self-handling in the fold), FR-018 (topics sourced from the registry, node strictly read-only).
- `src/state.rs` `handle_membership_update` (sole writer of `subscriptions`); `src/node.rs` (`subscriptions()` getter retained); `tests/topic_runtime.rs` (registry-driven runtime behaviour).
