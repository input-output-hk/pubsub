# Model comparison — bandwidth and latency at P(bad) ≤ 10⁻⁴ with a 2 % disturbance margin

This document compares the five dissemination models at one shared
operating point:

- **N = 20 000, μ = 0.2** (H = 16 000 honest nodes, 4 000 silent
  adversaries).
- **Full-coverage criterion** ([README](README.md)): a sampled graph is
  good if, and only if, every message of every honest publisher reaches
  all other honest nodes. Only standing per-epoch structure counts.
- **P(bad graph) ≤ 10⁻⁴ per epoch, held with a disturbance margin**: a
  point is admissible if, and only if, its coverage law keeps
  P(bad) ≤ 10⁻⁴ on the full interval μ_eff ≤ μ + Δμ_margin, with
  **Δμ_margin = 0.016**. The churn/loss reading of this margin is
  p_max = Δμ/(1−μ) ≥ 2 % (§4). We read the margin from the law. For
  M3, M4 and M5, churn tests validate the law at exactly the selected
  parameters
  ([churn-proposed-points](../../../pubsub-node/docs/experiments/churn-proposed-points.md),
  [churn-tolerance](../../../pubsub-node/docs/experiments/churn-tolerance.md)).
- **Dominance selects the points, not an objective function**: each
  model appears at its admissible points that are Pareto-minimal on
  two axes (msgs / message, mean standing links). We discard a point
  only if a second admissible point of the same model is equal or
  better on both axes and better on one. Latency is reported but does
  not prune points (the field spans less than one hop, §2). Robustness
  above the margin is reported (§4) but does not prune points. As an
  axis, robustness keeps every larger parameter alive. At the current
  numbers, each model's Pareto set is a single point.
- The count conventions are the same as everywhere in this folder:
  fire-once relaying, and no resend on the arrival link. Transmissions
  are honest→honest copies, with duplicates included.

All values below summarize each model's `properties/` files. Each
model's `scripts/` measure them at the selected operating points
(25–400 graphs per cell, with the seeds recorded per file).
Monte-Carlo runs in each model's
[`full_coverage.md`](m1/properties/full_coverage.md) validate the
coverage laws behind the parameter choices. The pubsub-node instrument
independently measured the M3 and M4 points
([m3-comparison §5](../../../pubsub-node/docs/experiments/m3-comparison.md)
/ [m4-comparison §5](../../../pubsub-node/docs/experiments/m4-comparison.md),
200 graphs each). The instrument and the models agree within 0.05 % on
every shared quantity.

## 1. Selected operating points per model

| model | mechanism | selected parameters | why these (dominance) | P(bad) | margin p_max |
|---|---|---|---|---|---|
| [M1](m1/properties/README.md) | push | F = 25 | Smallest admissible F. F = 24 holds p_max ~1.8 %. | 3.3×10⁻⁵ | ~5.9 % |
| [M2](m2/properties/README.md) | pull | RF = 25 | Smallest admissible RF. RF = 24 holds ~1.7 %. | 3.3×10⁻⁵ | ~5.8 % |
| [M3](m3/properties/README.md) | pull + initiation links | RF = 13, s = 7 | Every split of budget RF+(s−1) = 19 holds the same 38 links. Thus dominance selects the cheapest admissible split. (12, 8) is inadmissible (~0.5 %). Budgets ≥ 20 are dominated. | 4.4×10⁻⁵ | ~2.2 % |
| [M4](m4/properties/README.md) | undirected flood | RF = 9 | Smallest admissible RF. RF = 8 holds ~1.1 %. | 6.1×10⁻⁶ | ~7.4 % |
| [M5](m5/properties/README.md) | directed k_in/k_out | (k_in, k_out) = (9, 8) | Most-balanced split of the smallest budget k_in+k_out = 17. M5 is the one model whose δ-cheapest point already clears the margin. | 4.4×10⁻⁵ | ~2.2 % |

**Why a margin.** The natural criterion selects each model at the
cheapest parameters that meet δ alone. We name this point the
**δ-cheapest** point and use the term through this document. By
construction, this point sits nearest the failure cliff. Thus a
comparison made there measures the selection rule, not the mechanisms.
For example, M3's δ-cheapest split (12, 8) holds a churn margin of
~0.5 %. The same-budget re-split (13, 7) holds ~2.2 % at identical
state (§5C).

