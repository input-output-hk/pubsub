# Model comparison — bandwidth and latency at P(bad) ≤ 10⁻⁴ with a 2 % disturbance margin

Comparison of the five dissemination models at the shared operating point:

- **N = 20 000, μ = 0.2** (H = 16 000 honest, 4 000 silent adversaries);
- **full-coverage criterion** ([README](README.md)): a sampled graph is good
  iff every message of every honest publisher reaches all other honest nodes —
  only standing per-epoch structure counts;
- **P(bad graph) ≤ 10⁻⁴ per epoch, held with a disturbance margin**: a
  configuration is *admissible* iff its coverage law keeps P(bad) ≤ 10⁻⁴
  over the whole interval μ_eff ≤ μ + Δμ_margin, with
  **Δμ_margin = 0.016** — churn/loss reading p_max = Δμ/(1−μ) ≥ 2 %
  (§4). The margin is read off the law; for M3, M4 and M5 the law is
  churn-validated at exactly the selected parameters
  ([churn-proposed-points](../../../pubsub-node/docs/experiments/churn-proposed-points.md),
  [churn-tolerance](../../../pubsub-node/docs/experiments/churn-tolerance.md));
- **selection is by dominance, not objective**: each model appears at
  its admissible configurations that are Pareto-minimal over
  (msgs / message, mean standing links) — a point is discarded only if
  another admissible point of the same model is at least as good on
  both axes and strictly better on one. Latency is reported but does
  not prune (the field spans under a hop, §2); robustness beyond the
  margin is reported (§4) but does not prune (as an axis it would keep
  every larger parameter alive). At the current numbers every model's
  Pareto set is a single point;
- counting conventions as everywhere in this folder: fire-once relaying, no
  resend on the arrival link, transmissions = honest→honest copies
  (duplicates included).

All values below summarise each model's `properties/` files, measured
at the selected operating points by the model's `scripts/` (25–400
graphs per cell, seeds recorded per file); the coverage laws behind
the parameter choices are Monte-Carlo validated in each model's
[`full_coverage.md`](m1/properties/full_coverage.md). The M3 and M4
points are independently cross-checked by the pubsub-node instrument
([m3-comparison §5](../../../pubsub-node/docs/experiments/m3-comparison.md)
/ [m4-comparison §5](../../../pubsub-node/docs/experiments/m4-comparison.md),
200 graphs each) — agreement within 0.05 % on every shared quantity.

## 1. Selected configurations per model

| model | mechanism | selected parameters | why these (dominance) | P(bad) | margin p_max |
|---|---|---|---|---|---|
| [M1](m1/properties/README.md) | push | F = 25 | smallest admissible F (F = 24: p_max ~1.8 %) | 3.3×10⁻⁵ | ~5.9 % |
| [M2](m2/properties/README.md) | pull | RF = 25 | smallest admissible RF (RF = 24: ~1.7 %) | 3.3×10⁻⁵ | ~5.8 % |
| [M3](m3/properties/README.md) | pull + initiation links | RF = 13, s = 7 | every split of budget RF+(s−1) = 19 holds the same 38 links, so dominance reduces to the cheapest admissible split — (12, 8) is inadmissible (~0.5 %); budgets ≥ 20 are dominated outright | 4.4×10⁻⁵ | ~2.2 % |
| [M4](m4/properties/README.md) | undirected flood | RF = 9 | smallest admissible RF (RF = 8: ~1.1 %) | 6.1×10⁻⁶ | ~7.4 % |
| [M5](m5/properties/README.md) | directed k_in/k_out | (k_in, k_out) = (9, 8) | most-balanced split of the smallest budget k_in+k_out = 17 — the one model whose δ-cheapest point already clears the margin | 4.4×10⁻⁵ | ~2.2 % |

