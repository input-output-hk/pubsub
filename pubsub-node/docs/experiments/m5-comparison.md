# M5/M1 comparison — experiments framework vs the formal models (E8)

The model-family comparison for **M5 — directed k-in/k-out gossip** (each
node picks k_in pull forwarders and k_out push targets; every held message
flows over both link kinds) and its **M1 boundary** (k_in = 0: push-only,
every message over the k_out links), run on the publisher-pair machinery
of ADR 0041. Like the executed [M2 comparison](m2-comparison.md), it
**informs, it does not gate**; the statistical conventions are that
document's §4.

The mapping: `pick_count` is k_in (the relay seam), the publisher
sub-table's `pick_count` is k_out, fan-out is `forward-to-all`, and the
extracted propagation digraph is the union of relay and `Active` publisher
edges, deduplicated per pair. The model family's boundary reductions are
config points: `m1` names the k_in = 0 row (validated to require
`pick_count = 0`); the k_out = 0 row is the relay-only M2 shape, already
covered by the [M2 comparison](m2-comparison.md).

## Provenance

| | |
|---|---|
| Tool commit | `21acd36` (all cells) |
| Configurations | [`configs/experiments/comparisons/`](../../configs/experiments/comparisons/) — M5: `m5-n4k-4-4.toml` (seed 506, 500 runs), `m5-n4k-3-6.toml` (507, 1 500), `m5-n4k-6-3.toml` (508, 1 500), `m5-n4k-6-3-tight.toml` (601, 6 000), `m5-n4k-5-5.toml` (603, 2 000), `m5-n4k-6-6.toml` (604, 8 000), `m5-n20k-4-4.toml` (509, 200), `m5-n20k-op.toml` (703, 200); M1: `m1-n4k-f10.toml` (510, 1 000), `m1-n4k-f12.toml` (511, 2 000), `m1-n20k-f12.toml` (512, 400), `m1-n4k-f14.toml` (605, 4 000), `m1-n4k-f16.toml` (606, 8 000), `m1-n20k-op.toml` (704, 200) |
| Reference values | `formal_spec/hybrid_dissemination/models/{m5,m1}/properties/full_coverage.md` §3; `models/comparison.md` §2–§3 |
| Criterion match | both studies use a strong-connectivity check ⟺ one SCC of the union digraph |

## 1. M5 coverage law — P(bad) vs the published table (μ = 0.2)

| N | (k_in, k_out) | ours: bad/runs | P(bad) | Wilson 95 % | law | formal MC | z vs MC |
|---|---|---|---|---|---|---|---|
| 4 000 | (4,4) | 172/500 | 0.3440 | [0.304, 0.387] | 0.340 | 187/500 = 0.374 | −0.99 |
| 4 000 | (3,6) | 311/1500 | 0.2073 | [0.188, 0.229] | 0.204 | 306/1500 = 0.204 | +0.23 |
| 4 000 | (6,3) | 337/1500 | 0.2247 | [0.204, 0.247] | 0.204 | 285/1500 = 0.190 | +2.34 |
| 4 000 | (6,3), 6 000 runs | 1240/6000 | 0.2067 | [0.197, 0.217] | 0.204 | — | +1.43 |
| 4 000 | (5,5) | 70/2000 | 0.0350 | [0.028, 0.044] | 0.0364 | 63/2000 = 0.0315 | +0.62 |
| 4 000 | (6,6) | 28/8000 | 0.0035 | [0.0024, 0.0051] | 0.0033 | 31/8000 = 0.0039 | −0.39 |
| 20 000 | (4,4) | 175/200 | 0.8750 | [0.822, 0.914] | 0.876 | 172/200 = 0.860 | +0.44 |

**The swap symmetry** (k_in, k_out) ↔ (k_out, k_in) is a structural
prediction — the two cells share one law value, 0.204. Both teams' samples
fluctuate around it: theirs 0.204/0.190, ours 0.207/0.225, in opposite
directions; the independent 6 000-run (6,3) sample settles ours at 0.2067
with the law comfortably inside a ±0.010 interval. Every other law value
sits inside its Wilson interval directly.

## 2. M1 coverage law — the k_in = 0 boundary (μ = 0.2)

| N | F | ours: bad/runs | P(bad) | Wilson 95 % | law | formal MC | z vs MC |
|---|---|---|---|---|---|---|---|
| 4 000 | 10 | 637/1000 | 0.6370 | [0.607, 0.666] | 0.655 | 660/1000 = 0.660 | −1.08 |
| 4 000 | 12 | 388/2000 | 0.1940 | [0.177, 0.212] | 0.192 | 372/2000 = 0.186 | +0.64 |
| 20 000 | 12 | 255/400 | 0.6375 | [0.589, 0.683] | 0.661 | 268/400 = 0.670 | −0.97 |
| 4 000 | 14 | 172/4000 | 0.0430 | [0.037, 0.050] | 0.0420 | 165/4000 = 0.0413 | +0.39 |
| 4 000 | 16 | 72/8000 | 0.0090 | [0.0072, 0.0113] | 0.0086 | 82/8000 = 0.0103 | −0.81 |

Every law value inside the Wilson interval, both sizes, including the
H-scaling row (same F, ×5 N). These cells ran with a **literally empty
relay mesh**: `sends_by_kind.relay` ≡ 0 on every row — the boundary
reduction visible in the accounting, the mirror of the relay-only models'
zero publisher column.

## 3. Cost and latency — the P(bad) ≤ 10⁻⁴ operating points

200 runs each, all good with full coverage:

**M5 (k_in, k_out) = (9, 8)**

| quantity | published | measured |
|---|---|---|
| honest→honest sends per message | 217 562 | **217 529.7** (−0.015 %) |
| copies per honest node | 13.6 | **13.60** |
| hops, full coverage | 5.0 | **5.00** |
| hops, mean first receipt | 3.9 | **3.97** |

The kind split decomposes the two mechanisms: relay-kind (pull-serving)
143 979.0 vs publisher-kind (push) 127 936.8 sends per message (all
recipient classes) — ratio 1.1254, the k_in : k_out = 9 : 8 = 1.125 split
to four digits. The mean-hops figure sits 0.07 above the published one
decimal (ours rounds to 4.0 vs their 3.9); every other quantity matches at
table precision, and hops-to-full-coverage is exact.

**M1 (F = 24)**

| quantity | published | measured |
|---|---|---|
| honest→honest sends per message | 307 202 | **307 200.8** (−0.0004 %) |
| copies per honest node | 19.2 | **19.20** |
| hops, full coverage | 5.0 | **5.00** |
| hops, mean first receipt | 3.6 | **3.61** |

All sends publisher-kind, as the boundary requires.

## 4. Degrees

From the extracted union digraphs at the operating points: M5 mean
in-degree = mean out-degree = **13.60** (published 13.6/13.6 =
(k_in + k_out)·(1−μ)), max observed 31 vs their 33. M1 mean **19.20**
(published 19.2/19.2), max in 46 vs their 41 (ours over 200 graphs vs
their 25 — the balls-in-bins maximum grows with sample size), max
out-degree 24 = F exactly (a node pushes over at most its own picks).