We put the margin in μ_eff and not in a factor on δ, for two reasons.
First, a δ-factor k buys a μ-headroom of only ln k / sensitivity. The
steepest laws (d ln E/dμ ≈ 60 at M3's δ-cheapest split, ≈ 50 at M5's)
then get the least real protection exactly where the cliff is
sharpest. Second, Δμ is the operational currency. One number bounds
honest churn, an underestimate of μ itself, and per-epoch send loss at
the same time (the μ_eff identity, §§4, 7).

**The bar.** Δμ_margin = 0.016 (p_max 2 %) is anchored to the
disturbances that this folder already treats as realistic: WAN send
loss of ~1 % (§7) plus honest churn of the same order. The bar is not
tuned to the selections. Every bar in (1.1 %, 1.7 %] admits M1/M2's
δ-cheapest F/RF = 24 (p_max ~1.7–1.8 %) and rejects (12, 8) and
RF = 8. We rejected those bars, because a bar placed to keep rows
selects the criterion to fit the conclusion.

One caveat travels with M3's selection. Pooled across all churn
rounds, its law reads slightly optimistic
([churn-proposed-points §2](../../../pubsub-node/docs/experiments/churn-proposed-points.md),
Stouffer +2.41, mechanism unidentified).
[finite-n §6](../../../pubsub-node/docs/experiments/finite-n.md)
agrees along an independent axis. The direction is conservative: the
deviation can only decrease M3's 2.17 % toward the bar. If the
deviation becomes a real correction, examine M3's admissibility at
(13, 7) first.

**Provenance.** Every cell below traces to the owner model's
`properties/` file, measured at the selected parameters. Two
independent validation layers support the laws. The first layer is the
formal folder's own MC: elevated-μ_eff spot cells in each
`mu_shift_robustness.md`, and loss-injected cells in each
`transmission_unreliability.md`. The second layer, for M3, M4 and M5,
is the pubsub-node instrument's churn cells at exactly the selected
parameters. Those cells are 6/6 inside the Wilson intervals at
μ_eff = 0.36–0.48
([churn-proposed-points](../../../pubsub-node/docs/experiments/churn-proposed-points.md)).
The M5 cells are in the earlier round of
[churn-tolerance](../../../pubsub-node/docs/experiments/churn-tolerance.md).

## 2. The comparison

| model | parameters | msgs / message | copies / honest node | hops (full) | hops (mean) |
|---|---|---|---|---|---|
| **M3** | RF = 13, s = 7 | **166 428** | **10.4** | 5.5 | 4.2 |
| M4 | RF = 9 | 214 433 | 13.4 | 5.0 | 3.9 |
| M5 | (9, 8) | 217 562 | 13.6 | 5.0 | 3.9 |
| M1 | F = 25 | 319 974 | 20.0 | 4.9 | **3.6** |
| M2 | RF = 25 | 319 992 | 20.0 | **4.6** | **3.6** |

**Bandwidth: M3 wins decisively.** M3 is 22 % below M4, 24 % below M5,
and ~48 % below M1/M2. M4's lead over M5 is a small ~1.5 %.
**Latency: M2 wins, by a small amount.** The full field spans
~0.9 hops (4.6–5.5 hops to full coverage, or ~0.1–0.3 s at WAN per-hop
times of 100–300 ms). Bandwidth spans ~1.9×.

## 3. Node degrees (standing links per node)

The data comes from each model's
[`node_degrees.md`](m3/properties/node_degrees.md). The mean total
degree under protocol-compliant link opening is exactly 2× the nominal
budget, because every link has a chooser and an acceptor. The maximum
is a balls-in-bins tail on the accepted side. The table shows measured
values from 25 graphs:

| model | chosen (held, det.) | honest in / out (mean) | max observed | compliant total (mean) |
|---|---|---|---|---|
| **M4** | 9 | 14.4 / 14.4 (same links) | 34 | **18** |
| M5 | 9 in + 8 out | 13.6 / 13.6 | 33 | 34 |
| M3 | 13 in + 6 out | 15.2 / 15.2 | 38 accepted | 38 |
| M1 | 25 out | 20.0 / 20.0 | 45 | 50 |
| M2 | 25 in | 20.0 / 20.0 | 45 | 50 |

**On this axis the order flips: M4 wins decisively.** M4 holds
18 links per node. M3 holds 38 (2.1×), and M1/M2 hold 50 (2.8×). M4
and M5 also hold the smallest worst-case nodes (33–34, against 38–45).
M3's standing links follow the budget, not the split: every split of
budget 19 holds the same 38 links. The measurements for (12, 8) and
(13, 7) are identical
([node_degrees.md](m3/properties/node_degrees.md)).

Note that M3's degree is larger than its bandwidth rank shows: 12 of
its 38 links (the initiation kind) carry only their owner's
publications. These links are cheap in traffic, but they stay as held
state, connection slots, and churn surface. In M4 and M5, every held
link also carries relay traffic.

## 4. Degradation under μ-shift (frozen parameters)

The data comes from each model's
[`mu_shift_robustness.md`](m3/properties/mu_shift_robustness.md). We
freeze the operating points and sweep the effective adversarial
fraction upward. The values are read from the law, and MC runs
validate them at elevated μ. The table reports the **budget** (the
largest μ_eff that keeps P(bad) ≤ 10⁻⁴, with churn reading
p_max = Δμ/(1−μ)) and the **collapse point** (P(bad) = ½). A larger
value is better in all three data columns. The rows are sorted from
the best to the worst budget, and bold marks the best value in each
column. **M4 wins this section on the budget**, and M1/M2 win the
collapse point. M3, the bandwidth winner of §2, sits at the bar:

| model | parameters | budget μ_eff (Δμ) | churn p_max | collapse μ_eff |
|---|---|---|---|---|
| **M4** | RF = 9 | **0.259 (+0.059)** | **~7.4 %** | 0.55 |
| M1 | F = 25 | 0.247 (+0.047) | ~5.9 % | **0.62** |
| M2 | RF = 25 | 0.247 (+0.047) | ~5.8 % | **0.62** |
| M5 | (9, 8) | 0.217 (+0.017) | ~2.2 % | 0.49 |
| M3 | (13, 7) | 0.217 (+0.017) | ~2.2 % | 0.47 |

**Every selected point clears p_max ≥ 2 % by construction.** The
spread above the bar is itself informative: M4 holds ~3.7× the
required margin, M1/M2 hold ~2.9×, and M3/M5 sit at the bar. At the
δ-cheapest points, the robustness order is approximately the reverse
of the bandwidth order. M3's δ-cheapest split (12, 8) holds ~0.5 %,
and M4's RF = 8 holds ~1.1 %. This result disqualifies cheapest-point
selection: the position nearest the cliff is a property of that rule,
not of a mechanism. The re-split in §5C removes M3's apparent
brittleness at zero state cost.

Two properties here are genuine structure. The first is
**sensitivity**: d ln E/dμ ≈ 50 for M5 and ≈ 48 for M3, against ≈ 25
for M1/M2's exponential terms. (M3's μ^RF in-term alone runs at
RF/μ = 65. The 29 : 71 in : out defect split makes the mixture less
steep.) Thus the two bar-sitting models also erode fastest per unit of
unmodelled shift. The second is the **collapse cushion**: M1/M2
collapse at μ_eff ≈ 0.62, the latest in the family, against 0.47–0.55
for the rest. Their ~1.9× bandwidth buys this cushion.

The budgets are values read from the laws. Churn tests validate
M3/M4's laws at exactly these parameters (6/6 cells out to
μ_eff = 0.48). The formal elevated-μ MC in each
`mu_shift_robustness.md` validates M1/M2's laws. The budgets
themselves sit at P(bad) = 10⁻⁴ and are not directly sampleable at
feasible run counts
([churn-proposed-points §2](../../../pubsub-node/docs/experiments/churn-proposed-points.md)).

## 5. Re-provisioning — the robustness-adjusted frontier