**Why a margin.** The natural criterion — each model at its cheapest
parameters meeting δ alone (its **δ-cheapest** point, a term used
throughout) — selects, by construction, the point sitting nearest the
failure cliff, so comparisons made there measure the selection rule
rather than the mechanisms: M3's δ-cheapest split (12, 8) holds ~0.5 %
churn margin while the same-budget re-split (13, 7) holds ~2.2 % at
identical state (§5C). The margin is imposed in μ_eff rather than as a
factor on δ for two reasons. A δ-factor k buys μ-headroom of only
ln k / sensitivity, so the steepest laws — d ln E/dμ ≈ 60 at M3's
δ-cheapest split, ≈ 50 at M5's — would receive the least real
protection exactly where the cliff is sharpest. And Δμ is the operational currency: one number that
simultaneously bounds honest churn, an underestimate of μ itself, and
per-epoch send loss (the μ_eff identity, §§4, 7).

**The bar.** Δμ_margin = 0.016 (p_max 2 %) is anchored to the
disturbances this folder already treats as realistic: WAN send loss of
~1 % (§7) plus same-order honest churn. It is not tuned to the
selections: any bar in (1.1 %, 1.7 %] would admit M1/M2's δ-cheapest
F/RF = 24 (p_max ~1.7–1.8 %) while excluding (12, 8) and RF = 8 —
rejected, because a bar placed to spare rows chooses the criterion to
fit the conclusion. One caveat travels with M3's selection: pooled across
all churn rounds its law reads slightly optimistic
([churn-proposed-points §2](../../../pubsub-node/docs/experiments/churn-proposed-points.md),
Stouffer +2.41, mechanism unidentified;
[finite-n §6](../../../pubsub-node/docs/experiments/finite-n.md)
agrees along an independent axis), and the direction is conservative —
it can only shrink M3's 2.17 % toward the bar. If the deviation
resolves to a real correction, M3's admissibility at (13, 7) is the
first thing to re-read.

**Provenance.** Every cell below traces to the owning model's
`properties/` file, measured at the selected parameters. Two
independent validation layers back the laws: the formal folder's own
MC (elevated-μ_eff spot cells in each `mu_shift_robustness.md`,
loss-injected cells in each `transmission_unreliability.md`), and, for
M3, M4 and M5, the pubsub-node instrument's churn cells at exactly the
selected parameters — 6/6 inside the Wilson intervals at
μ_eff = 0.36–0.48
([churn-proposed-points](../../../pubsub-node/docs/experiments/churn-proposed-points.md);
M5 in the earlier round of
[churn-tolerance](../../../pubsub-node/docs/experiments/churn-tolerance.md)).

## 2. The comparison

| model | parameters | msgs / message | copies / honest node | hops (full) | hops (mean) |
|---|---|---|---|---|---|
| **M3** | RF = 13, s = 7 | **166 428** | **10.4** | 5.5 | 4.2 |
| M4 | RF = 9 | 214 433 | 13.4 | 5.0 | 3.9 |
| M5 | (9, 8) | 217 562 | 13.6 | 5.0 | 3.9 |
| M1 | F = 25 | 319 974 | 20.0 | 4.9 | **3.6** |
| M2 | RF = 25 | 319 992 | 20.0 | **4.6** | **3.6** |

**Bandwidth: M3 wins decisively** — 22 % below M4, 24 % below M5,
~48 % below M1/M2; M4's lead over M5 is a slim ~1.5 %. **Latency: M2
wins, marginally** — the whole field spans ~0.9 hops (4.6–5.5 full
coverage; ~0.1–0.3 s at WAN per-hop times of 100–300 ms) while
bandwidth spans ~1.9×.

## 3. Node degrees (standing links per node)

From each model's [`node_degrees.md`](m3/properties/node_degrees.md): the
mean total degree under protocol-compliant link opening is exactly 2× the
nominal budget (every link has a chooser and an acceptor); the maximum is a
balls-in-bins tail over the accepted side (measured, 25 graphs):

| model | chosen (held, det.) | honest in / out (mean) | max observed | compliant total (mean) |
|---|---|---|---|---|
| **M4** | 9 | 14.4 / 14.4 (same links) | 34 | **18** |
| M5 | 9 in + 8 out | 13.6 / 13.6 | 33 | 34 |
| M3 | 13 in + 6 out | 15.2 / 15.2 | 38 accepted | 38 |
| M1 | 25 out | 20.0 / 20.0 | 45 | 50 |
| M2 | 25 in | 20.0 / 20.0 | 45 | 50 |

