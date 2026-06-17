# Data Model: Cross-Registry Consistency Invariant + Declarative Topic Entry (014)

**Date**: 2026-06-15 | **Plan**: [plan.md](./plan.md)

Crate-internal `NodeState` and topic-domain types. Reshapes 013's projection; no public surface change beyond the additive `TopicRegistryEvent::SnapshotComplete`.

## 1. The maintained invariant

For all states reachable by folding any sequence of `Event`s through `apply`:

```
INV-1   subscriptions ⊆ registered_topics.keys()
INV-2   candidates.keys() ⊆ registered_topics.keys()
```

Both hold **at rest** (between folds), not only when read. Each fold either preserves them or is the fold that re-establishes them (the `Removed` cascade). No separate declared/pending buffer exists — `subscriptions` *is* the effective accept-filter.

## 2. `NodeState` fields (delta from 013)

| Field | 013 (main) | 014 |
|---|---|---|
| `subscriptions: HashSet<TopicId>` | declared set; may name unregistered topics; intersected at read | **maintained ⊆ registered**; never names an unregistered topic |
| `candidates: HashMap<TopicId, HashSet<PeerId>>` | per-topic, ungated by registration | **gated**: keys ⊆ registered |
| `registered_topics: HashMap<TopicId, BTreeSet<PublicKey>>` | bare publisher set | **`HashMap<TopicId, TopicEntry>`** |

Unchanged structurally: `self_id`, `received`, other 008/013 fields. Post-rebase (2026-06-17), 004's `upstream`/`downstream` connection fields are present and are **cleared by the `Removed` cascade** (FR-002/FR-010); no new persistent fields are added by this feature.

## 3. `TopicEntry` (NEW, `pub(crate)`)

```text
TopicEntry {
    publishers: BTreeSet<PublicKey>,   // empty ⇒ open topic
}
  fn is_open(&self) -> bool                              // publishers.is_empty()
  fn is_publisher_authorized(&self, key: &PublicKey)     // is_open() || publishers.contains(key)
  fn apply_publishers_diff(&mut self, added, removed)    // used by the PublishersChanged fold
  fn from_publishers(BTreeSet<PublicKey>) -> Self        // build from a Registered event's set
```

- **Placement**: `src/topic_registry/topic_entry.rs` (topic-domain cohesion); `pub(crate)`.
- **Public boundary unchanged**: `TopicRegistryEvent` keeps `BTreeSet<PublicKey>`; the fold builds `TopicEntry::from_publishers(set)`. The 012 reader is unaffected.
- **Forward shape**: owners/admins/etc. attach here later (012) without touching call sites — the ROADMAP-justified seam.
- **Replaces**: the inline `authorized.is_empty() || authorized.contains(key)` in `handle_signed_message`.

## 4. `TopicRegistryEvent` (delta — additive)

| Variant | Status |
|---|---|
| `Registered { topic, publishers }` | unchanged |
| `PublishersChanged { topic, added, removed }` | unchanged payload; **fold now defensive** (§5) |
| `Removed { topic }` | unchanged payload; **fold now cascades** (§5) |
| `SnapshotComplete` | **NEW** — terminates the cold-start `Registered` burst; carries no payload |

`#[non_exhaustive]` already; `SnapshotComplete` is additive. `InMemoryTopicRegistry::watch()` emits it once after the burst, before live deltas. The 012 reader emits it after initial chain-sync.

## 5. Fold transitions (the three folds, defensive)

### `handle_topic_registry_update` (topic-registry stream)

| Event | Transition |
|---|---|
| `Registered { topic, publishers }` | `registered_topics.insert(topic, TopicEntry::from_publishers(publishers))` (create/replace). |
| `PublishersChanged { topic, added, removed }` | **if `topic` is registered**: `entry.apply_publishers_diff(added, removed)`. **else**: drop + log (`cause = topic_not_registered`); **no `or_default` create**. |
| `Removed { topic }` | **Atomic cascade**: `registered_topics.remove(topic)`; `subscriptions.remove(topic)`; `candidates.remove(topic)`. No-op if `topic` absent. |
| `SnapshotComplete` | no-op stream delimiter (consumed by the registry indexer to find the burst end; never enqueued — a stray one folds harmlessly). |