Where §4 asks how a fixed deployment degrades as μ_eff increases, this
section asks the opposite question: the up-front cost to *provision
for* a higher μ. The data comes from each model's
[`re_provisioning.md`](m3/properties/re_provisioning.md). We invert
the coverage laws at design fractions μ_design > 0.2, with splits per
each model's documented rule. Costs are closed forms, and the
simulator agrees within 0.05 %.

Table A grids the δ-cheapest point per μ_design. Table B prices each
model's grid points against its §1 selection. Table C prices the notch
from the δ-cheapest μ = 0.2 points ((12, 8), RF = 8, (9, 8),
F/RF = 24). Table C is the analysis behind §1's margin rule. For M1,
M2 and M4, the §1 selections coincide with the 0.225-grid points, and
M4's also coincides with the 0.250 one. Thus their margins come from
parameters that the grid already prices.

**A — cheapest point per μ_design** (standing links as mean / max
observed, worst of 25 graphs):

| μ_design | model | params | msgs / message | copies / honest | links mean / max | P(bad) law |
|---|---|---|---|---|---|---|
| 0.225 | **M3** | (13, 8) | **156 166** | **10.1** | 40 / 38 | 7.7×10⁻⁵ |
| | M4 | RF = 9 | 200 723 | 13.0 | **18** / 34 | 2.1×10⁻⁵ |
| | M5 | (9, 9) | 216 222 | 14.0 | 36 / 34 | 4.3×10⁻⁵ |
| | M1 | F = 25 | 300 308 | 19.4 | 50 / 42 | 5.9×10⁻⁵ |
| | M2 | RF = 25 | 300 308 | 19.4 | 50 / 42 | 6.0×10⁻⁵ |
| 0.250 | **M3** | (14, 8) | **157 503** | **10.5** | 42 / 37 | 8.0×10⁻⁵ |
| | M4 | RF = 9 | 187 498 | 12.5 | **18** / 31 | 6.7×10⁻⁵ |
| | M5 | (10, 9) | 213 746 | 14.2 | 38 / 31 | 4.8×10⁻⁵ |
| | M1 | F = 26 | 292 495 | 19.5 | 52 / 44 | 5.0×10⁻⁵ |
| | M2 | RF = 26 | 292 495 | 19.5 | 52 / 44 | 5.1×10⁻⁵ |
| 0.300 | **M3** | (17, 7) | **166 601** | **11.9** | 46 / 36 | 8.7×10⁻⁵ |
| | M4 | RF = 10 | 181 997 | 13.0 | **20** / 32 | 7.5×10⁻⁵ |
| | M5 | (11, 10) | 205 796 | 14.7 | 42 / 33 | 6.0×10⁻⁵ |
| | M1 | F = 27 | 264 594 | 18.9 | 54 / 42 | 8.6×10⁻⁵ |
| | M2 | RF = 27 | 264 594 | 18.9 | 54 / 42 | 8.7×10⁻⁵ |
| 0.350 | **M3** | (19, 8) | **160 550** | **12.4** | 52 / 41 | 6.4×10⁻⁵ |
| | M4 | RF = 11 * | 172 896 | 13.3 | **22** / 30 | 9.8×10⁻⁵ |
| | M5 | (12, 11) | 194 345 | 15.0 | 46 / 32 | 8.5×10⁻⁵ |
| | M1 | F = 29 | 245 043 | 18.9 | 58 / 44 | 8.4×10⁻⁵ |
| | M2 | RF = 29 | 245 043 | 18.9 | 58 / 44 | 8.5×10⁻⁵ |

\* RF = 11 sits on the law crossing. The carried ~1.1× tail correction
pushes it just over δ, and a direct high-μ tail check (×1.04 ± 0.07)
cannot decide it either way. Thus the safe choice is RF = 12
(189 796 msgs, 14.6 copies / honest). A dedicated measurement
([`tail-correction.md`](../../../pubsub-node/docs/experiments/tail-correction.md),
370 k draws across both designs) later read the factor at
0.994 ± 0.021 — no correction at the measured cells. Thus RF = 11
stands on the measured basis. RF = 12 stays until the pass that
retires the correction. M3's corrected values stay under δ everywhere.