**On this axis the ordering flips: M4 wins decisively** — 18 links per
node vs M3's 38 (2.1×) and M1/M2's 50 (2.8×), with M4/M5 also holding
the smallest worst-case nodes (33–34 vs 38–45). M3's standing links
are a budget read, not a split read: every split of budget 19 holds
the same 38, measured identical for (12, 8) and (13, 7)
([node_degrees.md](m3/properties/node_degrees.md)). Note M3's degree
exceeds its bandwidth: 12 of its 38 links (the initiation kind) carry
only their owner's publications — cheap in traffic, but still held
state, connection slots, and churn surface. In M4 and M5 every held
link also carries relay traffic.

## 4. Degradation under μ-shift (frozen parameters)

From each model's
[`mu_shift_robustness.md`](m3/properties/mu_shift_robustness.md): the
operating points frozen, the effective adversarial fraction swept upward
(law-read, MC-validated at elevated μ). Reported: the **budget** (largest
μ_eff keeping P(bad) ≤ 10⁻⁴; churn reading p_max = Δμ/(1−μ)) and the
**collapse point** (P(bad) = ½):

| model | parameters | budget μ_eff (Δμ) | churn p_max | collapse μ_eff |
|---|---|---|---|---|
| M4 | RF = 9 | 0.259 (+0.059) | ~7.4 % | 0.55 |
| M1 | F = 25 | 0.247 (+0.047) | ~5.9 % | **0.62** |
| M2 | RF = 25 | 0.247 (+0.047) | ~5.8 % | **0.62** |
| M5 | (9, 8) | 0.217 (+0.017) | ~2.2 % | 0.49 |
| **M3** | (13, 7) | 0.217 (+0.017) | ~2.2 % | 0.47 |

**Every selected point clears p_max ≥ 2 % by construction**, and the
spread above the bar is itself informative: M4 holds ~3.7× the
required margin, M1/M2 ~2.9×, M3/M5 sit at the bar. Read instead at
the δ-cheapest points, the robustness ordering is roughly the
bandwidth ordering reversed — M3's δ-cheapest split (12, 8) holds
~0.5 %, M4's RF = 8 ~1.1 % — which is what disqualifies cheapest-point
selection: sitting nearest the cliff is a property of that rule, not
of a mechanism, and §5C's re-split dissolves M3's apparent brittleness
at zero state cost. What is genuine structure here: **sensitivity** —
d ln E/dμ ≈ 50 for M5 and ≈ 48 for M3 (its μ^RF in-term alone runs at
RF/μ = 65; the 29 : 71 in : out defect split tempers the mixture)
against ≈ 25 for M1/M2's exponential terms — the two bar-sitting
models also erode fastest per unit of unmodelled shift; and the
**collapse cushion** — M1/M2 collapse at μ_eff ≈ 0.62, jointly the
latest in the family vs 0.47–0.55 for the rest, a cushion bought by
their ~1.9× bandwidth. The budgets are law-reads: M3/M4's laws are
churn-validated at exactly these parameters (6/6 cells out to
μ_eff = 0.48), M1/M2's by the formal elevated-μ MC in each
`mu_shift_robustness.md`; the budgets themselves sit at
P(bad) = 10⁻⁴ and are not directly sampleable at feasible run counts
([churn-proposed-points §2](../../../pubsub-node/docs/experiments/churn-proposed-points.md)).

## 5. Re-provisioning — the robustness-adjusted frontier

