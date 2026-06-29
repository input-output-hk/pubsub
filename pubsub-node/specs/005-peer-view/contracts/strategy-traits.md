# Contract: connection-strategy seams

Two injected policies. This feature adds bounded impls and evolves the **acceptance** trait's return/inputs (ADR 0025); the **dial** trait shape is unchanged (R3). The strategy objects stay pure (seed/bounds as construction fields) and keep the current injection; they migrate unchanged if the parallel refactor later passes strategies as `apply` arguments (R6).

## Dial side — `ConnectionStrategy` (shape unchanged)

```
expected_upstream(subscriptions: &Set<TopicId>,
                  candidates:    &Map<TopicId, Set<PeerId>>) -> Set<(PeerId, TopicId)>
```

- Node hands in the **viable** candidate view (candidates minus `failed_upstream`).
- Impls:
  - `ConnectToAllCandidates` (existing, default) — all candidates per joined topic.
  - `SeededBoundedSelection { seed, self_id, out_degree }` (new) — lowest-`out_degree` candidates per topic by stable SHA-256 of `(seed, self_id, topic, candidate_id)`, tie-broken on `candidate_id`; all when candidates ≤ `out_degree`. Pure; deterministic; no RNG.
- Guarantees: identical inputs → identical output regardless of iteration order (FR-003); uniform over a seed sweep (FR-007); |output per topic| ≤ `out_degree` (FR-001/FR-002).

## Inbound side — `ConnectionAcceptanceStrategy` (evolved, ADR 0025)

```
admit(emitter, topic, subscriptions, candidates, downstream) -> Admission
enum Admission { Accept, RejectMembership, RejectOverCapacity }
```

- Change vs today: return was `bool`; now reason-bearing, and the decision sees the current `downstream` so the in-degree can be enforced.
- Impls:
  - `AcceptFromAllCandidates` (existing, default) — `Accept` when membership-valid, else `RejectMembership`; never `RejectOverCapacity`.
  - `BoundedAcceptance { in_degree }` (new) — `RejectMembership` if not membership-valid; else `RejectOverCapacity` if downstream count ≥ `in_degree`; else `Accept`.
- Handler mapping: `Accept` → downstream insert + `Accepted`; `RejectMembership` → silent drop; `RejectOverCapacity` → drop (distinct cause) + send `Rejected`.

## Construction / wiring

- Strategies are constructed at the edge from already-parsed values (seed, out-degree, in-degree) and injected at node construction (current shape; migrates to the argument shape with the parallel refactor). Bounded impls are wired only when the parameters are supplied; otherwise the unbounded defaults (FR-013, SC-005).