Latency (not tabulated) moves in the opposite direction. M3's
full-coverage depth decreases from 5.9 to 5.0 hops across the grid,
because a larger RF makes the trees shallower. M2's decreases from 4.9
to 4.7, and the rest stay at ≈ 5 throughout. Thus §2's latency spread
becomes narrower under re-provisioning.

**B — premium over each model's §1 selection** (Δmsgs / Δmean links):

| model | 0.225 | 0.250 | 0.300 | 0.350 |
|---|---|---|---|---|
| M3 | −6 % / +5 % | −5 % / +11 % | ±0 % / +21 % | −4 % / +37 % |
| M4 | −6 % / ±0 | −13 % / ±0 | −15 % / +11 % | −19 % / +22 % * |
| M5 | −1 % / +6 % | −2 % / +12 % | −5 % / +24 % | −11 % / +35 % |
| M1 | −6 % / ±0 | −9 % / +4 % | −17 % / +8 % | −23 % / +16 % |
| M2 | −6 % / ±0 | −9 % / +4 % | −17 % / +8 % | −23 % / +16 % |

(\* −12 % / +33 % with the tail-corrected RF = 12.) Absolute bandwidth
*decreases* across the grid: H = (1−μ)N shrinks faster than the
budgets grow, and the margined baselines start one notch up. Thus
copies per honest node is the honest cost axis. **The price of
robustness is almost fully state**: +16–37 % more standing links at
0.35.

**C — the price of one notch at μ = 0.2.** Integer parameters make
robustness come in discrete steps, and this table prices the first
step. Each deployment stays designed for μ = 0.2 but takes the next
parameter increment. For M3, the increment is a re-split of the same
budget, or one added link. We then read §4's budget again: the largest
μ_eff that the hardened point tolerates before P(bad) > 10⁻⁴ (churn
reading p_max = Δμ/0.8):

| model | notch | Δmsgs | Δlinks | budget μ_eff (Δμ) | churn p_max |
|---|---|---|---|---|---|
| M3 | re-split (13, 7), B = 19 | +8.3 % | ±0 | 0.204 → **0.217** (+0.017) | 0.5 → 2.2 % |
| M3 | +1 budget (12, 9), bw-min rule | ±0 % | +2 | 0.204 → 0.207 (+0.007) | 0.5 → 0.9 % |
| M3 | +1 budget (14, 7), rb-optimal | +16.7 % | +2 | 0.204 → **0.240** (+0.040) | 0.5 → 5.0 % |
| M4 | RF = 9 | +13.6 % | +2 | 0.209 → **0.259** (+0.059) | 1.1 → 7.4 % |
| M5 | (9, 9), B = 18 | +5.9 % | +2 | 0.217 → 0.244 (+0.044) | 2.2 → 5.4 % |
| M1 | F = 25 | +4.2 % | +2 | 0.214 → 0.247 (+0.047) | 1.8 → 5.9 % |
| M2 | RF = 25 | +4.2 % | +2 | 0.214 → 0.247 (+0.047) | 1.7 → 5.8 % |

M3's two flavors are not interchangeable. The same-budget re-split
(13, 7) makes its μ-budget four times larger, for +8.3 % bandwidth and
**zero extra state**. (With the ×1.11 tail correction: 0.215 and churn
~1.9 %, against ~0.3 % for the corrected base.) One added budget under
M3's own bandwidth-minimal rule ((12, 9)) buys almost nothing. Thus M3
headroom comes from links moved into RF, not from added links. M4's
RF = 9 is the family's biggest notch, at the biggest bandwidth price.

**Frontier verdict.** **The M3-over-M4 bandwidth order survives at
every analyzed μ_design.** The lead is 22 % at 0.225 and narrows to
7–15 % at 0.35. On the stair-free fractional trend, the ratio
flattens, and parity sits at μ ≈ 0.64. M4 stays the state winner, with
2.2–2.4× fewer mean links.