Where §4 asks how a fixed deployment degrades as μ_eff rises, this
section asks the converse: what it costs to *provision for* a higher μ
up front. From each model's
[`re_provisioning.md`](m3/properties/re_provisioning.md): the coverage
laws inverted at design fractions μ_design > 0.2 (splits per each
model's documented rule; costs are closed forms, simulator-checked
within 0.05 %). Table A grids the δ-cheapest point per μ_design;
table B prices each model's grid points against its §1 selection;
table C prices the notch from the δ-cheapest μ = 0.2 points ((12, 8),
RF = 8, (9, 8), F/RF = 24) — the analysis behind §1's margin rule.
For M1, M2 and M4 the §1 selections coincide with the 0.225-grid
points (M4's with the 0.250 one as well): their margins come from
parameters the grid already prices.

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

\* RF = 11 sits on the law crossing; the carried ~1.1× tail correction
pushes it just over δ, and a direct high-μ tail check (×1.04 ± 0.07)
cannot clear it either way — the safe choice is RF = 12 (189 796 msgs,
14.6 copies / honest). The dedicated measurement
([`tail-correction.md`](../../../pubsub-node/docs/experiments/tail-correction.md),
370 k draws across both designs) has since read the factor at
0.994 ± 0.021 — no correction at the measured cells — so RF = 11 stands
on the measured basis; RF = 12 is retained pending the pass that
retires the correction. M3's corrected values stay under δ everywhere.

Latency (not tabulated) moves the other way: M3's full-coverage depth
falls from 5.9 to 5.0 hops across the grid (larger RF ⇒ shallower
trees), M2's from 4.9 to 4.7, the rest ≈ 5 throughout — §2's latency
spread narrows under re-provisioning.

**B — premium over each model's §1 selection** (Δmsgs / Δmean links):

| model | 0.225 | 0.250 | 0.300 | 0.350 |
|---|---|---|---|---|
| M3 | −6 % / +5 % | −5 % / +11 % | ±0 % / +21 % | −4 % / +37 % |
| M4 | −6 % / ±0 | −13 % / ±0 | −15 % / +11 % | −19 % / +22 % * |
| M5 | −1 % / +6 % | −2 % / +12 % | −5 % / +24 % | −11 % / +35 % |
| M1 | −6 % / ±0 | −9 % / +4 % | −17 % / +8 % | −23 % / +16 % |
| M2 | −6 % / ±0 | −9 % / +4 % | −17 % / +8 % | −23 % / +16 % |

(\* −12 % / +33 % with the tail-corrected RF = 12.) Absolute bandwidth
*falls* across the grid — H = (1−μ)N shrinks faster than the budgets
grow, and the margined baselines start a notch up — so copies per
honest node is the honest cost axis. **Robustness is bought almost
entirely in state**: +16–37 % more standing links at 0.35.

**C — the price of one notch at μ = 0.2**. Integer parameters mean
robustness comes in discrete steps; this table prices the first one.
Each deployment stays designed for μ = 0.2 but takes the next parameter
increment (for M3: either re-splitting the same budget or adding one
link), and §4's budget is re-read: the largest μ_eff the hardened point
tolerates before P(bad) > 10⁻⁴ (churn reading p_max = Δμ/0.8):

| model | notch | Δmsgs | Δlinks | budget μ_eff (Δμ) | churn p_max |
|---|---|---|---|---|---|
| M3 | re-split (13, 7), B = 19 | +8.3 % | ±0 | 0.204 → **0.217** (+0.017) | 0.5 → 2.2 % |
| M3 | +1 budget (12, 9), bw-min rule | ±0 % | +2 | 0.204 → 0.207 (+0.007) | 0.5 → 0.9 % |
| M3 | +1 budget (14, 7), rb-optimal | +16.7 % | +2 | 0.204 → **0.240** (+0.040) | 0.5 → 5.0 % |
| M4 | RF = 9 | +13.6 % | +2 | 0.209 → **0.259** (+0.059) | 1.1 → 7.4 % |
| M5 | (9, 9), B = 18 | +5.9 % | +2 | 0.217 → 0.244 (+0.044) | 2.2 → 5.4 % |
| M1 | F = 25 | +4.2 % | +2 | 0.214 → 0.247 (+0.047) | 1.8 → 5.9 % |
| M2 | RF = 25 | +4.2 % | +2 | 0.214 → 0.247 (+0.047) | 1.7 → 5.8 % |

M3's two flavours are not interchangeable: the same-budget re-split
(13, 7) quadruples its μ-budget for +8.3 % bandwidth and **zero extra
state** (with the ×1.11 tail correction: 0.215, churn ~1.9 %, vs the
corrected base's ~0.3 %), while +1 budget under its own
bandwidth-minimal rule ((12, 9)) buys almost nothing — M3 headroom
comes from moving links into RF, not from adding links. M4's RF = 9 is the family's biggest notch, at the
biggest bandwidth price.

**Frontier verdict.** **The M3-over-M4 bandwidth ordering survives at
every analysed μ_design** (lead 22 % at 0.225, narrowing to 7–15 % at
0.35; on the stair-free fractional trend the ratio flattens and parity
sits at μ ≈ 0.64), and M4 stays state winner (2.2–2.4× fewer mean
links). At
*equal robustness* the choice also holds: M3's rb-optimal +1-budget
point (14, 7) reaches a 0.240 μ-budget for ≈ 179 200 msgs — 16 % under
M4's RF = 9, which holds the deeper 0.259 (§5C). Weighting robustness
does not reopen the M3/M4 choice — it changes
which *split* of M3's budget to deploy. M3's bandwidth-minimal split
is the family's most μ-brittle point at every μ_design; M1/M2 hold
the deepest collapse cushion throughout; M5 is best on no axis at any
μ_design.

## 6. Adaptive eclipse cost (corruptions to strand a victim)

From each model's
[`adaptive_eclipse_cost.md`](m3/properties/adaptive_eclipse_cost.md): once the
epoch's draws are public, stranding a victim costs its honest degree on the
attacked side — **deafen** (cut honest in-edges, the victim misses some
publisher) or **mute** (cut honest out-edges, its publications reach nobody).
Coverage fails either way. Min-cut equals degree here: at branching factors
of ~10–20 the depth-2 shell dwarfs the depth-1 shell, so Menger's
disjoint-path count saturates at the degree.

| model | parameters | deafen | mute | **A: chosen victim** | **B: any victim** | B via |
|---|---|---|---|---|---|---|
| **M3** | (13, 7) | 10.4 | 15.2 | **10.4** | 3.8 | deafen |
| M4 | RF = 9 | 14.4 | 14.4 | 14.4 | 4.5 | either |
| M5 | (9, 8) | 13.6 | 13.6 | 13.6 | **3.7** | joint |
| M1 | F = 25 | 20.0 | 20.0 | **20.0** | 5.0 | deafen |
| M2 | RF = 25 | 20.0 | 20.0 | **20.0** | 5.0 | mute |

**The two threat models rank the family differently, and the gap is
2.7–4×.** A *chosen* victim pays its own draw, so M3 is the cheapest
target and M1/M2 the dearest — the reading that "partially reverses
the frontier", since the bandwidth winner is the cheapest target. But
an adversary content with *any* victim shops the lower tail across
16 000 nodes and pays the network minimum, which is 2.7–4.0× below the
mean in every model — and on that reading M5 edges out M3 as the
cheapest (3.7 vs 3.8). Against an adversarial budget of μN = 4 000,
**no model costs more than ~5 corruptions to break the δ guarantee
somewhere.** Eclipse cost is a degree read, so it prices provisioning
too: the δ-cheapest points sit 0.8–1.6 corruptions cheaper on threat A
((12, 8): 9.6; RF = 8: 12.8; F/RF = 24: 19.2).

**Chosen links beat accepted links at equal mean.** M1 and M2 have identical
mean degree (20.0) on both sides and identical bandwidth, yet their
*directions* differ by 2.2×: a node **chooses** its own picks and always
holds exactly F of them, so only adversarial thinning applies and the law is
binomially concentrated (sd 2.00); it does **not** choose who picks *it*, so
the accepted side is a balls-in-bins draw with a Poisson lower tail (sd
4.47). M1 is therefore cheap to deafen (accepted in-side) and dear to mute;
M2 is the exact mirror. Both end at the same guarantee-breaking cost of 5.0,
so on this axis **M2 does not dominate M1** — it relocates the weakness from
the receiving side to the publishing side.

This also splits two weaknesses by cause: **M3's problem is level**
(its in-degree is chosen and tightly concentrated at sd 1.44, simply
low at 10.4, so more RF is the fix), while **M1's is spread** (high
mean, fat accepted tail, so converting accepted in-links to chosen
ones is the fix). Different remedies for the same symptom.

**Correction.** [`candidate_properties.md`](candidate_properties.md)
previously estimated this property at "M3 9.6, M5 13.6, M1/M2 19.2, M4 25.6"
and placed M4 at the safe end. The other four figures transcribe measured
means from each model's `node_degrees.md`; M4's does not — that file has
measured 12.80 since the models were first published, and 25.6 is exactly
twice it, i.e. 4·RF(1−μ), consistent with the 2× in the closed form being
applied a second time. The figure appears in no script or table anywhere in
the repository. Read as the honest degree, which is what the property costs,
12.80 is the defensible value; with the order statistic then applied, M4
moves from most eclipse-resistant to second cheapest among the
δ-cheapest points (at the §1 selections its RF = 9 sits mid-field,
table above).

## 7. Transmission unreliability — loss tolerance and the price of repair

This section asks what send loss does to each model frozen at its §1
operating point: every honest→honest send is dropped iid with
probability p_fail. Per-model analyses are in each
[`transmission_unreliability.md`](m3/properties/transmission_unreliability.md);
r per-link retries make the per-send failure p_fail^(r+1). A guarantee
over *every message of an epoch* cannot survive per-message randomness,
so it is re-read **per message**: ε_msg = P(one given message misses
≥ 1 honest node), held to the same δ. The law is §4's μ-shift curve at
μ_eff = μ + (1−μ)p_fail (a lost send silences an edge like an
adversarial pick), with two per-message corrections. First, H does not
shrink: a node behind lossy links still needs the message. Second, the
muted-publisher term loses its factor H: the per-epoch law charges a
publisher with no honest out-path (§6's mute) once for each of the H
publishers; a message has one.
All laws are MC-validated (each script's `--mc`, seed 20260813):
p_fail = 0 cells reproduce the exact per-graph computation on every
anchor graph, degree distributions match their predicted pmfs class by
class (worst single-class |z| = 2.4 across the family), and
loss-injected cells at elevated p_fail agree within |z| ≤ 2.1.

**Loss tolerance at the operating points** — the largest p_fail each
model absorbs while keeping ε_msg ≤ δ, without repair and with one
retry per link, and the law read at a realistic 1 % loss. The
churn-identity column repeats §4's p_max, which the μ_eff identity makes
each model's *per-epoch* loss tolerance, the baseline the per-message
reading relaxes:

| model | params | churn identity (§4) | per-message | with 1 retry | ε_msg at 1 % loss |
|---|---|---|---|---|---|
| M2 | RF = 25 | ~5.8 % | **33.7 %** | 58 % | **2.5×10⁻⁹** |
| M4 | RF = 9 | ~7.4 % | 7.2 % | 26.9 % | 9.3×10⁻⁶ |
| M1 | F = 25 | ~5.9 % | 5.6 % | 23.7 % | 4.0×10⁻⁵ |
| M5 | (9, 8) | ~2.2 % | 5.1 % | 22.6 % | 2.1×10⁻⁵ |
| **M3** | (13, 7) | ~2.2 % | 4.25 % | 20.6 % | 2.2×10⁻⁵ |

**Every model clears WAN-realistic loss without repair**: at 1 % iid
loss the worst ε_msg in the family is M1's 4.0×10⁻⁵, 2.5× under δ, and
the per-message tolerances run 4–34 %. The reading's structure: models
whose binding per-epoch term is a publisher-side event (§6's mute)
pocket the lost H-factor and grow over their churn identity — M3 ×2.0,
M5 ×2.3, M2 ×5.8 (its requester-less-publisher defect is a single
~2×10⁻⁹ event per message, leaving its 25-pull reception — 20.0 honest
tries per node, the family's deepest — as the binding term) — while M1
and M4, bound on the receiving side (§6's deafen), give a few per cent
back (×0.95 and ×0.97: H unshrunk).

**Bidirectionality is a degree effect, not extra loss protection.** An
uninformed node never fires, so the reverse direction of an undirected
link is not a second chance at the final hop. The μ_eff identity
already prices M4's 14.4 honest tries from a 9-link budget — the
family's best *per held link*, and the source of its 7.2 % budget, the
family's second-deepest. The genuine interior effect (B→A succeeding
after a failed A→B) is measurable only in the bulk regime and
negligible at the δ tail; the δ-cheapest RF = 8 cells' small negative
interior residual does not reproduce at RF = 9 and is recorded as
unresolved in
[`transmission_unreliability.md`](m4/properties/transmission_unreliability.md).

**Retries are a per-epoch instrument, not a per-message one.** ε_msg
has a floor at p_fail = 0 (the graph draw, shared by all the epoch's
messages), so the epoch reading is
P(bad epoch) ≤ P(structural defect) + R·[ε_msg(p) − ε_msg(0)] for R
messages/epoch, not ε_msg ≤ δ/R. At R = 10³ the no-retry guarantee
survives only for M2 (to ~16 % loss); M1/M3/M4/M5 need one per-link
retry, which holds them to ~0.9–1.9 % loss at a bandwidth price of
×(1−p^{r+1})/(1−p) ≈ ×(1+p_fail) and a latency price of ≈ p_fail
timeouts per delivered send (hop depth barely moves). Correlated or
bursty loss is out of scope (a failing peer's whole link set reads as
churn, §4); adversaries that withhold acks inflate retry bandwidth,
not coverage.

## 8. Bottom line

At P(bad) ≤ 10⁻⁴ held with the 2 % disturbance margin, N = 20 000,
μ = 0.2: **M3 (RF = 13, s = 7) is the most efficient model in
bandwidth** — 22 % below M4, ~48 % below M1/M2 — within ~0.9 hop
(~0.1–0.3 s) of the fastest, churn-validated at exactly these
parameters, and holding 1 % send loss with ~4.5× headroom (§7). **M4
(RF = 9) is the most efficient in per-node state** — 18 standing
links, 2.1× fewer than M3, with a single mechanism and one link type —
at ~29 % more bandwidth and near-identical latency, and it beats M5 on
every measured axis rather than trading against it: cost (214 433 vs
217 562), state (18 vs 34), margin (7.4 % vs 2.2 %), collapse (0.55 vs
0.49), eclipse (14.4 / 4.5 vs 13.6 / 3.7), loss (7.2 % vs 5.1 %). The
practical choice is M3 if bandwidth is the binding resource, M4 if
connection count / simplicity is — a choice §5 shows is stable under
re-provisioning (M3 keeps the bandwidth lead at every analysed
μ_design ≤ 0.35) and §7 shows holds under loss without transport
repair at WAN-realistic 0.1–1 %. The two leaders are also the two
cheapest chosen-victim targets (§6): 10.4 (M3) and 14.4 (M4)
corruptions vs 20.0 for M1/M2, so a design that weights adversarial
cost alongside efficiency would reopen the choice. Of the rest, M1 and
M2 tie on cost and state (≈ 320 000 msgs, 50 links, ~1.9× the leaders'
bandwidth and 2.8× M4's state) and on the collapse cushion (≈ 0.62,
the family's deepest); M2 alone is fastest (4.6 hops) and carries the
family's one absolute crown under loss — ~34 % per-message tolerance,
the only design whose epoch-level guarantee at R = 10³ survives
realistic loss without transport repair (§7) — yet §6 prices their
eclipse equally at 5.0 and only relocates the weak side (M1 is
deafened, M2 muted), so M2's wins come at no extra cost without
strictly dominating M1. M5 is best on no measured axis.
