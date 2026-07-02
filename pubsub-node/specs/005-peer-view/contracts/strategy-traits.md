# Contract: connection-strategy seams

Two injected policies. This feature adds bounded impls and evolves the **acceptance** trait's return/inputs (ADR 0025); the **dial** trait shape is unchanged (R3). The strategy objects stay pure (seed/bounds as construction fields) and keep the current injection; they migrate unchanged if the parallel refactor later passes strategies as `apply` arguments (R6).

## Dial side — `ConnectionStrategy` (shape unchanged)

```
expected_upstream(subscriptions: &BTreeSet<TopicId>,
                  candidates:    &BTreeMap<TopicId, BTreeSet<PeerId>>) -> BTreeSet<(PeerId, TopicId)>
```

- Node hands in the current `candidates` view directly (no failed-set pre-filter; the earlier candidates-minus-failed diff was removed in the PR-73 simplification).
- Impls:
  - `ConnectToAllCandidates` (existing, default) — all candidates per joined topic.
  - `SeededBoundedConnection { seed, self_id, upstream_degree }` (new) — a PRNG-sampled `upstream_degree`-subset of the candidates per topic (partial Fisher–Yates via `ChaCha20Rng`, re-seeded per call from `(seed, self_id, topic)`) over the ordered candidate set; all when candidates ≤ `upstream_degree`. Pure; deterministic (fixed algorithm + ordered inputs); no state carried across calls.
- Guarantees: identical inputs → identical output regardless of iteration order (FR-003); uniform over a seed sweep (FR-007); |output per topic| ≤ `upstream_degree` (FR-001/FR-002).

## Inbound side — `ConnectionAcceptanceStrategy` (evolved, ADR 0025)

```
admit(emitter, topic, subscriptions, candidates, downstream) -> Admission
enum Admission { Accept, RejectMembership, RejectOverCapacity }
```

- Change vs today: return was `bool`; now reason-bearing, and the decision sees the current `downstream` so the downstream degree can be enforced.
- Impls:
  - `AcceptFromAllCandidates` (existing, default) — `Accept` when membership-valid, else `RejectMembership`; never `RejectOverCapacity`.
  - `BoundedAcceptance { downstream_degree }` (new) — `RejectMembership` if not membership-valid; else `RejectOverCapacity` if downstream count ≥ `downstream_degree`; else `Accept`.
- Handler mapping: `Accept` → downstream insert + `Accepted`; `RejectMembership` → silent drop; `RejectOverCapacity` → drop (distinct cause) + send `Rejected`.

## Construction / wiring

- Strategies are constructed at the edge from already-parsed values (seed, upstream degree, downstream degree) and injected at node construction (current shape; migrates to the argument shape with the parallel refactor). Bounded impls are wired only when the parameters are supplied; otherwise the unbounded defaults (FR-013, SC-005).