At *equal robustness* the choice also holds. M3's rb-optimal +1-budget
point (14, 7) gets to a 0.240 μ-budget for ≈ 179 200 msgs. That is
16 % under M4's RF = 9, which holds the deeper 0.259 (§5C). Thus a
weight on robustness does not open the M3/M4 choice again — it changes
which *split* of M3's budget to deploy. M3's bandwidth-minimal split
is the family's most μ-brittle point at every μ_design. M1/M2 hold the
deepest collapse cushion throughout. M5 is best on no axis at any
μ_design.

## 6. Adaptive eclipse cost (corruptions to strand a victim)

The data comes from each model's
[`adaptive_eclipse_cost.md`](m3/properties/adaptive_eclipse_cost.md).
When the epoch's draws are public, an attack that strands a victim
costs the victim's honest degree on the attacked side. The two attacks
are **deafen** (cut the honest in-edges — the victim misses some
publisher) and **mute** (cut the honest out-edges — its publications
reach nobody). Coverage fails in both cases. Min-cut equals degree
here: at branching factors of ~10–20, the depth-2 shell is much larger
than the depth-1 shell. Thus Menger's disjoint-path count saturates at
the degree.

How to read the table: each value is a number of corruptions, and a
larger value is better for the network. The deafen and mute columns
give the mean cost of each attack. Column A gives the cost to strand
one chosen victim. Column B gives the minimum cost across all 16 000
honest nodes, for an adversary that accepts any victim. Bold marks the
best value in each threat column. The cheapest targets are M3 at 10.4
on threat A and M5 at 3.7 on threat B. **M1 and M2 win this section on
both threats (20.0 and 5.0):**

| model | parameters | deafen | mute | **A: chosen victim** | **B: any victim** | B via |
|---|---|---|---|---|---|---|
| M3 | (13, 7) | 10.4 | 15.2 | 10.4 | 3.8 | deafen |
| M4 | RF = 9 | 14.4 | 14.4 | 14.4 | 4.5 | either |
| M5 | (9, 8) | 13.6 | 13.6 | 13.6 | 3.7 | joint |
| M1 | F = 25 | 20.0 | 20.0 | **20.0** | **5.0** | deafen |
| M2 | RF = 25 | 20.0 | 20.0 | **20.0** | **5.0** | mute |

**The two threat models rank the family differently, and the gap is
2.7–4×.** A *chosen* victim pays its own draw. Thus M3 is the cheapest
target, and M1/M2 are the most expensive. This is the reading that
"partially reverses the frontier", because the bandwidth winner is the
cheapest target. But an adversary that accepts *any* victim searches
the lower tail across 16 000 nodes and pays the network minimum. That
minimum is 2.7–4.0× below the mean in every model, and in that reading
M5 is cheaper than M3 (3.7 against 3.8).

Against an adversarial budget of μN = 4 000, **no model costs more
than ~5 corruptions to break the δ guarantee somewhere.** Eclipse cost
is a degree value, thus it also prices provisioning: the δ-cheapest
points sit 0.8–1.6 corruptions cheaper on threat A ((12, 8) at 9.6,
RF = 8 at 12.8, F/RF = 24 at 19.2).

**Chosen links beat accepted links at equal mean.** M1 and M2 have
identical mean degree (20.0) on both sides and identical bandwidth.
But their *directions* differ by 2.2×. A node **chooses** its own
picks and always holds exactly F of them. Thus only adversarial
thinning applies, and the law is binomially concentrated (sd 2.00).
The node does **not** choose who picks *it*. Thus the accepted side is
a balls-in-bins draw with a Poisson lower tail (sd 4.47).

As a result, M1 is cheap to deafen (the accepted in-side) and costly
to mute. M2 is the exact mirror. The two models end at the same
guarantee-breaking cost of 5.0. Thus, on this axis, **M2 does not
dominate M1**: it moves the weakness from the receiving side to the
publishing side.

This also splits two weaknesses by cause. **M3's problem is level**:
its in-degree is chosen and tightly concentrated (sd 1.44), but low at
10.4. Thus more RF is the fix. **M1's problem is spread**: a high mean
with a fat accepted tail. Thus the fix is to convert accepted in-links
to chosen ones. The same symptom has different remedies.

