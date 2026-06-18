# Research: Cross-Registry Consistency Invariant + Declarative Topic Entry (014)

**Date**: 2026-06-15 | **Plan**: [plan.md](./plan.md)

Consolidated design decisions. Each: Decision / Rationale / Alternatives considered. Structural rationale lands in ADR 0020 (amends 0016). All five spec clarifications (2026-06-15) are inputs.

## D1 — Maintained single subscription set + strict drop (vs read-time intersection)

**Decision**: `NodeState` keeps a single `subscriptions: HashSet<TopicId>` that is, by construction, always a subset of `registered_topics.keys()`. `handle_membership_update`, when folding the node's **own** entry, admits a topic only if it is currently registered; an unregistered topic is **dropped** (not added, not buffered) and logged (`cause = "topic_not_registered"`). There is no auto-promotion: a topic registered after a dropped subscription requires a fresh membership event. `subscriptions_snapshot()` returns the set directly (no intersection). 013's `subscriptions ∩ registered_topics` read-time intersection (013 ADR 0016 note 4) is removed.

**Rationale**: the maintainer decision (PR #55 review, 2026-06-15) — the node enforces `subscriptions ⊆ registered` on its own state by **dropping** violating events, relying on the chain follower's event ordering as the source of consistency. A single maintained set is the simplest representation that makes the invariant true at rest (not only at read), gives 004 a subscription set it can trust without re-checking registration, and removes the masking that hid inconsistency.

**Alternatives considered**: (a) **Keep declared intent + promote on registration** (the "hold" model) — preserves 013 SC-004 and tolerates unordered streams, but keeps a second buffer and a dynamic the team explicitly chose to drop; rejected per the meeting. (b) **Keep 013's read-time intersection** — no maintained invariant; rejected, it is exactly what this feature replaces. (c) **Strict drop without a readiness gate** — correct in production but flaky at mock cold start (D5).

## D2 — Symmetric candidate gating

**Decision**: `handle_membership_update`, folding **another** node's entry, records a `(peer, topic)` candidate only if `topic` is currently registered; an unregistered topic is dropped + logged. The invariant `candidates.keys() ⊆ registered_topics.keys()` holds alongside the subscription invariant.

**Rationale**: clarify 2026-06-15 (Q2). By the same chain-order premise, other nodes also subscribe only to registered topics, so a candidate on an unregistered topic is as anomalous as a self-subscription on one. Symmetry removes special-casing and hands 004's dialing a candidate set containing only registered topics. The readiness gate (D5) orders registry-before-membership, so candidate gating is correct at cold start too.

**Alternatives considered**: gate only the node's own subscriptions, leaving candidates ungated (cleared only by the cascade) — an asymmetry where `candidates` can transiently name unregistered topics; rejected for inconsistency with the subscription side and a messier hand-off to 004.

## D3 — Defensive topic-registry fold (create-only-on-`Registered`)

**Decision**: `handle_topic_registry_update` becomes defensive: only `Registered` creates a topic in the projection; a `PublishersChanged` for a topic with no current entry is **dropped + logged** (no `or_default` auto-create); a `Removed` for an unknown topic is a no-op. `Registered` (create/replace), `PublishersChanged` on a registered topic (apply add/remove diff), and `Removed` on a registered topic (delete + cascade, D4) behave as in 013.

**Rationale**: clarify 2026-06-15 (Q1). Makes "validate, don't assume" uniform across both streams; an `or_default`-created topic would be a phantom registration that no `Registered` ever authorized. Amends 013 FR-013 / the lenient fold; the 013 `or_default` fold test is reworked.

**Alternatives considered**: keep 013's lenient `or_default` (the registry stream is the source of truth, don't second-guess it) — simpler and FR-008-as-013, but the maintainer chose the defensive posture for uniformity; rejected.

## D4 — Atomic cascade on `Removed`

**Decision**: a topic-registry `Removed { topic }` fold removes `topic` from `subscriptions`, from `candidates`, and from `registered_topics` within the single `apply` call. No intermediate state where some-but-not-all structures dropped it is observable.

