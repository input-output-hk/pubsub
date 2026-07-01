# 0024 — Seeded deterministic bounded peer selection

**Status**: Accepted

**Context**: Feature 005 (`specs/005-peer-view/`) replaces the full-mesh `ConnectToAllCandidates` dial policy with a bounded one so dissemination experiments have a non-trivial topology. Selection must be (a) bounded to a uniform upstream degree, (b) reproducible from a recorded seed across runs and machines (FR-003), (c) a pure decision drawing no randomness during a state transition (FR-009), (d) per-node diverse from a single network seed (FR-005), and (e) unbiased across candidate identities over a seed sweep (FR-007).

## Decision

Select by **deterministic keyed-hash ranking**, not a stateful PRNG. For each joined topic, rank candidate peers by a SHA-256 digest of a canonical, length-prefixed encoding of `(domain-tag, seed, self_id, topic, candidate_id)` and take the lowest `upstream_degree`, breaking ties on `candidate_id`. The strategy struct `SeededBoundedSelection { seed, self_id, upstream_degree }` carries the seed and identity as fixed fields set at construction, so `expected_upstream` stays a pure function of its inputs.

- **Digest**: SHA-256 (reusing the in-tree `sha2` dependency that `MessageHash` already uses) — explicitly **not** `std::hash::DefaultHasher`, which is unspecified and not stable across platforms/compiler versions (a cross-machine-reproducibility defect, Principle I).
- **Domain tag**: the hash is domain-separated by the strategy's own unique byte-string, `ConnectionStrategyKind::SeededBounded.tag()`, so distinct strategies never share a hash domain. Strategy selection is a readable, case-insensitive **`ConnectionStrategyKind`** enum (`connect-to-all` / `seeded-bounded`) parsed at the edge — not an implicit "params present ⇒ bounded" rule — and each variant carries that predefined unique tag.
- **Seed scope**: one **network** seed, folded with `self_id` for per-node diversity (FR-005). A `u64` supplied at startup; **default 0** when absent (FR-004).
- **Tie-break**: secondary order on `candidate_id` so equal-ranked candidates resolve identically every run (FR-008), never on incidental iteration order.

## Ordered structures (FR-017)

The new state this feature introduces uses ordered structures for reproducibility: `failed_upstream` is a `BTreeSet<(PeerId, TopicId)>`. This required adding `Ord`/`PartialOrd` to `PeerId` (it wraps `PublicKey`, which already derives `Ord`) — a small additive change. The `ConnectionStrategy` return type stays `HashSet<(PeerId, TopicId)>`: its *membership* is deterministic (the same ranked top-k), and the realized connection set is order-independent, so reproducibility holds without changing the trait.

## Back-fill (sticky failed-set, `ConnectionSetup`-driven)

A dial rejected for over-capacity marks the peer **failed for the run** (sticky, no retry). Selection runs over the *viable* view (`candidates` minus `failed_upstream`), so re-invoking the existing `ConnectionSetup` event re-selects the next-ranked candidate — back-fill falls out of recomputation, with no new round/timer event (FR-014). There is no timeout/no-response path: "rejected" is always an explicit over-capacity signal.

## Consequences

- Reproducible by construction: same `(seed, self_id, topic, viable candidates)` → identical selection (FR-003, SC-001).
- `apply` stays pure (no RNG state, no clock) — FR-009.
- Selection is pseudo-uniform, validated statistically over a seed sweep (FR-007/SC-004) rather than asserted exactly.

## Alternatives rejected

- **Stateful seeded PRNG** in the strategy — non-deterministic across calls on an evolving candidate set; hidden state in the transition.
- **`DefaultHasher`** — not portable/stable; would silently break FR-003 cross-machine.
- **Resetting the failed-set** (per `ConnectionSetup` or on membership change) — oscillation / lifecycle coupling; rejected per Clarifications (sticky chosen).