**Correction.**
[`candidate_properties.md`](candidate_properties.md) gave an earlier
estimate for this property: "M3 9.6, M5 13.6, M1/M2 19.2, M4 25.6".
That estimate put M4 at the safe end. The other four figures
transcribe measured means from each model's `node_degrees.md`. M4's
figure does not: that file showed 12.80 from the first publication of
the models. 25.6 is exactly two times 12.80, that is 4·RF(1−μ),
consistent with a second application of the 2× in the closed form. The
figure appears in no script or table anywhere in the repository.

Read as the honest degree, which is what the property costs, 12.80 is
the defensible value. With the order statistic then applied, M4 moves
from most eclipse-resistant to second cheapest among the δ-cheapest
points. At the §1 selections, its RF = 9 sits mid-field (see the table
above).

## 7. Transmission unreliability — loss tolerance and the price of repair

This section asks what send loss does to each model frozen at its §1
operating point. Every honest→honest send is dropped iid with
probability p_fail. The per-model analyses are in each
[`transmission_unreliability.md`](m3/properties/transmission_unreliability.md).
With r per-link retries, the per-send failure is p_fail^(r+1). A
guarantee over *every message of an epoch* cannot survive per-message
randomness. Thus we read the guarantee again **per message**:
ε_msg = P(one given message misses ≥ 1 honest node), held to the same
δ.