**Rationale**: spec FR-002; maintainer "delete a topic and its subscriptions atomically/sequentially." `apply` runs synchronously under the state lock (ADR 0012), so a single-fold multi-structure update **is** atomic with respect to any observer (getters take the same lock) — atomicity is by construction, no extra mechanism. Keeps `candidates`/`subscriptions ⊆ registered` true the instant the topic leaves the projection.

**Alternatives considered**: rely on a separate membership `Left`/`TopicsChanged` event from the chain to remove the subscription (no node-side cascade) — leaves an inconsistent window between the topic removal and the membership event, the exact intermediate state the feature forbids; rejected.

## D5 — `SnapshotComplete` readiness marker + drain-then-spawn ordering

**Decision**: add `TopicRegistryEvent::SnapshotComplete` (additive, `#[non_exhaustive]`). `InMemoryTopicRegistry::watch()` emits it once, **after** the cold-start `Registered` burst, before any live delta. `Node::new` opens the topic watch and **drains + folds events up to `SnapshotComplete`** (seeding `registered_topics`) **before** spawning the membership reader; it then spawns the topic-reader producer to drain live deltas from the same watch. The membership reader therefore never folds an event before the registered set is warm, so strict drop (D1) and candidate gating (D2) are correct at cold start. `handle_topic_registry_update` treats `SnapshotComplete` as a no-op (idempotent; supports a future re-sync).

> **Superseded 2026-06-17 (b):** the drain-then-spawn / two-reader-+-oneshot mechanism (here and as built in Session 2) is replaced by a single `registry_indexer_loop` that drains the topic burst then the membership burst in one reader (ordering intrinsic, no oneshot) and pushes one `Event::ConnectionSetup`. The `SnapshotComplete` markers become reader-consumed delimiters (both fold arms no-ops). See ADR 0020 (Amendment 2026-06-17 (b)) and spec Clarifications Session 2026-06-17 (b). **Further superseded 2026-06-18 (snapshot reshape):** `watch()` now returns `(snapshot, live)`, both `SnapshotComplete` variants are removed (no burst to delimit), and the single readiness signal is `Event::Synced` (driving a `Syncing → Synced` lifecycle). See ADR 0020 (Amendment 2026-06-18).

**Rationale**: strict drop evaluates each subscription against the *current* registered set; without ordering, the mock's two independent watches race and a subscription folded before its topic's registration is wrongly dropped with no recovery (D1 has no promotion). A stream marker (i) stays within 013's **watch-only** model — no point-read, honoring 013 FR-001; (ii) **generalizes to the 012 on-chain reader**, whose initial chain-sync is asynchronous and cannot guarantee the burst is queued before `watch()` returns; (iii) is the minimal slice of the "registry synchronization complete" event the team deferred, un-deferred only as far as strict drop requires. In production the chain follower supplies this ordering; the marker makes the mock faithful to it.

**Alternatives considered**: (a) **Synchronous drain-until-empty in `Node::new`** (no marker) — relies on `InMemoryTopicRegistry` queuing the whole burst before `watch()` returns; true today but a **mock-only timing assumption** that breaks for 012's async snapshot; rejected as not forward-compatible (Constitution: shape for the named 012 consumer). (b) **A point-read `snapshot()` on the `TopicRegistry` trait** — reintroduces the point-read 013 FR-001 deliberately excluded, and double-counts against the watch burst; rejected. (c) **A non-blocking gate** (spawn both readers; the membership reader awaits a shared "topic ready" flag before applying) — avoids blocking `Node::new` but adds intra-node coordination; the drain-then-spawn ordering is simpler and `Node::new` is already async, so blocking on immediate mock readiness is acceptable (ADR 0020 notes the blocking semantics; a broken/empty registry signals readiness immediately after zero `Registered`s).

## D6 — `TopicEntry` declarative type

**Decision**: introduce `TopicEntry` wrapping the authorized-publisher set, with `is_open(&self) -> bool` (the set is empty) and `is_publisher_authorized(&self, key: &PublicKey) -> bool` (`is_open() || publishers.contains(key)`), plus an internal `apply_publishers_diff(added, removed)` used by the `PublishersChanged` fold. The receive path (`handle_signed_message`) calls `entry.is_publisher_authorized(key)` instead of the inline `authorized.is_empty() || authorized.contains(key)`. It is the extension point for owners/admins (012) without reshaping call sites.