Returns `Vec::new()` (`Effect` uninhabited).

### `handle_membership_update` (subscription-list stream)

| Event (own id) | Transition |
|---|---|
| `Joined { self, topics }` | `subscriptions = topics ∩ registered_topics.keys()`; each dropped topic logged (`cause = topic_not_registered`). |
| `TopicsChanged { self, added, removed }` | for each `added`: insert **iff registered** (else drop + log); for each `removed`: `subscriptions.remove`. |
| `Left { self }` | `subscriptions.clear()` (as 008). |

| Event (other id) | Transition |
|---|---|
| `Joined { other, topics }` | for each topic: record `(other, topic)` candidate **iff registered** (else drop + log). |
| `TopicsChanged { other, added, removed }` | `added` recorded iff registered; `removed` cleared. |
| `Left { other }` | remove `other` from all candidate sets (as 008). |

INV-1/INV-2 hold after every arm: membership never adds an unregistered topic; `Removed` cascades.

### `handle_signed_message` (receive path — behaviour-preserving)

Order unchanged (013 FR-015): connection? *(n/a here — 004)* → subscribed? → registered? → **publisher authorized?** → signature? → record. The authorization step changes expression only:

```
before:  if !(authorized.is_empty() || authorized.contains(key)) { drop }
after:   if !entry.is_publisher_authorized(key) { drop }     // entry = registered_topics.get(topic)
```

Every accept/drop outcome identical to 013 (SC-004 behaviour-preservation matrix).

## 6. Readiness / construction ordering (as built — single registry indexer)

`Node::new` does **not** block: it spawns its producers and returns. Readiness is owned by a **single** reader — the registry indexer — modelling the one chain follower a realistic deployment runs (ADR 0020, 2026-06-17 (b)). It drains the two cold-start bursts in chain order, then triggers the dial.

```
Node::new (async, non-blocking):
  spawn network mailbox
  spawn registry_indexer(subscription_registry, topic_registry, node_id)
  return                                // construction does not await readiness

registry_indexer:
  topic_watch = topic_registry.watch()          // opened up front so no event is missed
  sub_watch   = subscription_registry.watch(node)
  for ev in topic_watch until SnapshotComplete: // TOPIC burst first → registered set warm
    queue.push(TopicRegistryUpdate(ev))         // (delimiter consumed, not enqueued)
  for ev in sub_watch until SnapshotComplete:   // then MEMBERSHIP burst
    queue.push(MembershipUpdate(ev))            // (delimiter consumed, not enqueued)
  queue.push(ConnectionSetup)                   // single dial trigger: both registries warm
  loop select { topic_watch | sub_watch }:      // live deltas; stray delimiters ignored
    queue.push(TopicRegistryUpdate / MembershipUpdate)
```

Because one reader drains topics before membership, the topic burst is folded before any membership event ⇒ strict drop / candidate gating are correct at cold start (no spurious drop), with **no cross-stream ordering primitive** — the oneshot of the first as-built is gone, the ordering is intrinsic to the reader's sequence. Empty registry ⇒ the `SnapshotComplete` delimiter arrives immediately and the burst loop exits at once. A watch-open error degrades gracefully (that burst is skipped; `ConnectionSetup` still fires). The per-stream `SnapshotComplete` markers are stream-replay delimiters consumed here and never enqueued; both fold arms are symmetric no-ops.

## 7. State invariants (test targets)

- **INV-1 / INV-2** after every fold step (SC-001; `proptest` over random interleavings).
- **Strict drop, no auto-promotion**: a membership topic not registered never enters `subscriptions`; a later `Registered` does not add it (SC-008).
- **Candidate gating**: an unregistered candidate topic is never recorded (SC-008).
- **Defensive fold**: `PublishersChanged` for an unknown topic does not create it (SC-010).
- **Atomic cascade**: after `Removed`, no structure retains the topic (SC-002/003).
- **Behaviour preservation**: the 013 accept/drop matrix is unchanged (SC-004).
- **Cold-start convergence**: a multi-node bring-up converges deterministically, no spurious drop (SC-009).
