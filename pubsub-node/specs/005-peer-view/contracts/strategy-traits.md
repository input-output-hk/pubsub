# Contract: connection-strategy seams

Two injected policies plus the shared verifiable edge predicate they both consult, realising the overlay mechanics of `docs/extensions/bucketed-pull.md`. This feature adds verifiable bucketed impls (ADR 0024), evolves the **acceptance** trait's return/inputs (ADR 0025), and threads the current **interval** through both trait methods (ADR 0030). The strategy objects stay pure (genesis / `target_degree` / `c` as construction fields, interval an input) and keep the current injection (`Arc<dyn …>` at `Node::new`). The read-only node-state params are grouped into a `NodeView` (see below).

## Shared predicate — `strategies::edge` (ADR 0024/0030)

```
is_valid_edge(genesis: u64, topic: &TopicId, requester: &PeerId,
              candidate: &PeerId, interval: u64, buckets: usize) -> bool
bucket_count(candidates_len: usize, target_degree: usize) -> usize   // max(1, round(len / target_degree))
accept_cap(target_degree: usize, c: usize) -> usize                  // ⌈target_degree + c·√target_degree⌉
```

- `is_valid_edge` = `H(genesis, topic, requester, candidate, interval) mod buckets == 0`, ordered `(requester, candidate)` so it is directional. `H` = SHA-256 over a canonical length-prefixed encoding (domain-separated; cross-machine stable — **not** `DefaultHasher`). `buckets <= 1` is always valid (the small-topic / connect-to-all floor).
- Both seams call the *same* function over the *same* tuple, so they can never drift: the dial side uses it to *select* upstreams, the accept side to *verify* a request.

## Read-only node view — `strategies::view::NodeView` (re-exported as `pubsub_node::NodeView`)

```
struct NodeView<'a> {
    subscriptions: &'a BTreeSet<TopicId>,
    candidates:    &'a BTreeMap<TopicId, BTreeSet<PeerId>>,
    downstream:    &'a HashSet<(PeerId, TopicId)>,
    interval:      u64,
}
```

- Groups the read-only node-state params (`{subscriptions, candidates, downstream, interval}`) passed into both seams, so the trait methods take a single `&NodeView` rather than a growing parameter list.

## Dial side — `ConnectionStrategy` (interval threaded, ADR 0030)

```
expected_upstream(&self, view: &NodeView) -> BTreeSet<(PeerId, TopicId)>
```

- Node hands in a `NodeView` grouping the read-only state `{subscriptions, candidates, downstream, interval}`; the dial side reads the current `candidates` view and the current `interval` from it (no failed-set pre-filter).
- Impls:
  - `ConnectToAllCandidates` (existing, default) — all candidates per joined topic.
  - `HashGatedConnection { genesis, self_id, target_degree }` (new) — per joined topic `T`, compute `B = bucket_count(|candidates_T|, target_degree)` and select each candidate `U` with `is_valid_edge(genesis, T, self_id, U, interval, B)`. Expected out-degree per topic ≈ the **fixed** target (connection) degree `target_degree`; a topic with `≤ ~target_degree` candidates has `B = 1` and connects to **all** of them (small-topic fallback — no threshold, no `ln`). Pure; deterministic (fixed hash + ordered inputs); the result is a function of the *set* (order-independent).
- Guarantees: identical inputs → identical output regardless of iteration order (FR-002); pseudo-uniform per candidate over an interval/genesis sweep (SC-003); expected |output per topic| ≈ `target_degree` and `≤ ~target_degree` candidates ⇒ all selected (FR-001/FR-003).

## Inbound side — `ConnectionAcceptanceStrategy` (evolved, ADR 0025/0030)

```
admit(&self, emitter: &PeerId, topic: &TopicId, view: &NodeView) -> Admission
enum Admission { Accept, RejectMembership, RejectIllegitimate, RejectOverCapacity }
```

- Change vs today: return was `bool`; now reason-bearing, and the read-only node state is grouped into a `NodeView` (`{subscriptions, candidates, downstream, interval}`) — the decision sees the current `downstream` (to count the per-topic cap) and the current `interval` (to recompute the verifiable predicate) via the view.
- Impls:
  - `AcceptFromAllCandidates` (existing, default) — `Accept` when membership-valid, else `RejectMembership`; never `RejectIllegitimate`/`RejectOverCapacity`.
  - `VerifiableBoundedAcceptance { genesis, self_id, target_degree, cap_buffer }` (new) — `RejectMembership` if not membership-valid; else `RejectIllegitimate` if `is_valid_edge(genesis, topic, emitter, self_id, interval, B)` fails (the acceptor **verifies**, with the same `B = bucket_count(|candidates_T|, target_degree)` the dialer used); else `RejectOverCapacity` if downstream-on-topic `≥ OC = accept_cap(target_degree, cap_buffer)`; else `Accept`.
- Handler mapping: `Accept` → downstream insert + `Accepted`; `RejectMembership` → silent drop (`membership_validation_failed`); `RejectIllegitimate` → silent drop (`illegitimate_request`); `RejectOverCapacity` → drop (`downstream_capacity_reached`) + send `Rejected`.

## Construction / wiring

- Strategies are constructed at the edge from already-parsed values (`genesis`, `target_degree`, `cap_buffer`) via the two-phase builder (ADR 0028): phase 1 resolves each seam key into its `*StrategyKind`; phase 2 `NodeStrategies::builder(conn, acc).build(&ConnectionParams, &AcceptanceParams)` binds each seam's own params and builds, validating that a chosen `hash-gated`/`verifiable-bounded` kind was given its required `target_degree`. Injected at `Node::new`. Bounded impls are wired only when the parameters are supplied; otherwise the unbounded defaults (FR-010, SC-006).