**Rationale**: maintainer request — a dedicated, idiomatic structure for publishers, more expressive than a bare set; the inline open-topic check was hard to recognize (took a reviewer minutes). Behaviour-preserving (the predicates encode exactly today's rule).

**Alternatives considered**: keep the bare `BTreeSet<PublicKey>` with a free helper function — less discoverable, no natural home for future fields; rejected per the team decision.

## D7 — `TopicEntry` placement and visibility

**Decision**: `TopicEntry` is `pub(crate)`, living in the topic-registry module (`src/topic_registry/topic_entry.rs`) and used as the value type of `NodeState.registered_topics: HashMap<TopicId, TopicEntry>`. The **public** `TopicRegistryEvent` keeps carrying `BTreeSet<PublicKey>`; the fold constructs a `TopicEntry` from the event's set. No public type changes (beyond `SnapshotComplete`).

**Rationale**: `TopicEntry` is the node's internal projection representation, not part of the registry's wire/event contract; keeping it crate-internal avoids leaking a node-projection type through the `TopicRegistry` interface and leaves the 012 reader unaffected. Placing it in `topic_registry/` (not `state.rs`) keeps the topic-domain types together and lets `from_file`/`watch` build it directly if useful later.

**Alternatives considered**: (a) make `TopicEntry` public and have `TopicRegistryEvent` carry it — couples the event contract to the node's projection shape; rejected (parse-at-the-edge / anti-corruption). (b) define it in `state.rs` — fine, but the topic-registry module is the cohesive home for topic-domain types; minor, tactical.

## D8 — Remove 013 SC-004 (subscribe-before-register dynamic)

**Decision**: 013 SC-004 ("a subscription-list topic unregistered at startup but registered later becomes effective without restart") is **removed**. Under strict drop + the readiness gate + chain ordering, a subscription only ever arrives after its topic's registration; the subscribe-before-register case is an anomaly that is dropped + logged, not a supported dynamic. The 013 test asserting SC-004 is reworked into strict-drop + readiness coverage.

**Rationale**: SC-004 existed because 013 folded two unordered streams with no barrier. 014 adopts the chain-order premise and the readiness gate, which makes that scenario off-nominal. Keeping SC-004 would contradict strict drop.

**Alternatives considered**: preserve SC-004 via the hold/promote model (D1 alt a) — rejected with D1.

## D9 — Connection cascade ~~deferred to the 004 rebase~~ → **in scope (superseded 2026-06-17)**

**Original decision (pre-rebase)**: no connection-state cascade here, since `NodeState` on the pre-004 `main` had no `upstream`/`downstream` fields.

**Superseded (2026-06-17 rebase, ADR 0020 amendment)**: 004 merged to `main` and 014 rebased onto it, so the connection fields exist and the cascade IS in scope: a `Removed` now clears `upstream`/`downstream` too (FR-002/FR-010), and S7/N-015 is resolved (rejection of requests on unregistered topics, via strict drop). The dial trigger (`MembershipEvent::SnapshotComplete`) and the removal of `connection_setup_delay` also landed here.

**Rationale**: spec FR-010; the fields literally do not exist on this branch's base. Establishing the invariant now is the resolution N-015 named; the connection wiring is 004's to carry on rebase.

**Alternatives considered**: pull a placeholder connection cascade in now — impossible without 004's fields and out of scope; rejected.

## Cross-cutting

- **Testing** (Constitution II, critical): state-machine tests for both invariants, strict drop (self + candidates), defensive fold, atomic cascade, and the 013 no-regression matrix precede the `state.rs` changes; `TopicEntry` unit tests precede its wiring; a cold-start convergence integration test precedes the `Node::new` reorder. The two subset invariants are `proptest` candidates over random interleavings.
- **Declarative test construction** (v1.2.0): reuse `TopicRegistryScript` (gains `snapshot_complete()`) and `MembershipScript`; no inline literals for multi-step scripts.
- **Logs never tested**: the `topic_not_registered` drop/anomaly lines are operator UX; assertions use snapshots and `received_messages()`.
- **No new dependencies**; **parse-at-the-edge** unchanged (no new file formats).
