# Contract: Cross-Registry Consistency Invariant + Declarative Topic Entry (014)

**Date**: 2026-06-15 | **Plan**: [plan.md](./plan.md) | Amends [013 contract](../013-topic-registry/contracts/topic-registry.md)

Surfaces this feature touches. "Public" = crate-public (`pub` re-exported from `lib.rs`); "internal" = `pub(crate)`/private.

## A. Public surface delta (snapshot-reshaped watch; 2026-06-18)

- **`watch()` returns a snapshot + live stream** on both registries (was a burst-then-marker stream):
  - `TopicRegistry::watch() -> Result<(TopicSnapshot, TopicRegistryWatch), _>`, where `TopicSnapshot = Vec<(TopicId, BTreeSet<PublicKey>)>` is the current registered topics.
  - `SubscriptionRegistry::watch(node) -> Result<(MembershipSnapshot, MembershipWatch), _>`, where `MembershipSnapshot = Vec<(PeerId, BTreeSet<TopicId>)>` is the node's own entry first, then its topics' scoped members.
  The snapshot reflects watch-time state; the live watch carries only subsequent deltas (no overlap). Every implementor (incl. the 012 reader) follows this shape.
- **Both `SnapshotComplete` variants are removed** — `TopicRegistryEvent::SnapshotComplete` and `MembershipEvent::SnapshotComplete` no longer exist; the remaining variants are purely live deltas. The snapshot is delivered out-of-band, so there is no burst to delimit.
- **`Event::Synced`** (new, node-`Event`) — the single readiness signal, pushed once by the registry indexer after both snapshots are folded; folding it transitions the node to `Synced` and establishes connections.
- **`Node::is_synced() -> bool`** (new getter) — the observable `Syncing`/`Synced` lifecycle.
- **Unchanged**: `Node::new`'s signature, the `subscriptions()` getter, `TopicEntry` (internal `pub(crate)`), and the `Control` write traits.

## B. Behavioural contract — `subscriptions()` getter

- Returns the node's maintained subscription set (the message accept-filter). **Semantics unchanged from 013's collapsed `subscriptions()`** (still the effective set), but the implementation no longer intersects at read time — the stored set is already `⊆ registered_topics` (INV-1). Observationally identical for any registered/stays-registered topic.

## C. Behavioural contract — the three folds (`apply`)

- **Strict drop (membership, own)**: folding `Joined`/`TopicsChanged` for the node's own id admits a topic to `subscriptions` **only if** it is currently registered; an unregistered topic is dropped (not stored, not buffered) and logged (`cause = "topic_not_registered"`). No auto-promotion on later `Registered`.
- **Candidate gating (membership, others)**: a `(peer, topic)` candidate is recorded **only if** `topic` is registered; else dropped + logged. `candidates.keys() ⊆ registered_topics.keys()` (INV-2).
- **Defensive registry fold**: only `Registered` creates a topic; `PublishersChanged` for an unregistered topic is dropped + logged (no `or_default` create); `Removed` of an unknown topic is a no-op.
- **Atomic cascade**: `Removed { topic }` clears `topic` from `subscriptions`, `candidates`, and `registered_topics` within the one `apply` fold (synchronous under the state lock — no partial state observable).
- **`Synced`**: flips `NodeState.synced` to `true` on the rising edge and returns the establishment effects (`handle_connection_setup`); idempotent thereafter. It is the single readiness/lifecycle transition; the dial **action** `ConnectionSetup` remains available directly.
- Every `apply` returns an empty `Vec<Effect>` (`Effect` uninhabited).

## D. Behavioural contract — receive path (`handle_signed_message`)

- Check order unchanged (013 FR-015): subscribed? → registered? → publisher authorized? → signature? → record.
- The authorization check is expressed via `TopicEntry::is_publisher_authorized(key)` instead of the inline `set.is_empty() || set.contains(key)`. **Every accept/drop outcome is identical to 013** (open / restricted / authorized / unauthorized / unsubscribed / unregistered / valid/invalid signature) — behaviour-preserving.

## E. Construction-ordering contract — `Node::new`

- `Node::new` is **non-blocking**: it spawns its producers and returns. A **single registry indexer** reader owns both watches (the one chain follower a realistic deployment runs; ADR 0020, 2026-06-18). It folds the **topic snapshot** first, then the **membership snapshot**, so the topic projection is warm before any membership event is folded — cold-start ordering is **intrinsic to the single reader's sequence**, with no cross-stream primitive. Once both snapshots are folded it pushes one `Event::Synced` (the single readiness signal → `Synced` transition + dial), then forwards live deltas from both watches. The two registries remain separate streams (no data merge); only the readiness signal is unified. A watch-open error degrades gracefully (that snapshot is empty; `Synced` still fires). There are no per-registry readiness markers — the snapshot/live split replaces them.

## F. Internal surface — `TopicEntry` (`pub(crate)`)

- `TopicEntry { publishers: BTreeSet<PublicKey> }` (empty ⇒ open).
- `is_open() -> bool`; `is_publisher_authorized(&PublicKey) -> bool` (= `is_open() || publishers.contains(key)`); `apply_publishers_diff(added, removed)`; `from_publishers(BTreeSet<PublicKey>) -> Self`.
- Value type of `NodeState.registered_topics: HashMap<TopicId, TopicEntry>`. Not exposed through any public trait or event.

## G. Out of scope (contract boundaries)

- **Connection surface — in scope post-rebase (2026-06-17).** After rebasing onto merged 004, the `Removed` cascade also clears `upstream`/`downstream`, and `Event::ConnectionSetup` — pushed by the registry indexer once both registries are warm (2026-06-17 (b)) — is the dial trigger (replacing the removed `connection_setup_delay` timer). S7/N-015 is resolved: an unregistered topic is never subscribed/candidate, so a `Request` on it is rejected (no acceptance-path code change — strict drop makes it unreachable). (Originally this was deferred while 004 was unmerged.)
- **No registry merge** — two distinct traits/streams retained (FR-009).
- **No governance fields** — `TopicEntry` is publishers-only; owners/admins deferred to 012 (the seam exists, unused here).
- **No new public methods on `TopicRegistry`/`TopicRegistryControl`** — readiness rides the existing watch stream as an event, not a new accessor (honors 013 FR-001 watch-only).
