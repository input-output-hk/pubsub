# 0024 — Verifiable hash-gated (bucketed-pull) connection selection

**Status**: Accepted (supersedes the earlier "seeded bounded selection" decision recorded under this number). **Amended by ADR 0033/0034/0035** (feature 015): the predicate gained per-role domain tags and a symmetric mode; `HashGatedConnection` merged into the role-parameterised `HashGatedSelection`. The predicate mechanics and B-derivation stand.

**Context**: Feature 005 replaces the full-mesh `ConnectToAllCandidates` dial policy with a bounded one so dissemination has a non-trivial topology **and** a spam-resistant admission rule. Requirements: (a) bounded degree, (b) reproducible across machines, (c) pure (no ambient RNG / wall-clock in a transition), (d) per-node diverse, (e) **verifiable** — the acceptor must be able to confirm a request is legitimate without trusting the dialer, so an adversary cannot exhaust a victim's serving slots by spamming (the attack in `docs/extensions/bucketed-pull.md`).

This supersedes two earlier revisions of this decision (keyed-hash *ranking*, then seeded-PRNG *sampling*): neither was verifiable, and the formal model (bucketed-pull) is a per-round hash-bucket **predicate**, not a sample of a candidate list.

## Decision

Select by the **verifiable per-round hash-bucket predicate** of `docs/extensions/bucketed-pull.md`.

- **Predicate** (`strategies::edge::is_valid_edge`): a directional edge `requester → candidate` on topic `T` at interval `I` is valid iff
  `H(genesis, T, requester, candidate, I) mod B == 0`.
  `H` = SHA-256 over a canonical, length-prefixed encoding (in-tree `sha2`, cross-machine stable — explicitly **not** `DefaultHasher`). Ordered `(requester, candidate)` so it is directional. The predicate is a pure function of public values, so **both peers compute it** — the dial side to select, the accept side to verify.
- **Bucket count**: `B = max(1, round(|candidates_on_topic| / target_degree))`. Expected edges per topic = `|candidates| / B ≈ target_degree`.
- **Fixed `target_degree`** (the target connection degree): a configured constant applied uniformly for the run, **not** derived from network size (Denis's conservative option). This makes small topics self-handle — when `|candidates| ≤ ~target_degree`, `B` floors to 1 and `mod 1 == 0` always holds, so the node connects to **all** candidates (the graceful small-topic degradation the doc describes) — with no `ln`-based degeneracy (`ln(2)=0`) and no network-size estimation.
- **View**: the model samples within a per-peer view `H_v`. **v1 uses `view = the full candidate set`** (no discovery-layer sampling); `B` derives from the full per-topic candidate count. The seam is shaped so a later discovery/experiment layer can sub-sample `H_v` before the predicate without a seam change.
- **Interval** is an input (from `Heartbeat` — ADR 0030), not a field; `genesis` is a strategy field. So `HashGatedConnection { genesis, self_id, target_degree }` stays pure and reproducible.

`ConnectionStrategy::expected_upstream` gains the `interval` argument; the concrete policy is `HashGatedConnection`, replacing `SeededBoundedConnection`.

## Consequences

- **Verifiable + spam-resistant**: an id satisfies the predicate for a given `(victim, interval)` with probability `1/B`; sharing a descriptor across sybils yields only the `1/B` honest density — no amplification (`bucketed-pull.md` §Concentration). The acceptor confirms by recomputing one hash (ADR 0025).
- Reproducible by construction (fixed hash + fixed arithmetic + ordered inputs); `apply` stays pure.
- Order-independent: the predicate is evaluated per candidate, so the selected *set* does not depend on iteration order (ordered structures are kept only for deterministic effect emission, FR-014).
- Small topics connect-to-all automatically; no special-case threshold.

## Alternatives rejected

- **Keyed-hash ranking** / **seeded-PRNG sampling** (earlier revisions of this ADR) — not verifiable; an acceptor cannot confirm a sampled edge, so they give no spam resistance. Replaced by the bucket predicate.
- **Degree derived from `ln(N)`** — `ln(2)=0` degeneracy + needs network-size estimation; the fixed-`target_degree` bucketing handles small topics without either.
- **`DefaultHasher`** — not portable/stable; would break cross-machine reproducibility. **SHA-256** is used.
- **In-feature back-fill / sticky failed-set** — removed earlier; retry is a future strategy family (see ADR 0025, N-029).
- **Discovery-layer view sampling now** — deferred; v1 uses the full candidate set as the view.
