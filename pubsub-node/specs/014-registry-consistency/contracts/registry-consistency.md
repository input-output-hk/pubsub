# Contract: Cross-Registry Consistency Invariant + Declarative Topic Entry (014)

**Date**: 2026-06-15 | **Plan**: [plan.md](./plan.md) | Amends [013 contract](../013-topic-registry/contracts/topic-registry.md)

Surfaces this feature touches. "Public" = crate-public (`pub` re-exported from `lib.rs`); "internal" = `pub(crate)`/private.

## A. Public surface delta (additive, minimal)

- **`TopicRegistryEvent`** (public, `#[non_exhaustive]`) gains one variant:
  - `SnapshotComplete` — terminates the cold-start `Registered` burst on a watch; carries no payload. Every `TopicRegistry` implementor MUST emit it once after the initial burst and before any live delta. `InMemoryTopicRegistry::watch()` emits it; the 012 on-chain reader emits it after initial chain-sync. Re-exported via the existing `TopicRegistryEvent` re-export.
- **No other public type changes.** `TopicRegistry` / `TopicRegistryControl` traits, `TopicRegistryWatch`, `InMemoryTopicRegistry`, `Node::new`'s **signature**, and the `subscriptions()` getter's **signature** are unchanged. `TopicEntry` is **internal** (`pub(crate)`), not re-exported.

## B. Behavioural contract — `subscriptions()` getter

- Returns the node's maintained subscription set (the message accept-filter). **Semantics unchanged from 013's collapsed `subscriptions()`** (still the effective set), but the implementation no longer intersects at read time — the stored set is already `⊆ registered_topics` (INV-1). Observationally identical for any registered/stays-registered topic.

## C. Behavioural contract — the three folds (`apply`)

- **Strict drop (membership, own)**: folding `Joined`/`TopicsChanged` for the node's own id admits a topic to `subscriptions` **only if** it is currently registered; an unregistered topic is dropped (not stored, not buffered) and logged (`cause = "topic_not_registered"`). No auto-promotion on later `Registered`.
- **Candidate gating (membership, others)**: a `(peer, topic)` candidate is recorded **only if** `topic` is registered; else dropped + logged. `candidates.keys() ⊆ registered_topics.keys()` (INV-2).
- **Defensive registry fold**: only `Registered` creates a topic; `PublishersChanged` for an unregistered topic is dropped + logged (no `or_default` create); `Removed` of an unknown topic is a no-op.
- **Atomic cascade**: `Removed { topic }` clears `topic` from `subscriptions`, `candidates`, and `registered_topics` within the one `apply` fold (synchronous under the state lock — no partial state observable).
- **`SnapshotComplete`** (both `TopicRegistryEvent` and `MembershipEvent`): no-op in the fold — a stream-replay delimiter the registry indexer consumes and never enqueues; a stray one folds harmlessly.
- Every `apply` returns an empty `Vec<Effect>` (`Effect` uninhabited).

## D. Behavioural contract — receive path (`handle_signed_message`)

- Check order unchanged (013 FR-015): subscribed? → registered? → publisher authorized? → signature? → record.
- The authorization check is expressed via `TopicEntry::is_publisher_authorized(key)` instead of the inline `set.is_empty() || set.contains(key)`. **Every accept/drop outcome is identical to 013** (open / restricted / authorized / unauthorized / unsubscribed / unregistered / valid/invalid signature) — behaviour-preserving.

## E. Construction-ordering contract — `Node::new`

- `Node::new` is **non-blocking**: it spawns its producers and returns. A **single registry indexer** reader owns both watches (the one chain follower a realistic deployment runs; ADR 0020, 2026-06-17 (b)). It drains the **topic** cold-start burst first, then the **membership** burst, so the topic projection is warm before any membership event is folded — cold-start ordering is **intrinsic to the single reader's sequence**, with no cross-stream primitive (the earlier in-node oneshot is removed). Once both bursts have drained it pushes one `Event::ConnectionSetup` (the single dial trigger), then forwards live deltas from both watches. The two registries remain separate streams (no data merge); only the readiness signal is unified. A watch-open error degrades gracefully (that burst is skipped; `ConnectionSetup` still fires). The per-stream `SnapshotComplete` markers are stream-replay delimiters consumed by the indexer and never enqueued.

## F. Internal surface — `TopicEntry` (`pub(crate)`)

- `TopicEntry { publishers: BTreeSet<PublicKey> }` (empty ⇒ open).
- `is_open() -> bool`; `is_publisher_authorized(&PublicKey) -> bool` (= `is_open() || publishers.contains(key)`); `apply_publishers_diff(added, removed)`; `from_publishers(BTreeSet<PublicKey>) -> Self`.
- Value type of `NodeState.registered_topics: HashMap<TopicId, TopicEntry>`. Not exposed through any public trait or event.

## G. Out of scope (contract boundaries)

- **Connection surface — in scope post-rebase (2026-06-17).** After rebasing onto merged 004, the `Removed` cascade also clears `upstream`/`downstream`, and `Event::ConnectionSetup` — pushed by the registry indexer once both registries are warm (2026-06-17 (b)) — is the dial trigger (replacing the removed `connection_setup_delay` timer). S7/N-015 is resolved: an unregistered topic is never subscribed/candidate, so a `Request` on it is rejected (no acceptance-path code change — strict drop makes it unreachable). (Originally this was deferred while 004 was unmerged.)
- **No registry merge** — two distinct traits/streams retained (FR-009).
- **No governance fields** — `TopicEntry` is publishers-only; owners/admins deferred to 012 (the seam exists, unused here).
- **No new public methods on `TopicRegistry`/`TopicRegistryControl`** — readiness rides the existing watch stream as an event, not a new accessor (honors 013 FR-001 watch-only).
