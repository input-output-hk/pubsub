# Research: Subscription Registry (008)

**Date**: 2026-06-10 | **Plan**: [plan.md](./plan.md) | **Spec**: [spec.md](./spec.md)

All decisions were resolved before planning — through the post-merge review of PRs #44/#49/#50, the `/speckit-clarify` session (2026-06-10), and ADR 0013. No `NEEDS CLARIFICATION` markers remain in the spec. This file records the load-bearing choices in Decision / Rationale / Alternatives form.

## R1 — Two registries, separate types (no shared trait)

- **Decision**: This feature is the **subscription list** (`SubscriptionRegistry`), a distinct type from the future **topic registry** (`TopicRegistry`, ~007/012). No shared `Registry` trait.
- **Rationale**: `../docs/node-lifecycle/README.md` defines them as separate on-chain artifacts with different keys (node vs topic), payloads (interest+deposit vs authorised publishers), and readers (subscribers computing candidate sets vs relayers verifying signatures). A merged trait would be fat — half the methods nonsense for each side. The generic name `Registry` also collides with the topic registry.
- **Alternatives**: a bundled `Registry` trait (rejected — fat abstraction, name collision); see ADR 0013/0014.

## R2 — Subscription list is authoritative for a node's own interests, not config

- **Decision**: A node sources its topic-interest set from its own subscription-list entry via `interests_of(self_id)`; config carries identity + bootstrap only; 002's `subscribed_topics` is removed. Absent entry at startup → fail fast.
- **Rationale**: Config authority would let an operator make a node participate beyond its registered, deposited commitment — defeating the deposit's accountability. Fail-fast suits the in-memory mock (register/seed before constructing nodes); the protocol's retry-with-backoff is a 012 concern.
- **Alternatives**: config-authoritative; config-validated-against-chain; self-seed-from-config (all rejected — see ADR 0013).

## R3 — Read model is push/subscribe, not poll/diff

- **Decision**: `subscribe(topics) → SubscriptionWatch` replays current members as a `Joined` cold-start burst, then streams live deltas. The in-memory impl fans out a delta to subscriber channels on each write.
- **Rationale**: Matches how a chain follower exposes accepted state transitions, and reuses the `Network::register → handle{mpsc}` actor-handle idiom (ADR 0007) already in the crate. Polling reinvents change detection. The protocol's *authoritative* periodic chain re-read (`subscription_list_poll_interval`) is reconciliation that belongs to the on-chain reader (012); for the in-memory mock, push suffices.
- **Alternatives**: poll-and-diff loop (rejected — latency, redundant reads, impedance mismatch with the real backend); the deleted `docs/registry-node-contract.md` sketch's own-handle that bypassed the event queue (rejected — broke the agreed seam).

## R4 — Watch mirrors `NetworkHandle`; unbounded channel; no boundary marker

- **Decision**: `SubscriptionWatch` is single-consumer, not `Clone`, owns the receive half, ends on drop. Unbounded channel (ADR 0007). The cold-start burst has **no** explicit end-of-snapshot marker. The burst + live deltas are one gap-free, duplicate-free sequence (atomic snapshot + subscriber registration).
- **Rationale**: Direct reuse of the proven `NetworkHandle` shape. No v1 consumer needs a "warm" boundary signal (the node folds uniformly into a set); `SubscriptionEvent` is `#[non_exhaustive]`, so a `SnapshotComplete` variant can be added when feature 010's sampler needs it — honoring "no forward-compatible surface without a live consumer."
- **Alternatives**: a `SnapshotComplete` variant now (deferred); a bounded channel + `Lagged` repair (deferred to a real transport).

## R5 — Node integration on the merged 004 pure core; candidate set is state

- **Decision**: Add `Event::SubscriptionUpdate(SubscriptionEvent)` + a named `handle_subscription_update` handler in `apply`; fold deltas into `NodeState.candidates: HashMap<TopicId, HashSet<PeerId>>`, self-excluded; expose `Node::candidates(&TopicId) -> Vec<PeerId>`. A node-owned reader producer (via `spawn_producer`) drains the watch.
- **Rationale**: Feature 004 (PR #50) merged the pure `apply`/`NodeState`/`Effect` core (ADR 0011/0012); the candidate set is mutated by a transition, so it is state and belongs in `NodeState` — exactly the trigger N-007 named ("`peers` joins `NodeState` when a transition first consumes peer data; revisit at 008"). The handler returns an empty `Vec<Effect>` (`Effect` uninhabited; effects arrive with the dialer/connections).
- **Alternatives**: a side `TopicPeerView` outside `apply` (rejected — bypasses the pure core / the seam; was the deleted sketch's mistake).

## R6 — Candidate set coexists with the config bootstrap `peers`

- **Decision**: The registry-derived candidate set is distinct from the existing config `[[peers]]` field (the `Node` shell's bootstrap list). This feature adds the candidate set and does not touch `peers`; `Node::peers()` is unchanged.
- **Rationale**: `joining.md` connects to bootstrap nodes (step 4) **and separately** filters the subscription list into a candidate set (step 6) — two roles, two sources. Connecting/dialing from the candidate set is the dialer's job (~006 / `004-connections`), out of scope here.
- **Alternatives**: merge/replace `peers` with the candidate set (rejected — conflates bootstrap with interest-derived membership; would break the dialer's bootstrap contract).

## R7 — Node is strictly read-only; write API is for the file loader + tests

- **Decision**: `set_interest` / `unregister` exist on the trait but are called only by the `from_file` loader's equivalent and by test harnesses simulating operator churn; the node daemon issues no registry writes.
- **Rationale**: `joining.md`: "the node does NOT initiate a registration transaction; that is the operator's job." Read-only keeps the mock faithful and the node's role clean; it also resolves the earlier self-seed tension. Multi-node networks share one `Arc<dyn SubscriptionRegistry>` (as in-process nodes share one `InMemoryNetwork`); membership originates from the shared file / harness.
- **Alternatives**: node self-seeds on startup (rejected — circular, makes the node a writer, contradicts the read-only role).

## R8 — No new dependencies; subscription-list file is TOML at the edge

- **Decision**: `InMemorySubscriptionRegistry::from_file` parses a TOML subscription-list file (`[[entry]]` with `node_id`, `topics`; strict unknown-field rejection per 001) into the initial membership; `new()` builds an empty registry. `serde`/`toml` are already direct deps (ADR 0002); `tokio` unbounded mpsc backs the watch (ADR 0001/0007).
- **Rationale**: Parse-at-the-edge — the file/IO/deserialization lives in the loader; the trait and `NodeState` take decoded values. No dependency requiring a new ADR.
- **Alternatives**: a bespoke file format (rejected — TOML matches the existing config convention); JSON/CBOR (unnecessary for a local mock).
