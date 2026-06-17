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

> **As-built note (see 2026-06-17 amendment §1):** the realised mechanism is an **in-node oneshot between the two reader producers** — `Node::new` is non-blocking (spawns and returns); the membership reader awaits the topic reader's signal. The "drain-and-fold in `Node::new`" wording below is the originally-planned shape; the marker decision stands, only the realisation differs.

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

1. **Readiness gate — as built.** Both registry watches now terminate their cold-start burst with a `SnapshotComplete` marker (`TopicRegistryEvent::SnapshotComplete` **and** `MembershipEvent::SnapshotComplete`). `Node::new` wires an in-node `oneshot`: the topic-registry reader signals once it has enqueued its `SnapshotComplete`; the membership reader holds its events until that signal. The single FIFO event queue then guarantees the topic burst is folded before any membership event. This is the chosen realisation of §5's readiness gate (the synchronous-drain and point-read alternatives stay rejected).

2. **Event-driven establishment replaces the setup timer.** `MembershipEvent::SnapshotComplete`, when folded, runs the connection-selection diff (`handle_connection_setup`) and returns the dial `Request`s — the node establishes connections when its membership view converges. 004's wall-clock `connection_setup_delay` (config field, TOML key, and `setup_timer_producer`) is **removed**: event-driven readiness is strictly better than a guessed delay and removes a wall-clock dependency (reproducibility standard). `Event::ConnectionSetup` is retained as the establishment mechanism (triggered by the readiness fold or external injection).

3. **Cascade extends to connection state (FR-010 flip).** A topic-registry `Removed` now also drops every `upstream`/`downstream` entry on the removed topic, in the same atomic fold — no connection outlives a topic's legitimacy.

4. **S7 / N-015 resolved, not deferred.** Under strict drop an unregistered topic is never in the subscription/candidate sets, so a connection `Request` on it fails membership validation and is rejected — acceptance is consistent with registration. No new check was added to the acceptance path; strict drop makes the unregistered-topic case unreachable. `IMPLEMENTATION_NOTES` N-015 is marked resolved.

5. **Symmetry.** Both registries now share the `SnapshotComplete` readiness-marker pattern; the node's startup is readiness-ordered (topic projection warm → membership folded → dial).
