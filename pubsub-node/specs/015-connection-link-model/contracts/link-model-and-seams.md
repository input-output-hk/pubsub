# Contract — link model, seam signatures, and wire changes (015)

The boundary this feature commits to; consumers are the experiment framework and feature 016.

## Link store observation

```rust
// node.rs — public getters
pub fn links(&self) -> Vec<(PeerId, TopicId, LinkRole, LinkDirection, LinkState)>;
pub fn upstream_connections(&self) -> Vec<(PeerId, TopicId, LinkState)>;   // = Relay/Out view (semantics preserved)
pub fn downstream_connections(&self) -> Vec<(PeerId, TopicId)>;            // = Relay/In view (semantics preserved)
```

`LinkRole { Relay, Publisher }`, `LinkDirection { Out, In }` (`#[non_exhaustive]`), `LinkState { AwaitingAccept, Active }` are exported from the crate root. `UpstreamState` is retired.

## Seam signatures (after)

```rust
pub trait ConnectionStrategy: Send + Sync {          // relay selection — shape unchanged
    fn expected_upstream(&self, view: &NodeView<'_>) -> BTreeSet<(PeerId, TopicId)>;
}
pub trait PublishStrategy: Send + Sync {             // NEW
    /// Publish targets per topic; empty unless the M3 trigger holds for the
    /// topic (no expected relay downstream — research R6).
    fn expected_publish(&self, view: &NodeView<'_>) -> BTreeSet<(PeerId, TopicId)>;
}
pub trait ConnectionAcceptanceStrategy: Send + Sync { // one instance per role slot
    fn admit(&self, emitter: &PeerId, topic: &TopicId, view: &NodeView<'_>) -> Admission;
}
pub trait FanoutStrategy: Send + Sync {              // origin-aware
    fn targets(
        &self,
        topic: &TopicId,
        links: &BTreeMap<(PeerId, TopicId, LinkRole, LinkDirection), LinkState>,
        origin: &Origin,
        exclude: Option<&PeerId>,
    ) -> Vec<PeerId>;
}
```

`NodeView` carries `links` (borrow of the store) instead of `downstream`; the role-scoped helper `inbound_scan(role, emitter, topic)` is part of the view's contract. (An observed-downstream helper was planned but dropped: the M3 trigger reads the *expected* set via the predicate — research R6 — so no consumer exists; analysis.md A1.)

## Two-phase construction (ADR 0028, extended)

`NodeStrategies::builder(connection, acceptance, publish, publish_acceptance)` → `build(&ConnectionParams, &AcceptanceParams, &PublishParams, &PublishAcceptanceParams)`. Relay params carry `relay_degree` (renamed); publish params carry `publish_degree`. Kinds: `PublishStrategyKind { None, HashGated }` (`none` | `hash-gated`); the publish acceptance slot reuses `AcceptanceStrategyKind`.

## Wire

`ConnectionAction::{Request, Accepted, Terminated, Rejected}` each carry `role: LinkRole`. `PlainConnection::signed_bytes`: emitter (len-prefixed) · action tag (`0x00`–`0x03`) · topic (len-prefixed) · role tag (`0x00` Relay / `0x01` Publisher). Pre-release layout change; the encoder doc is updated in the same commit (its standing contract).

## Edge predicates (`strategies::edge`)

- Relay: `is_valid_edge(nonce, topic, requester, candidate, buckets)` — domain `pubsub/bucketed-pull/edge/v1`, **bytes unchanged** (SC-001).
- Publish: same signature under domain `pubsub/bucketed-pull/publish-edge/v1` — an independent draw (FR-009).
- Caps: relay `accept_cap(relay_degree, c)`; publish cap `accept_cap(publish_degree, c)` — counted against disjoint, role-scoped In-link sets (FR-008a).

## Behavioural invariants

1. With `--publish-strategy none` and no inbound publish requests, observable behaviour is byte-identical to pre-015 for identical inputs (FR-012/SC-001).
2. A `Publisher` link never carries an `Origin::Peer` forward (send side, FR-005) and never admits one (receive side, publisher binding — drop cause `relay_over_publish_link`).
3. Publish admissions never consume relay `OC` slots and vice versa (FR-008a).
4. Duplicate delivery over publish + relay paths is suppressed by the existing content-hash dedup (FR-011).