The law is §4's μ-shift curve at μ_eff = μ + (1−μ)p_fail, because a
lost send silences an edge like an adversarial pick. Two per-message
corrections apply. First, H does not shrink: a node behind lossy links
still needs the message. Second, the muted-publisher term loses its
factor H. The per-epoch law charges a publisher with no honest
out-path (§6's mute) one time for each of the H publishers. A message
has one publisher.

MC runs validate all the laws (each script's `--mc`, seed 20260813).
The p_fail = 0 cells reproduce the exact per-graph computation on
every anchor graph. The degree distributions match their predicted
pmfs class by class (worst single-class |z| = 2.4 across the family).
The loss-injected cells at elevated p_fail agree within |z| ≤ 2.1.

**Loss tolerance at the operating points.** The table gives the
largest p_fail that each model absorbs and keeps ε_msg ≤ δ: without
repair, and with one retry per link. It also gives the law read at a
realistic 1 % loss. The churn-identity column repeats §4's p_max. The
μ_eff identity makes that value each model's *per-epoch* loss
tolerance — the baseline that the per-message reading relaxes.

How to read the table: in the churn-identity, per-message, and
with-1-retry columns, a larger value is better (the model absorbs more
loss). In the ε_msg column, a smaller value is better (fewer message
failures). The rows are sorted from the best to the worst per-message
tolerance, and bold marks the best value in each column. **M2 wins
this section**: it is best per message, best with one retry, and best
in ε_msg. M4 is best in the per-epoch churn identity. M3, the
bandwidth winner of §2, is the weakest model here, but every model
clears the realistic 1 % bar:

| model | params | churn identity (§4) | per-message | with 1 retry | ε_msg at 1 % loss |
|---|---|---|---|---|---|
| **M2** | RF = 25 | ~5.8 % | **33.7 %** | **58 %** | **2.5×10⁻⁹** |
| M4 | RF = 9 | **~7.4 %** | 7.2 % | 26.9 % | 9.3×10⁻⁶ |
| M1 | F = 25 | ~5.9 % | 5.6 % | 23.7 % | 4.0×10⁻⁵ |
| M5 | (9, 8) | ~2.2 % | 5.1 % | 22.6 % | 2.1×10⁻⁵ |
| M3 | (13, 7) | ~2.2 % | 4.25 % | 20.6 % | 2.2×10⁻⁵ |

**Every model clears WAN-realistic loss without repair.** At 1 % iid
loss, the worst ε_msg in the family is M1's 4.0×10⁻⁵, 2.5× under δ.
The per-message tolerances run 4–34 %. The structure of the reading:
models whose binding per-epoch term is a publisher-side event (§6's
mute) keep the benefit of the removed H-factor and grow above their
churn identity. M3 grows ×2.0, M5 ×2.3, and M2 ×5.8. (M2's
requester-less-publisher defect is a single ~2×10⁻⁹ event per message.
Thus its 25-pull reception — 20.0 honest tries per node, the family's
deepest — is the binding term.) M1 and M4 are bound on the receiving
side (§6's deafen) and lose a few percent (×0.95 and ×0.97: H does not
shrink).

**Bidirectionality is a degree effect, not extra loss protection.** An
uninformed node never fires. Thus the reverse direction of an
undirected link is not a second chance at the final hop. The μ_eff
identity already prices M4's 14.4 honest tries from a 9-link budget.
That count is the family's best *per held link*, and it is the source
of M4's 7.2 % budget, the family's second-deepest. The genuine
interior effect (B→A succeeds after a failed A→B) is measurable only
in the bulk regime and negligible at the δ tail. The δ-cheapest RF = 8
cells show a small negative interior residual. This residual does not
appear again at RF = 9, and
[`transmission_unreliability.md`](m4/properties/transmission_unreliability.md)
records it as unresolved.

**Retries are a per-epoch instrument, not a per-message one.** ε_msg
has a floor at p_fail = 0: the graph draw, shared by all the epoch's
messages. Thus the epoch reading is
P(bad epoch) ≤ P(structural defect) + R·[ε_msg(p) − ε_msg(0)] for R
messages per epoch, not ε_msg ≤ δ/R. At R = 10³, the no-retry
guarantee survives only for M2 (to ~16 % loss). M1/M3/M4/M5 need one
per-link retry, which holds them to ~0.9–1.9 % loss. The bandwidth
price is ×(1−p^{r+1})/(1−p) ≈ ×(1+p_fail). The latency price is
≈ p_fail timeouts per delivered send, and the hop depth almost does
not move. Correlated or bursty loss is out of scope, because a failing
peer's whole link set reads as churn (§4). Adversaries that withhold
acks inflate the retry bandwidth, not the coverage.

## 8. Bottom line

At P(bad) ≤ 10⁻⁴ held with the 2 % disturbance margin, N = 20 000,
μ = 0.2: **M3 (RF = 13, s = 7) is the most efficient model in
bandwidth.** It is 22 % below M4 and ~48 % below M1/M2. It is within
~0.9 hop (~0.1–0.3 s) of the fastest model. Churn tests validate it at
exactly these parameters, and it holds 1 % send loss with ~4.5×
headroom (§7).

**M4 (RF = 9) is the most efficient model in per-node state.** It
holds 18 standing links, 2.1× fewer than M3, with a single mechanism
and one link type. The price is ~29 % more bandwidth at near-identical
latency. M4 also beats M5 on every measured axis, with no trade: cost
(214 433 against 217 562), state (18 against 34), margin (7.4 %
against 2.2 %), collapse (0.55 against 0.49), eclipse (14.4 / 4.5
against 13.6 / 3.7), and loss (7.2 % against 5.1 %).

The practical choice is M3 if bandwidth is the binding resource, and
M4 if connection count or simplicity is. Section 5 shows that this
choice is stable under re-provisioning: M3 keeps the bandwidth lead at
every analyzed μ_design ≤ 0.35. Section 7 shows that the choice holds
under loss without transport repair at WAN-realistic 0.1–1 %. The two
leaders are also the two cheapest chosen-victim targets (§6): 10.4
(M3) and 14.4 (M4) corruptions, against 20.0 for M1/M2. Thus a design
that weights adversarial cost together with efficiency opens the
choice again.

Of the rest, M1 and M2 tie on cost and state: ≈ 320 000 msgs and
50 links, ~1.9× the leaders' bandwidth and 2.8× M4's state. They also
tie on the collapse cushion (≈ 0.62, the family's deepest). M2 alone
is the fastest (4.6 hops). M2 also carries the family's one absolute
crown under loss: ~34 % per-message tolerance, and it is the only
design whose epoch-level guarantee at R = 10³ survives realistic loss
without transport repair (§7). But §6 prices the eclipse of M1 and M2
equally at 5.0 and only moves the weak side: the adversary deafens M1
and mutes M2. Thus M2's wins come at no extra cost, but M2 does not
strictly dominate M1. M5 is best on no measured axis.
