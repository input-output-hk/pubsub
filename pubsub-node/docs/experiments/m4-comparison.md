# M4 comparison — experiments framework vs the formal M4 model (E7)

The model-family comparison for **M4 — bidirectional RF-out gossip** (every
node picks RF peers, each pick a bidirectional link), run on the
publisher-pair machinery of ADR 0041. Like the executed
[M2 comparison](m2-comparison.md), it **informs, it does not gate**; the
statistical conventions (raw counts + Wilson 95 %, and why not ±1σ) are
documented there in §4 and apply unchanged.

## Provenance

| | |
|---|---|
| Tool commit | `21acd36` (the RF = 6 cell at `2102b6f`, a docs-only successor — the instrument binary is unchanged between them) |
| Configurations | [`configs/experiments/comparisons/`](../../configs/experiments/comparisons/) — `m4-n20k-rf3.toml` (seed 501, 200 runs), `m4-n20k-rf4.toml` (seed 502, 1 000), `m4-n20k-rf5.toml` (seed 607, 8 000), `m4-n20k-rf6-tail.toml` (seed 705, 30 000), `m4-n20k-op.toml` (seed 702, 200); plus the shipped structure point [`m4-uniform-symmetric.toml`](../../configs/experiments/m4-uniform-symmetric.toml) (N = 4 000, seed 44, 500 runs, baseline `d7e7132`) |
| Reference values | `formal_spec/hybrid_dissemination/models/m4/properties/full_coverage.md` §3; `models/comparison.md` §2–§3 (M4 rows) |
| Criterion match | the model's *good* ⟺ the honest-induced undirected subgraph is connected; the instrument's symmetric digraph is mirrored by construction, so one SCC is the same predicate |

Artifacts are reproduced byte-for-byte from the tool commit and master
seeds (worker count is a wall-clock choice, not part of the contract).

## 1. Coverage law — P(bad) vs the published table (N = 20 000, μ = 0.2)

| RF | ours: bad/runs | P(bad) | Wilson 95 % | law | formal MC | z vs MC |
|---|---|---|---|---|---|---|
| 3 | 200/200 | 1.0000 | [0.981, 1.000] | ≈ 1.000 | 2000/2000 | +0.00 |
| 4 | 624/1000 | 0.6240 | [0.594, 0.654] | 0.647 | 2585/4000 = 0.646 | −1.31 |
| 5 | 719/8000 | 0.0899 | [0.084, 0.096] | 0.0893 | 749/8000 = 0.0936 | −0.82 |
| 6 | 251/30000 | 0.00837 | [0.0074, 0.0095] | 0.00836 | 260/30000 = 0.00867 | −0.40 |

Every law value sits inside the Wilson interval, across three orders of
magnitude in P(bad). The RF = 3 row's "law 1.000" is the published table's
rounding of 1 − e^(−11.6); an all-bad sample is exact agreement (a Wilson
interval never contains exactly 1 at finite n). On the deep-tail row the
formal study expects the law to run mildly optimistic (~1.1× second-order
small-component under-count, measured at RF = 7); at RF = 6 the two
30 000-run samples land at 1.00× (ours) and 1.04× (theirs) of the law —
the correction is within sampling noise at this depth.

## 2. Cost and latency — the P(bad) ≤ 10⁻⁴ operating point (RF = 8)

200 runs, all good with full coverage (consistent with the published
6.8×10⁻⁵ at this point):

| quantity | published (comparison.md, M4 row) | measured (mean over 200 runs) |
|---|---|---|
| honest→honest sends per message | 188 795 | **188 750.6** (−0.024 %) |
| copies per honest node | 11.8 | **11.80** |
| hops, full coverage | 5.1 | **5.12** |
| hops, mean first receipt | 4.1 | **4.09** |

The sends deviation is within a 200-run mean's sampling noise (the
published value was itself measured over 40–200 graphs).

## 3. Degrees

From the extracted up-honest digraph over the 200 operating-point runs:
mean in-degree = mean out-degree = **12.80**, matching the published
12.8/12.8 (= 2·RF·(1−μ)) exactly; per-graph in- and out-histograms are
identical — the constructed reciprocity, fleet-wide. Max observed degree
**30** vs the published 29 (theirs over 25 graphs, ours over 200 — the
balls-in-bins maximum grows with the sample). The whole-population
minimum-degree ≥ RF floor is separately evidenced in
`tests/model_family.rs` and the `d7e7132` structure baseline.

## 4. Reading the kind columns

M4 is relay-only: the run records' `sends_by_kind.publisher` column is
identically zero across all cells — the degenerate-column convention
(ADR 0041), and a per-row structural check that no initiation-link
machinery leaked into this model's runs.
