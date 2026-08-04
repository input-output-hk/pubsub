# M3 comparison — experiments framework vs the formal M3 model (E6)

The model-family comparison for **M3 — pull relaying plus standing
initiation links** (M2's relay mesh plus s−1 seeding links per node,
carrying only their owner's own publications), run on the publisher-pair
machinery of ADR 0041. Like the executed [M2 comparison](m2-comparison.md),
it **informs, it does not gate**; the statistical conventions are that
document's §4.

The parameter mapping: the model's `s` counts the publisher itself, so the
honest class's publisher sub-table carries `pick_count = s − 1`. The
instrument's goodness is **seed-aware** for M3 (ADR 0041): per potential
publisher, the downward closure of its seed set — itself plus its `Active`
initiation targets — over the relay-edge condensation must cover the whole
graph. This is the formal study's own criterion ("a message from p spreads
from {p} ∪ (p's honest initiation targets) over the pull relay edges;
initiation links never relay"), computed exactly, per publisher.

## Provenance

| | |
|---|---|
| Tool commit | `21acd36` (all cells) |
| Configurations | [`configs/experiments/comparisons/`](../../configs/experiments/comparisons/) — `m3-n4k-rf6-s4.toml` (seed 503, 600 runs), `m3-n4k-rf8-s3.toml` (504, 1 000), `m3-n20k-rf8-s3.toml` (505, 300), `m3-n4k-rf10-s4.toml` (602, 8 000), `m3-n4k-rf9-s5-tail.toml` (608, 30 000), `m3-n20k-op.toml` (701, 200) |
| Reference values | `formal_spec/hybrid_dissemination/models/m3/properties/full_coverage.md` §3; `models/comparison.md` §2–§3 (M3 rows) |

## 1. Coverage law — P(bad) vs the published table (μ = 0.2)

| N | RF | s | ours: bad/runs | P(bad) | Wilson 95 % | law | formal MC | z vs MC |
|---|---|---|---|---|---|---|---|---|
| 4 000 | 6 | 4 | 213/600 | 0.3550 | [0.318, 0.394] | 0.337 | 209/600 = 0.348 | +0.24 |
| 4 000 | 8 | 3 | 215/1000 | 0.2150 | [0.191, 0.242] | 0.197 | 195/1000 = 0.195 | +1.11 |
| 20 000 | 8 | 3 | 209/300 | 0.6967 | [0.642, 0.746] | 0.668 | 194/300 = 0.647 | +1.30 |
| 4 000 | 10 | 4 | 75/8000 | 0.0094 | [0.0075, 0.0117] | 0.0088 | 74/8000 = 0.0092 | +0.08 |
| 4 000 | 9 | 5 | 150/30000 | 0.0050 | [0.0043, 0.0059] | 0.0053 | 178/30000 = 0.0059 | −1.55 |

Every law value sits inside the Wilson interval, across two network sizes
and 2.5 orders of magnitude in P(bad). On the deep-tail row the formal
study notes its law runs ~1.11× optimistic (second-order small
components); their MC sample landed above the law and ours below it — a
±1.5σ straddle between two independent 30 000-run samples, law-consistent
from both sides.

## 2. Cost and latency — the P(bad) ≤ 10⁻⁴ operating point (RF = 12, s = 8)

200 runs, all good with full coverage (consistent with the published
7.8×10⁻⁵):

| quantity | published (comparison.md, M3 row) | measured (mean over 200 runs) |
|---|---|---|
| honest→honest sends per message | 153 570 | **153 577.2** (+0.005 %) |
| copies per honest node | 9.6 | **9.60** |
| hops, full coverage | 5.9 | **5.87** |
| hops, mean first receipt | 4.3 | **4.31** |

**The seeding/relaying split, measured.** The per-link-kind send columns
(ADR 0041) decompose each message's traffic: **publisher-kind sends =
7.0 per message, exactly s − 1** — every publisher seeds each publication
over its s−1 initiation links, once, and they carry nothing else — against
~192 000 relay-kind sends (all recipient classes). This is the model's
"initiation links attack exactly the muted-publisher defect, at ~zero
bandwidth" measured directly: the healing mechanism costs 7 sends in
~2×10⁵.

## 3. Degrees

The extracted **relay** digraph over the operating-point runs: mean
in-degree = mean out-degree = **9.60** = RF·(1−μ), with max in-degree 12 —
bounded by RF exactly (picks without replacement). The published degree
table's 15.2/15.2 counts initiation links too (12 + 7 links per node,
honest side); the instrument's per-node degree histograms deliberately
cover the propagation digraph only — initiation links are seed edges, not
propagation edges — so the initiation-degree axis is checked at the
aggregate level instead (the exact s−1 seeding sends above, and 017's
`tests/model_family.rs` degree evidence).

## 4. What the seed-aware goodness changed

Bare one-SCC (the M2 criterion) would misclassify M3's healed topologies:
a muted publisher — a relay-sink node — is exactly what the initiation
links repair, and that repair is why every P(bad) row above lands on the
law. The worked examples in `src/experiments/graph.rs` pin both directions:
seeding heals the muted publisher (bad under M2's verdict, good under
M3's, same relay digraph), and an in-isolated node stays bad — initiation
links deliver only their owner's publications, so no seeding can supply a
node that cannot receive (the law's eclipse floor carried by RF alone).
