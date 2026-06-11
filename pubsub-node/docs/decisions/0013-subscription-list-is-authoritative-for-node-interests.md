# ADR 0013: The subscription list is the authoritative source of a node's topic interests

**Status**: Accepted
**Date**: 2026-06-10
**Feature**: 008-node-registry
**Source**: `specs/008-node-registry/spec.md` (Clarifications 2026-06-10, FR-008/FR-018); `../docs/node-lifecycle/joining.md`; `../docs/node-lifecycle/README.md`

## Context

Feature 008 is the **subscription list** — the per-node membership artifact (node pubkey → topic-interest set → deposit) that subscribers read to compute candidate sets — as distinguished by `../docs/node-lifecycle/README.md` from the **topic registry** (topic id → authorised publisher keys). The two are separate artifacts with different keys, payloads, and readers; this feature models the subscription list only.

A node needs to know *its own* topic-interest set to (a) filter inbound messages (feature 002) and (b) decide which topics to watch for candidate building (008). The question is where that set comes from. `../docs/node-lifecycle/joining.md` is **ambiguous**: it places the topic-interest set in *both* the on-chain subscription transaction (operator step 3; node startup steps 2–3 read it back) *and* the node config file (operator step 4), and step 6's "filter the subscription list by the node's own topic interests" does not say which of the two is authoritative at runtime.

If **config** is authoritative, a node operator can make the node participate in (build candidate sets for, later open dissemination links on) topics **beyond what it registered and deposited for** on-chain. That defeats the subscription-list deposit's purpose: a node's participation must be bounded by its on-chain commitment, or the bond buys no accountability.

This is structural per Principle III: the source-of-truth choice shapes the `SubscriptionRegistry` read surface (it needs a self-lookup), the node's config schema (whether it carries topics), and the 002 message-filter's input — reversing it later touches all three.

## Decision

**The subscription-list entry is the single authoritative source of a node's own topic interests.** A node derives its effective interest set from its own entry, not from a locally-editable config topic list.

Concretely:

1. **`SubscriptionRegistry` exposes a self-lookup**: `interests_of(node) -> Option<BTreeSet<TopicId>>`. The node calls `interests_of(self_id)` at startup to learn its authoritative interests *before* it knows which topics to `subscribe` to for candidate building.
2. **The sourced set seeds the (retained) 002 message-filter** and determines the node's `subscribe` topics. There is one interest set per node, and it comes from the registry.
3. **The 002 `subscribed_topics` config field is removed.** Node config carries **identity (`node_id`) + bootstrap peers** only — never an authoritative topic list.
4. **The node is strictly read-only toward the registry** — no `set_interest`, no `unregister`, no self-seed. Registration / change / leave are operator actions (an on-chain transaction in production; entries in the subscription-list file and/or test-harness `set_interest` calls in the in-memory mock).
5. **In the mock, the subscription-list file is the source of truth.** `InMemorySubscriptionRegistry::from_file(path)` loads `node_id → topics` entries; this stands in for the on-chain list. The same `interests_of` / `subscribe` path swaps to a chain reader at feature 012 with no reshaping.

## Consequences

- The source-of-truth invariant holds in both worlds: a node configured as `S` against an entry `S → {T}` acts only on `{T}`, regardless of any other configured value (spec SC-007). The deposit's accountability is preserved — participation cannot exceed the registered commitment.
- 002's message filter is now seeded from the registry self-lookup rather than the removed `subscribed_topics`; the filter mechanism itself is unchanged.
- The `SubscriptionRegistry` trait gains `interests_of` alongside `subscribe`; the node performs exactly one extra read at startup and zero writes thereafter.
- Clean 012 swap: `from_file` → chain reader; `interests_of(self)` → on-chain own-entry lookup; the node and 002 are untouched.
- The upstream `joining.md` ambiguity should be resolved in the protocol doc to state on-chain authority explicitly. Per Principle V this ADR does not edit the protocol docs; the fix is proposed as a follow-up docs PR (and was surfaced for the doc owner). Until then, this ADR records the pubsub-node interpretation.
- Startup behaviour when a node's id has **no** subscription-list entry is left open (spec edge case / `/speckit-clarify`): error, wait-and-retry (faithful to `joining.md` step 3), or run with empty interests.

> **Superseded mechanism (see [ADR 0014](0014-subscription-registry-interface-and-node-integration.md)).** This ADR's *decision* — the subscription list is the single authoritative source of a node's topics, not config — **stands unchanged**. Its illustrative *mechanism* did not survive ADR 0014: the self-lookup method sketched here as `interests_of(node)` (later `entry(node)`) was **removed entirely**, and the read trait collapsed to a single node-keyed `watch(node)`. The node now derives its own topics by folding the **head `Joined { self_id, .. }`** of its `watch(self_id)` stream — no separate point-read — and starts from an empty subscription set, converging as the cold-start burst drains. Consequently the open startup-behaviour question above was resolved to **no fail-fast**: a node with no entry constructs cleanly and stays at empty derived state (ADR 0014; spec FR-018). Read `interests_of`/`entry`/"self-lookup" in the Decision, Consequences, and Sources below as "the own-entry head event of the node's watch stream"; the `from_file` → 012 chain-reader swap is likewise via `watch`, not a point-read.

## Alternatives considered

- **Config is authoritative** (the draft 008 spec's first cut): rejected — leaves a config/chain divergence hole open in everything built until 012, and lets an operator widen a node's participation beyond its on-chain commitment, nullifying the deposit's accountability.
- **Config working-set validated against chain** (node uses config topics but errors if they exceed the on-chain entry): rejected — two sources of truth with a reconciliation rule, where one source (the chain) can simply *be* the truth; more surface for no benefit.
- **Node self-seeds its registration from config on startup** (the prior mock affordance): rejected — circular (the node writes what it then reads back, so registry ≡ config trivially), so it does not actually enforce the invariant, and it makes the node a registry writer, contradicting the read-only protocol role.
- **Keep `subscribed_topics` and treat the registry as advisory**: rejected — same divergence hole; the candidate set and the message filter could disagree with the on-chain entry.

## Sources

- `../docs/node-lifecycle/README.md` — on-chain artifacts table (topic registry vs subscription list; "subscribers compute candidate sets"; endpoints off-chain).
- `../docs/node-lifecycle/joining.md` — operator pre-conditions (steps 3–4) and node startup (steps 2–3, 6); the config-vs-chain authority ambiguity this ADR resolves for pubsub-node and flags upstream.
- `specs/008-node-registry/spec.md` — FR-004 (subscription-list file), FR-008 (`interests_of`), FR-018 (interest sourcing + read-only + `subscribed_topics` removal), SC-007.
- `specs/event-loop-and-registry-contract.md` — the seam this feature consumes.
- ADR 0011 / 0012 — the feature 004 pure-core and shell decisions the node integration builds on.
