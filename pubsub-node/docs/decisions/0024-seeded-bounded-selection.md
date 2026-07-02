# 0024 — Seeded deterministic bounded peer selection

**Status**: Accepted

**Context**: Feature 005 (`specs/005-peer-view/`) replaces the full-mesh `ConnectToAllCandidates` dial policy with a bounded one so dissemination experiments have a non-trivial topology. Selection must be (a) bounded to a uniform upstream degree, (b) reproducible from a recorded seed across runs and machines (FR-003), (c) a pure decision drawing no randomness during a state transition (FR-009), (d) per-node diverse from a single network seed (FR-005), and (e) unbiased across candidate identities over a seed sweep (FR-007).

## Decision

Select by **deterministic keyed-hash ranking**, not a stateful PRNG. For each joined topic, rank candidate peers by a SHA-256 digest of a canonical, length-prefixed encoding of `(domain-tag, seed, self_id, topic, candidate_id)` and take the lowest `upstream_degree`, breaking ties on `candidate_id`. The strategy struct `SeededBoundedConnection { seed, self_id, upstream_degree }` carries the seed and identity as fixed fields set at construction, so `expected_upstream` stays a pure function of its inputs.

- **Digest**: SHA-256 (reusing the in-tree `sha2` dependency that `MessageHash` already uses) — explicitly **not** `std::hash::DefaultHasher`, which is unspecified and not stable across platforms/compiler versions (a cross-machine-reproducibility defect, Principle I).
- **Domain tag**: the hash is domain-separated by the strategy's own unique byte-string, `ConnectionStrategyKind::SeededBounded.tag()`, so distinct strategies never share a hash domain. Strategy selection is a readable, case-insensitive **`ConnectionStrategyKind`** enum (`connect-to-all` / `seeded-bounded`) parsed at the edge — not an implicit "params present ⇒ bounded" rule — and each variant carries that predefined unique tag.
- **Seed scope**: one **network** seed, folded with `self_id` for per-node diversity (FR-005). A `u64` supplied at startup; **default 0** when absent (FR-004).
- **Tie-break**: secondary order on `candidate_id` so equal-ranked candidates resolve identically every run (FR-008), never on incidental iteration order.

## Reproducibility without new ordered state (FR-017)

This feature adds **no new persistent connection state** (the earlier `failed_upstream` set was removed together with back-fill — see below). Reproducibility does not depend on any container's iteration order: `expected_upstream` ranks each candidate by *its own* keyed hash, so the selected **membership** is a pure function of the candidate set regardless of order, and the return type stays `HashSet<(PeerId, TopicId)>`. (`PeerId` keeps the `Ord`/`PartialOrd` derive added earlier — harmless, though no longer required by this feature.)

## Rejection handling (no back-fill)

A dial rejected for over-capacity causes the dialer to remove the matching pending `AwaitingAccept` upstream — that is the **only** handling. There is no retry, no back-fill, and no failed-peer set; the realized upstream degree may settle below target, and re-forming connections is deferred to the future heartbeat/reshuffle layer. A retry-to-a-minimum policy is a separate future strategy family (`BackfillingSeededBoundedConnection`). "Rejected" is always an explicit over-capacity signal; there is no timeout/no-response path.

> An earlier revision of this feature added a sticky failed-set + `ConnectionSetup`-driven back-fill; it was removed in the PR-73 simplification to start from the no-retry baseline (spec Clarifications, Session 2026-07-02).

## Consequences

- Reproducible by construction: same `(seed, self_id, topic, candidates)` → identical selection (FR-003, SC-001).
- `apply` stays pure (no RNG state, no clock) — FR-009.
- Selection is pseudo-uniform, validated statistically over a seed sweep (FR-007/SC-004) rather than asserted exactly.

## Alternatives rejected

- **Stateful seeded PRNG** in the strategy — non-deterministic across calls on an evolving candidate set; hidden state in the transition.
- **`DefaultHasher`** — not portable/stable; would silently break FR-003 cross-machine.
- **In-feature back-fill / sticky failed-set** — an earlier revision added it; removed for simplicity so the no-retry baseline is observed first. Retry/back-fill is a separate future strategy family.
