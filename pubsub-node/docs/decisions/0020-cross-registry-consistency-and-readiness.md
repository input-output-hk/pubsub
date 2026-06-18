# ADR 0020: Cross-registry consistency invariant, defensive folds, and the readiness gate

**Status**: Accepted
**Date**: 2026-06-15
**Feature**: 014-registry-consistency
**Amends**: ADR 0016 (topic-registry interface and node integration)
**Source**: `specs/014-registry-consistency/{spec,plan,research,data-model}.md` + `contracts/registry-consistency.md`; the PR #55 maintainer design discussion (team meeting 2026-06-15); ADR 0016 (013 projection + accept path), ADR 0014 (008 membership fold), ADR 0011/0012 (004 pure core + `Arc<Mutex<NodeState>>` lifecycle); `IMPLEMENTATION_NOTES.md` N-015 + data-model staleness row S7 (the cross-registry ordering invariant the 004-connections PR flagged).

## Context

013 (ADR 0016, note 4) folded two independent streams into `NodeState` — 008's membership-derived `subscriptions` and 013's `registered_topics` — and computed the effective accept-filter as a **read-time intersection** (`subscriptions ∩ registered_topics`), leaving the stored subscription set possibly inconsistent (it could name an unregistered topic, masked at read). The 004-connections PR (#56, unmerged) flagged the consequence (N-015 / S7): connection acceptance could establish on topics that delivery would drop, because nothing maintained `subscriptions ⊆ registered` as a real invariant.

The maintainers resolved this (PR #55 review): both registries are chain-derived; the chain follower delivers their events **in order** (a topic is registered before any subscription references it; deletions are ordered), so consistency is *sourced* on-chain — but the node must **validate, not assume**, enforcing the invariant on its own state by **dropping** events that would violate it. This is structural per Principle III: it changes the fold contract of already-merged code (008/013), the receive path's expression, and `Node::new`'s construction ordering, and it adds a public protocol-event variant the 012 reader must emit — none of it a local rewrite. Five `/speckit-clarify` resolutions (2026-06-15) fixed the specifics; this ADR records the decisions and amends ADR 0016.

## Decision

### 1. A maintained consistency invariant replaces the read-time intersection

`NodeState` keeps a single `subscriptions: HashSet<TopicId>` that is, by construction, always `⊆ registered_topics.keys()`, and the candidate map obeys the same relation:

```
INV-1   subscriptions ⊆ registered_topics.keys()
INV-2   candidates.keys() ⊆ registered_topics.keys()
```

Both hold at rest, not only at read. `subscriptions_snapshot()` returns the set directly (no intersection). ADR 0016 note 4's "two sets ANDed at accept time" is superseded. There is **no** separate declared/pending buffer.

### 2. Strict drop (membership), symmetric for candidates

Folding the node's **own** membership admits a topic to `subscriptions` only if it is currently registered; an unregistered topic is **dropped** (not stored, not buffered) and logged (`cause = "topic_not_registered"`). Folding **another** node's membership records a `(peer, topic)` candidate only if `topic` is registered; else dropped + logged. **No auto-promotion**: a topic registered after a dropped subscription requires a fresh membership event (the chain follower's ordering guarantees that ordering in production). **Consequence**: 013 SC-004 ("subscribe-before-register → becomes effective") is **removed** — it existed only because 013 folded unordered streams; under the ordering premise + readiness gate (§4) it is an off-nominal case, not a supported dynamic.

### 3. Atomic cascade on `Removed`

A topic-registry `Removed { topic }` fold clears `topic` from `subscriptions`, from `candidates`, and from `registered_topics` within the single `apply` call. Because `apply` runs synchronously under the state lock (ADR 0012) and every getter takes the same lock, the multi-structure update is **atomic with respect to any observer** by construction — no partial state is visible, no extra mechanism needed.

### 4. Defensive topic-registry fold (create-only-on-`Registered`)

Only `Registered` creates a topic in the projection. A `PublishersChanged` for a topic with no current entry is **dropped + logged** (no `or_default` auto-create); a `Removed` for an unknown topic is a no-op. This makes "validate, don't assume" uniform across both streams and removes the phantom-registration `or_default` path. Amends ADR 0016's lenient fold (013 FR-013); the 013 `or_default` fold test is reworked.

### 5. The readiness gate — `SnapshotComplete` marker + drain-then-spawn ordering

> **As-built note (superseded — see Amendment 2026-06-17 (b)):** the realised mechanism is a **single `registry_indexer_loop`** that drains the topic burst before the membership burst (ordering intrinsic, no oneshot) and pushes one `Event::ConnectionSetup`. An earlier as-built used an in-node oneshot between two reader producers (Amendment 2026-06-17 §1), and the "drain-and-fold in `Node::new`" wording below is the originally-planned shape; the marker decision stands, only the realisation differs.

Strict drop evaluates each subscription against the *current* registered set, so the node must warm `registered_topics` before folding membership. `TopicRegistryEvent` gains an additive (`#[non_exhaustive]`) **`SnapshotComplete`** variant terminating the cold-start `Registered` burst. `InMemoryTopicRegistry::watch()` emits it once after the burst, before live deltas; the 012 reader emits it after initial chain-sync. `Node::new` opens the topic watch, **drains and folds events up to `SnapshotComplete`** (seeding the projection), and only **then** spawns the membership reader (it then spawns the topic-reader producer to continue draining live deltas from the same watch). `handle_topic_registry_update` treats `SnapshotComplete` as a no-op. The two registries remain separate streams (no merge); this is an ordering gate only — the minimal un-deferred slice of the "registry synchronization complete" event the team otherwise deferred.

### 6. `TopicEntry` — the declarative publisher type (crate-internal)

`registered_topics` becomes `HashMap<TopicId, TopicEntry>`, where `TopicEntry` (`pub(crate)`, in `src/topic_registry/topic_entry.rs`) wraps `BTreeSet<PublicKey>` and exposes `is_open()` (empty set) and `is_publisher_authorized(&PublicKey)` (`is_open() || contains`), plus `apply_publishers_diff` and `from_publishers`. `handle_signed_message` calls `is_publisher_authorized` instead of the inline `set.is_empty() || set.contains(key)` — behaviour-preserving. `TopicEntry` is **internal**: the public `TopicRegistryEvent` keeps carrying `BTreeSet<PublicKey>` (the fold builds a `TopicEntry` from it), so no node-projection type leaks through the registry interface and the 012 reader is unaffected. It is the ROADMAP-justified seam for future per-topic governance fields (owners/admins — 012).

## Consequences

- **Positive**: the invariant 004's acceptance/dialing can rely on (`subscriptions/candidates ⊆ registered`) holds at all times; no inconsistent intermediate state survives a removal; the open-topic rule is a named predicate, not an inline idiom; the readiness gate makes the mock faithful to the chain follower's ordering and generalizes to 012.
- **Behaviour-preserving** for registered/stays-registered topics: the 013/008/003 accept/drop matrix is unchanged.
- **Removes 013 SC-004**; its test is reworked into strict-drop + readiness coverage. 013 integration/state tests that assumed the read-time-intersection model are reworked.
- **Public surface**: one additive variant (`TopicRegistryEvent::SnapshotComplete`), which every `TopicRegistry` implementor must emit. `Node::new` and `subscriptions()` signatures unchanged.
- **`Node::new` blocks** on topic-registry readiness (immediate for the mock — `SnapshotComplete` follows zero `Registered`s on an empty registry; a future async 012 reader makes this a genuine await).
- **Defers** the connection-state cascade and acceptance-path registration enforcement to the **004-connections rebase** (no connection fields on `main`); `IMPLEMENTATION_NOTES` N-015 / S7 updated to record the invariant is established and what 004 must carry through.

## Alternatives considered

- **Keep the read-time intersection / declared-intent + promote (the "hold" model)** — preserves 013 SC-004 and tolerates unordered streams, but keeps a second buffer and a dynamic the maintainers chose to drop. Rejected (research D1/D8).
- **Lenient registry fold (`or_default`)** — simpler, FR-008-as-013, but leaves phantom registrations and breaks the uniform validate-don't-assume posture. Rejected (research D3).
- **Synchronous drain-until-empty in `Node::new`** (no marker) — relies on the whole burst being queued before `watch()` returns; true for `InMemoryTopicRegistry`, **false for 012's async chain-sync**. Rejected as not forward-compatible (research D5).
- **A point-read `snapshot()` on `TopicRegistry`** — reintroduces the point-read 013 (ADR 0016 §3, FR-001) deliberately excluded, and double-counts against the watch burst. Rejected (research D5).
- **Public `TopicEntry` carried by `TopicRegistryEvent`** — couples the event contract to the node's projection shape, violating the anti-corruption boundary. Rejected (research D7).
- **Merge the two registries / a single merged delete event** — declined by the maintainers; consistency is achieved by the invariant + cascade + readiness gate over two separate streams (research; spec FR-009).

## Amendment 2026-06-17 — rebased onto merged 004-connections (as built)

004-connections merged to `main` mid-implementation; 014 was rebased onto it. The decisions above stand; this records what the merged foundation changed:

1. **Readiness gate — first as-built (superseded by the 2026-06-17 (b) amendment below).** Both registry watches terminate their cold-start burst with a `SnapshotComplete` marker (`TopicRegistryEvent::SnapshotComplete` **and** `MembershipEvent::SnapshotComplete`). `Node::new` wired an in-node `oneshot`: the topic-registry reader signalled once it had enqueued its `SnapshotComplete`; the membership reader held its events until that signal; the single FIFO event queue then guaranteed the topic burst was folded before any membership event. This used **two** readiness events — one (topic) a fold no-op, one (membership) the dial trigger — modelling two independent chain read-positions. The 2026-06-17 (b) amendment collapses them to a single-indexer reader; the synchronous-drain and point-read alternatives stay rejected.

2. **Event-driven establishment replaces the setup timer.** Establishment is triggered by `Event::ConnectionSetup` (which runs the connection-selection diff `handle_connection_setup` and returns the dial `Request`s) rather than a wall-clock timer — the node establishes connections when its membership view converges. 004's `connection_setup_delay` (config field, TOML key, and `setup_timer_producer`) is **removed**: event-driven readiness is strictly better than a guessed delay and removes a wall-clock dependency (reproducibility standard). In this first as-built, `Event::ConnectionSetup` was reached by folding `MembershipEvent::SnapshotComplete`; the 2026-06-17 (b) amendment has the indexer push `Event::ConnectionSetup` directly.

3. **Cascade extends to connection state (FR-010 flip).** A topic-registry `Removed` now also drops every `upstream`/`downstream` entry on the removed topic, in the same atomic fold — no connection outlives a topic's legitimacy.

4. **S7 / N-015 resolved, not deferred.** Under strict drop an unregistered topic is never in the subscription/candidate sets, so a connection `Request` on it fails membership validation and is rejected — acceptance is consistent with registration. No new check was added to the acceptance path; strict drop makes the unregistered-topic case unreachable. `IMPLEMENTATION_NOTES` N-015 is marked resolved.

5. **Symmetry.** Both registries now share the `SnapshotComplete` readiness-marker pattern; the node's startup is readiness-ordered (topic projection warm → membership folded → dial).

## Amendment 2026-06-17 (b) — single-indexer readiness collapse (supersedes the first as-built §1/§2)

The first as-built used **two** readiness events: `TopicRegistryEvent::SnapshotComplete` (a fold no-op, present only so the topic reader could fire the oneshot) and `MembershipEvent::SnapshotComplete` (the dial trigger). That asymmetry — one event that changes no state, one that does — modelled **two independent chain read-positions**. A realistic later prototype follows the chain with a **single indexer**: there is exactly one "caught up to tip" moment covering both the topic registry and the subscription list. The two-marker + oneshot apparatus was an artifact of the mock having two independently-ordered in-memory watches, not a property the real system will have. Decision (maintainer review, 2026-06-17): collapse the readiness signal to match the single-indexer model.

1. **One reader: the registry indexer.** The two reader producers and the in-node `oneshot` are replaced by a single `registry_indexer_loop` that owns both watches. It drains the **topic** cold-start burst first, then the **membership** burst — so cold-start ordering (a topic is registered before any membership event references it; strict drop, §2) is **intrinsic to the single reader's sequence**, not imposed by a cross-stream primitive. The oneshot is gone. After both bursts it forwards live deltas from both watches (a `tokio::select!` over the two `mpsc` receivers).

2. **One dial trigger, no new event.** Once both bursts have drained, the indexer pushes the **existing** `Event::ConnectionSetup` — the single event-driven dial trigger. No new node-`Event` variant is introduced; the node's reaction to "chain caught up" *is* connection setup. 012's real single indexer emits this one readiness directly.

3. **Per-stream markers demoted to delimiters.** `TopicRegistryEvent::SnapshotComplete` and `MembershipEvent::SnapshotComplete` are retained as the **stream-replay delimiters each mock watch emits** to mark its burst end; the indexer **consumes** them and never enqueues them. Both fold arms are now **symmetric no-ops** (`=> {}`) — a stray marker reaching `apply` is harmless. This removes the asymmetry §5 (above) papered over: neither marker folds into node state. The registry mocks and their watch contracts are unchanged (they still emit the markers).

4. **Scope: readiness signal only — registries stay separate.** Only the *readiness* signal collapses; the topic registry and subscription list remain distinct data artifacts with distinct watches (013/014's deliberate keep-separate; distinct on-chain contracts). No registry-data merge.

5. **Behaviour preserved.** The dial still fires exactly once, after both registries are warm; cold-start ordering and strict drop are unchanged. The reworked unit test asserts the membership marker is a fold no-op and that `Event::ConnectionSetup` is the dial trigger; the autonomous-establishment integration test is unchanged (the node still dials on its own after cold start).

## Amendment 2026-06-18 — snapshot-reshaped watch + single `Synced` lifecycle (supersedes the 2026-06-17 (b) markers/dial)

(b) kept two `SnapshotComplete` markers as stream-replay delimiters and reused `Event::ConnectionSetup` as the readiness-driven dial. Maintainer review then asked the next question: *why two markers at all?* The node only cares about **one** thing — that it is **synced** (both registries up to date). The two markers existed only because each watch streamed its initial state as a **burst** that needed an in-band end delimiter. Removing the markers means moving the snapshot out of the stream. Decision (2026-06-18): reshape the watch contract to deliver a current-state snapshot up front, and model node readiness as an explicit `Syncing → Synced` lifecycle.

1. **Snapshot-plus-live watch contract.** Both `watch()` methods now return `(snapshot, live-watch)` instead of a burst-then-marker stream:
   - `TopicRegistry::watch() -> (TopicSnapshot, TopicRegistryWatch)`, `TopicSnapshot = Vec<(TopicId, BTreeSet<PublicKey>)>`.
   - `SubscriptionRegistry::watch(node) -> (MembershipSnapshot, MembershipWatch)`, `MembershipSnapshot = Vec<(PeerId, BTreeSet<TopicId>)>` (own entry first, then scoped members).
   The snapshot reflects the registry at watch time; the live watch carries only subsequent deltas (no overlap). This is **more faithful to a real chain indexer** (query state at tip, then subscribe) than the burst-with-marker model, and it makes the strict-drop ordering trivially a matter of folding the topic snapshot before the membership snapshot.

2. **Both `SnapshotComplete` variants are removed.** With the snapshot delivered out-of-band there is no burst to delimit, so `TopicRegistryEvent::SnapshotComplete` and `MembershipEvent::SnapshotComplete` are deleted entirely (and the `snapshot_complete()` test constructor with them). The remaining `*RegistryEvent`/`MembershipEvent` variants are purely live deltas.

3. **One readiness signal: `Event::Synced`.** The indexer folds the topic snapshot (as `Registered` events), then the membership snapshot (as `Joined` events), then pushes a single new `Event::Synced`. There are no per-registry markers and no manual `ConnectionSetup` orchestration in the reader.

4. **Explicit `Syncing → Synced` lifecycle in the pure core.** `NodeState` gains `synced: bool` (exposed via `NodeState::is_synced()` / `Node::is_synced()`). `handle_synced` flips it to `Synced` on the rising edge and establishes connections (delegating to `handle_connection_setup`); a redundant `Synced` is an idempotent no-op. `Event::ConnectionSetup` is retained as the dial **action** (tests, operator injection, future epochal re-dial); `Synced` is the lifecycle **transition** that invokes it. The node's "two behaviours depending on proximity to the tip" are now first-class and observable — the abstraction generalises to a real indexer where a node can fall behind and re-enter `Syncing`.

5. **Scope unchanged from (b).** Still readiness-signal-only: the two registries remain distinct data artifacts with distinct watches; no data merge. 012's single indexer emits the one `Synced` directly. Full gate green (`fmt`, `clippy -D warnings`, all test binaries + doctests).
