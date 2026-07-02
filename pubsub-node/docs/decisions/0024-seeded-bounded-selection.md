# 0024 — Seeded deterministic bounded peer selection

**Status**: Accepted

**Context**: Feature 005 (`specs/005-peer-view/`) replaces the full-mesh `ConnectToAllCandidates` dial policy with a bounded one so dissemination experiments have a non-trivial topology. Selection must be (a) bounded to a uniform upstream degree, (b) reproducible from a recorded seed across runs and machines (FR-003), (c) a pure decision drawing no ambient randomness during a state transition (FR-009), (d) per-node diverse from a single network seed (FR-005), and (e) unbiased across candidate identities over a seed sweep (FR-007).

## Decision

Select by **seeded pseudo-random sampling over a canonically-ordered candidate set** — not by keyed-hash ranking of peers, and not with a persistent/stateful generator.

- **PRNG**: `rand_chacha::ChaCha20Rng` (reusing the in-tree `rand` + `rand_chacha` that `crypto::mock` already depends on — no new dependency). ChaCha20 is a **fixed algorithm**, so its stream is identical across platforms and library versions — explicitly **not** `rand`'s `StdRng`, whose algorithm is unspecified and not reproducible across versions (the cross-machine-reproducibility requirement, Principle I).
- **Sampling**: for each joined topic, collect the candidates into a `Vec` and take a uniform `upstream_degree`-subset with a partial Fisher–Yates (`rand::seq::SliceRandom::partial_shuffle`). When there are fewer candidates than the bound, all are selected (FR-002).
- **Determinism via ordered inputs**: the candidates arrive in **canonical sorted order** because `NodeState` and the `ConnectionStrategy` trait carry them as `BTreeMap<TopicId, BTreeSet<PeerId>>` / `BTreeSet<TopicId>` (FR-017). The sampled set is therefore a pure function of the *set*, independent of any iteration order — no per-peer hashing is needed to impose order.
- **Per-call seeding**: the PRNG is re-seeded per `(seed, self_id, topic)` — no cursor is carried across calls, so `expected_upstream` stays a pure `&self` function with no hidden state (FR-009/FR-018). The 32-byte `ChaCha20Rng` seed is derived with **SHA-256** over a canonical, length-prefixed encoding of `(domain-tag, seed, self_id, topic)`. SHA-256 (the in-tree `sha2`, already used by `MessageHash`) is used **only as a key-derivation step for the PRNG seed** — peers are picked by the PRNG, not ranked by hash. Folding `self_id` in gives per-node diversity from the single network seed (FR-005); the domain tag (`ConnectionStrategyKind::SeededBounded.tag()`) keeps distinct strategies in distinct seed domains.

## Ordered structures (FR-017)

Reproducibility now depends on ordered inputs: `subscriptions` and `candidates` are `BTreeSet`/`BTreeMap` on `NodeState` and in the trait signature, and the strategy returns `BTreeSet<(PeerId, TopicId)>`. A `BTreeSet` iterates in sorted order, so the sampler collects it to a `Vec` and shuffles — the canonical order is structural, not re-derived. (`downstream` stays a `HashSet` — the acceptance policy *counts* it, order-independent; it converts to `BTreeSet` when 015's fan-out samples over it.)

## Rejection handling (no back-fill)

A dial rejected for over-capacity causes the dialer to remove the matching pending `AwaitingAccept` upstream — that is the **only** handling. There is no retry, no back-fill, and no failed-peer set; the realized upstream degree may settle below target, and re-forming connections is deferred to the future heartbeat/reshuffle layer. A retry-to-a-minimum policy is a separate future strategy family (`BackfillingSeededBoundedConnection`). "Rejected" is always an explicit over-capacity signal; there is no timeout/no-response path.

> An earlier revision of this feature added a sticky failed-set + `ConnectionSetup`-driven back-fill; it was removed in the PR-73 simplification to start from the no-retry baseline (spec Clarifications, Session 2026-07-02).

## Consequences

- Reproducible by construction: same `(seed, self_id, topic, candidates)` → identical sample (FR-003, SC-001), because the algorithm is fixed and the inputs are canonically ordered.
- `apply` stays pure (the PRNG is local to the call, re-seeded from the strategy's `seed` field; no ambient RNG, no clock) — FR-009.
- Sampling is exactly uniform over `upstream_degree`-subsets (partial Fisher–Yates), validated statistically over a seed sweep (FR-007/SC-004).
- FR-017's ordered structures become load-bearing (they carry the canonical order the sampler consumes), not merely a reproducibility nicety.

## Alternatives rejected

- **Keyed-hash ranking of peers** (rank each candidate by `hash(seed, self_id, topic, candidate)`, take lowest-k) — the previous revision of this decision. Replaced (this PR) with seeded PRNG sampling: the selection is genuinely random rather than an artefact of a digest ordering, and determinism is instead guaranteed by fixing the PRNG algorithm and ordering the inputs. SHA-256 is retained only to derive the PRNG seed.
- **A persistent/stateful PRNG** carried on the strategy — hidden mutable state in the transition; non-idempotent across calls on an evolving set. Rejected in favour of per-call re-seeding.
- **`rand`'s `StdRng`** — not reproducible across versions; `ChaCha20Rng` is the fixed-algorithm choice. **`DefaultHasher`** for the seed KDF — not portable/stable; SHA-256 is used instead.
- **In-feature back-fill / sticky failed-set** — an earlier revision added it; removed for simplicity so the no-retry baseline is observed first. Retry/back-fill is a separate future strategy family.
