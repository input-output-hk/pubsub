# Contract — link model, seam signatures, and wire changes (015)

The boundary this feature commits to; consumers are the experiment framework and feature 016.

## Link store observation

```rust
// node.rs — public getters
pub fn links(&self) -> Vec<(PeerId, TopicId, LinkRole, LinkDirection, LinkState)>;
pub fn upstream_connections(&self) -> Vec<(PeerId, TopicId, LinkState)>;   // = Relay/Out view (semantics preserved)
pub fn downstream_connections(&self) -> Vec<(PeerId, TopicId)>;            // = Relay/In view (semantics preserved)
```

`LinkRole { Relay, Publisher }`, `LinkDirection { Out, In }` (`#[non_exhaustive]`), `LinkState { AwaitingAccept, Active }`, and `LinkStore` are exported from the crate root. `UpstreamState` is retired.

## Seam signatures (after — ADR 0034 model-family shape)

```rust
pub trait LinkSelectionStrategy: Send + Sync {       // ONE dial seam, one instance per role slot
    fn expected_links(&self, view: &NodeView<'_>) -> BTreeSet<(PeerId, TopicId)>;
}
pub trait ConnectionAcceptanceStrategy: Send + Sync { // one instance per role slot
    fn admit(&self, emitter: &PeerId, topic: &TopicId, view: &NodeView<'_>) -> Admission;
}
pub trait FanoutStrategy: Send + Sync {              // origin-aware — the dissemination-model knob
    fn targets(
        &self,
        topic: &TopicId,
        links: &LinkStore,
        origin: &Origin,
        exclude: Option<&PeerId>,
    ) -> Vec<PeerId>;
}
```

`NodeView` carries `links: &LinkStore` — the **cell-structured** store (`relay_out()` / `relay_in()` / `publish_out()` / `publish_in()`, each an ordered `LinkCell`) — so a fan-out strategy selects or unions exactly the cells its model prescribes; the role-scoped `inbound_scan(role, emitter, topic)` serves the acceptance prelude. Selection kinds (`LinkSelectionKind`): `none` | `connect-to-all` | `hash-gated` — one family for both slots (`HashGatedSelection { role, self_id, degree, bucket_override }`; standing initiation links select **unconditionally**, `m3/README.md`). Fan-out kinds (`FanoutStrategyKind`): `forward-to-all` (default; **the M3 semantics** — relay links carry every held message, initiation links owner-exclusive) | `role-scoped` (strict-partition experiment variant, no published model) | `flood-all` (**the M5 semantics** — every held message over relay-in ∪ publish-out, any origin; pair with `any-verified`). Both hash-gated relay kinds accept a **symmetric** mode (`SelectionParams.symmetric`/`AcceptanceParams.symmetric`, CLI `--symmetric-edges`; `edge::is_valid_edge_sym` under `…/edge-sym/v1` domains) — the M4 bidirectional mode. The receive gate's inbound-initiation admission is `PublishInAdmission { OwnerOnly (default), AnyVerified }` (`--publish-in-admission`), a `Node::new` parameter (ADR 0035).

## Two-phase construction (ADR 0028, extended by 0034)

`NodeStrategies::builder(relay_selection, relay_acceptance, publish_selection, publish_acceptance, fanout)` → `build(&SelectionParams, &AcceptanceParams, &SelectionParams, &AcceptanceParams)`. Both params types carry `role: LinkRole` plus the slot's `degree` (`relay_degree` / `publish_degree`); the role picks the hash domain and the flag names in errors.

## Wire

`ConnectionAction::{Request, Accepted, Terminated, Rejected}` each carry `role: LinkRole`. `PlainConnection::signed_bytes`: emitter (len-prefixed) · action tag (`0x00`–`0x03`) · topic (len-prefixed) · role tag (`0x00` Relay / `0x01` Publisher). Pre-release layout change; the encoder doc is updated in the same commit (its standing contract).

## Edge predicates (`strategies::edge`)

- Relay: `is_valid_edge(nonce, topic, requester, candidate, buckets)` — domain `pubsub/bucketed-pull/edge/v1`, **bytes unchanged** (SC-001).
- Publish: same signature under domain `pubsub/bucketed-pull/publish-edge/v1` — an independent draw (FR-009).
- Caps: relay `accept_cap(relay_degree, c)`; publish cap `accept_cap(publish_degree, c)` — counted against disjoint, role-scoped In-link sets (FR-008a).

## Behavioural invariants

1. With `--publish-strategy none` and no inbound publish requests, observable behaviour is byte-identical to pre-015 for identical inputs (FR-012/SC-001).
2. Under the M3 configuration (`forward-to-all`/`role-scoped` + `owner-only`), a `Publisher` link never carries an `Origin::Peer` forward (send side, FR-005) and never admits one (receive side, publisher binding — drop cause `relay_over_publish_link`). The M5 configuration (`flood-all` + `any-verified`) waives both halves by design (FR-015).
3. Publish admissions never consume relay `OC` slots and vice versa (FR-008a).
4. Duplicate delivery over publish + relay paths is suppressed by the existing content-hash dedup (FR-011).
