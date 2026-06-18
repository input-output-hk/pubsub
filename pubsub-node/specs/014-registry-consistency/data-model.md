# Data Model: Cross-Registry Consistency Invariant + Declarative Topic Entry (014)

**Date**: 2026-06-15 | **Plan**: [plan.md](./plan.md)

Crate-internal `NodeState` and topic-domain types. Reshapes 013's projection. Public surface deltas (2026-06-18): both `watch()` methods return `(snapshot, live-watch)`; both `SnapshotComplete` event variants are removed; a new `Event::Synced` + `Node::is_synced()` model the readiness lifecycle (see §4, §6, contract §A).

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
| `synced: bool` | — | **NEW** (2026-06-18) — the `Syncing → Synced` lifecycle flag; set by `Event::Synced` once both registry snapshots are folded; exposed via `is_synced()` |

Unchanged structurally: `self_id`, `received`, other 008/013 fields. Post-rebase (2026-06-17), 004's `upstream`/`downstream` connection fields are present and are **cleared by the `Removed` cascade** (FR-002/FR-010).

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

## 4. Registry events + the watch snapshot (2026-06-18 snapshot reshape)

`TopicRegistryEvent` and `MembershipEvent` are now **purely live deltas**; the cold-start state is delivered out-of-band as a snapshot returned by `watch()`.

| `TopicRegistryEvent` variant | Status |
|---|---|
| `Registered { topic, publishers }` | live registration (the snapshot carries already-registered topics); folds as create/replace |
| `PublishersChanged { topic, added, removed }` | unchanged payload; **fold defensive** (§5) |
| `Removed { topic }` | unchanged payload; **fold cascades** (§5) |
| ~~`SnapshotComplete`~~ | **removed** — no burst to delimit |

- `TopicRegistry::watch() -> (TopicSnapshot, TopicRegistryWatch)`, `TopicSnapshot = Vec<(TopicId, BTreeSet<PublicKey>)>`.
- `SubscriptionRegistry::watch(node) -> (MembershipSnapshot, MembershipWatch)`, `MembershipSnapshot = Vec<(PeerId, BTreeSet<TopicId>)>` (own entry first, then scoped members). `MembershipEvent::SnapshotComplete` is likewise **removed**.
- The indexer folds the snapshot (as `Registered`/`Joined` events) then forwards the live watch. The 012 reader returns its at-tip snapshot the same way.

## 5. Fold transitions (the three folds, defensive)

### `handle_topic_registry_update` (topic-registry stream)

| Event | Transition |
|---|---|
| `Registered { topic, publishers }` | `registered_topics.insert(topic, TopicEntry::from_publishers(publishers))` (create/replace). |
| `PublishersChanged { topic, added, removed }` | **if `topic` is registered**: `entry.apply_publishers_diff(added, removed)`. **else**: drop + log (`cause = topic_not_registered`); **no `or_default` create**. |
| `Removed { topic }` | **Atomic cascade**: `registered_topics.remove(topic)`; `subscriptions.remove(topic)`; `candidates.remove(topic)`; `upstream`/`downstream` entries on the topic dropped. No-op if `topic` absent. |

`Event::Synced` (node-`Event`, separate from the registry streams) folds via `handle_synced`: on the rising edge it sets `NodeState.synced = true` and returns `handle_connection_setup` effects; idempotent thereafter.

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

## 6. Readiness / construction ordering (as built — single registry indexer, snapshot watch)

`Node::new` does **not** block: it spawns its producers and returns. Readiness is owned by a **single** reader — the registry indexer — modelling the one chain follower a realistic deployment runs (ADR 0020, 2026-06-18). It folds each registry's current-state **snapshot** in order, then pushes one `Synced`.

```
Node::new (async, non-blocking):
  spawn network mailbox
  spawn registry_indexer(subscription_registry, topic_registry, node_id)
  return                                       // construction does not await readiness

registry_indexer:
  (topic_snapshot, topic_watch) = topic_registry.watch()        // TOPIC snapshot first
  for (topic, publishers) in topic_snapshot:                    //   → registered set warm
    queue.push(TopicRegistryUpdate(Registered{topic,publishers}))
  (sub_snapshot, sub_watch) = subscription_registry.watch(node) // then MEMBERSHIP snapshot
  for (node, topics) in sub_snapshot:
    queue.push(MembershipUpdate(Joined{node,topics}))
  queue.push(Synced)                           // single readiness signal → Synced + dial
  loop select { topic_watch | sub_watch }:     // live deltas
    queue.push(TopicRegistryUpdate / MembershipUpdate)
```

One reader folds the topic snapshot before the membership snapshot ⇒ strict drop / candidate gating are correct at cold start (no spurious drop), with **no cross-stream ordering primitive** — ordering is intrinsic to the reader's sequence. The snapshot and the live watch do not overlap, so there is no burst to delimit — both `SnapshotComplete` markers are gone. Empty registry ⇒ empty snapshot, `Synced` fires immediately. A watch-open error degrades gracefully (that snapshot is empty; `Synced` still fires). `Synced` folds into `NodeState.synced` (the `Syncing → Synced` lifecycle) and establishes connections once.

## 7. State invariants (test targets)

- **INV-1 / INV-2** after every fold step (SC-001; `proptest` over random interleavings).
- **Strict drop, no auto-promotion**: a membership topic not registered never enters `subscriptions`; a later `Registered` does not add it (SC-008).
- **Candidate gating**: an unregistered candidate topic is never recorded (SC-008).
- **Defensive fold**: `PublishersChanged` for an unknown topic does not create it (SC-010).
- **Atomic cascade**: after `Removed`, no structure retains the topic (SC-002/003).
- **Behaviour preservation**: the 013 accept/drop matrix is unchanged (SC-004).
- **Cold-start convergence**: a multi-node bring-up converges deterministically, no spurious drop (SC-009).
