# Model comparison — bandwidth and latency at P(bad) ≤ 10⁻⁴

Comparison of the five dissemination models at the shared operating point:

- **N = 20 000, μ = 0.2** (H = 16 000 honest, 4 000 silent adversaries);
- **full-coverage criterion** ([README](README.md)): a sampled graph is good
  iff every message of every honest publisher reaches all other honest nodes —
  only standing per-epoch structure counts;
- **P(bad graph) ≤ 10⁻⁴ per epoch**, each model at its cheapest parameters
  meeting the target;
- counting conventions as everywhere in this folder: fire-once relaying, no
  resend on the arrival link, transmissions = honest→honest copies
  (duplicates included).

All values below are **measured** (40–200 graphs per cell) by each model's
`scripts/`; the coverage laws behind the parameter choices are Monte-Carlo
validated in each model's [`full_coverage.md`](m1/properties/full_coverage.md).

## 1. Best parameters per model

| model | mechanism | best parameters | why these | P(bad) |
|---|---|---|---|---|
| [M1](m1/properties/README.md) | push | F = 24 | smallest F (F = 23 → 1.6×10⁻⁴) | 7.3×10⁻⁵ |
| [M2](m2/properties/README.md) | pull | RF = 24 | smallest RF (RF = 23 → 1.6×10⁻⁴) | 7.3×10⁻⁵ |
| [M3](m3/properties/README.md) | pull + initiation links | RF = 12, s = 8 | bandwidth-minimal split of the smallest budget RF+(s−1) = 19 | 7.8×10⁻⁵ |
| [M4](m4/properties/README.md) | undirected flood | RF = 8 | smallest RF (RF = 7 → 7.5×10⁻⁴) | 6.8×10⁻⁵ |
| [M5](m5/properties/README.md) | directed k_in/k_out | (k_in, k_out) = (9, 8) | most-balanced split of the smallest budget k_in+k_out = 17 | 4.4×10⁻⁵ |

## 2. The comparison

| model | parameters | msgs / message | copies / honest node | hops (full) | hops (mean) |
|---|---|---|---|---|---|
| **M3** | RF = 12, s = 8 | **153 570** | **9.6** | 5.9 | 4.3 |
| M4 | RF = 8 | 188 795 | 11.8 | 5.1 | 4.1 |
| M5 | (9, 8) | 217 562 | 13.6 | 5.0 | 3.9 |
| M1 | F = 24 | 307 202 | 19.2 | 5.0 | 3.6 |
| M2 | RF = 24 | 307 153 | 19.2 | **4.8** | **3.6** |

**Bandwidth: M3 wins decisively** — 19 % below M4, 29 % below M5, half of
M1/M2. **Latency: M2 wins, marginally** — the whole field spans only ~1.2
hops (4.8–5.9 full coverage; ~0.1–0.4 s at WAN per-hop times of 100–300 ms),
while bandwidth spans 2×.

## 3. Node degrees (standing links per node)

From each model's [`node_degrees.md`](m3/properties/node_degrees.md): the
mean total degree under protocol-compliant link opening is exactly 2× the
nominal budget (every link has a chooser and an acceptor); the maximum is a
balls-in-bins tail over the accepted side (measured, 25 graphs):

| model | chosen (held, det.) | honest in / out (mean) | max observed | compliant total (mean) |
|---|---|---|---|---|
| **M4** | 8 | 12.8 / 12.8 (same links) | 29 | **16** |
| M5 | 9 in + 8 out | 13.6 / 13.6 | 33 | 34 |
| M3 | 12 in + 7 out | 15.2 / 15.2 | ~36 accepted | 38 |
| M1 | 24 out | 19.2 / 19.2 | 41 | 48 |
| M2 | 24 in | 19.2 / 19.2 | 41 | 48 |

**On this axis the ordering flips: M4 wins decisively** — 16 links per node
vs M3's 38 (2.4×) and M1/M2's 48 (3×), with the smallest worst-case node
(29 vs 33–41). Note M3's degree exceeds its bandwidth: 14 of its 38 links
(the initiation kind) carry only their owner's publications — cheap in
traffic, but still held state, connection slots, and churn surface. In M4
and M5 every held link also carries relay traffic.

## 4. Degradation under μ-shift (frozen parameters)

From each model's
[`mu_shift_robustness.md`](m3/properties/mu_shift_robustness.md): the
operating points frozen, the effective adversarial fraction swept upward
(law-read, MC-validated at elevated μ). Reported: the **budget** (largest
μ_eff keeping P(bad) ≤ 10⁻⁴; churn reading p_max = Δμ/(1−μ)) and the
**collapse point** (P(bad) = ½):

| model | parameters | budget μ_eff (Δμ) | churn p_max | collapse μ_eff |
|---|---|---|---|---|
| M5 | (9, 8) | 0.217 (+0.017) | ~2.2 % | 0.49 |
| M1 | F = 24 | 0.214 (+0.014) | ~1.8 % | **0.61** |
| M2 | RF = 24 | 0.214 (+0.014) | ~1.7 % | **0.61** |
| M4 | RF = 8 | 0.209 (+0.009) | ~1.1 % | 0.50 |
| **M3** | (12, 8) | **0.204 (+0.004)** | **~0.5 %** | 0.44 |

**The robustness ordering is roughly the bandwidth ordering reversed: M3,
the bandwidth winner, is the most μ-brittle** — its μ^RF in-term has
log-sensitivity RF/μ = 60 vs 24 for M1/M2's exponential terms. M5's
top budget is mostly margin (its cheapest integer point lands 2.3× under
δ), not structure (sensitivity ≈ 50). M1/M2 degrade most gracefully
(collapse ≈ 0.61) — a cushion bought by their 2× bandwidth. On the live
frontier: **M4 tolerates ~2× more shift than M3** (~1.1 % vs ~0.5 % churn)
before leaving the target — though §5 shows this gap is a property of
M3's bandwidth-minimal split, not its mechanism: the same-budget re-split
(13, 7) reverses it.

## 5. Re-provisioning — the robustness-adjusted frontier

Where §4 asks how a fixed deployment degrades as μ_eff rises, this
section asks the converse: what it costs to *provision for* a higher μ
up front. From each model's
[`re_provisioning.md`](m3/properties/re_provisioning.md): the coverage
laws inverted at design fractions μ_design > 0.2 (splits per each
model's documented rule; costs are closed forms, simulator-checked
within 0.05 %). Baseline μ = 0.2 points in §§1–4.

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

**B — premium over each model's μ = 0.2 point** (Δmsgs / Δmean links):

| model | 0.225 | 0.250 | 0.300 | 0.350 |
|---|---|---|---|---|
| M3 | +2 % / +5 % | +3 % / +11 % | +8 % / +21 % | +5 % / +37 % |
| M4 | +6 % / +13 % | −1 % / +13 % | −4 % / +25 % | −8 % / +38 % * |
| M5 | −1 % / +6 % | −2 % / +12 % | −5 % / +24 % | −11 % / +35 % |
| M1 | −2 % / +4 % | −5 % / +8 % | −14 % / +13 % | −20 % / +21 % |
| M2 | −2 % / +4 % | −5 % / +8 % | −14 % / +13 % | −20 % / +21 % |

(\* +1 % / +50 % with the tail-corrected RF = 12.) Absolute bandwidth
mostly *falls* — H = (1−μ)N shrinks faster than the budgets grow — so
copies per honest node is the honest cost axis. **Robustness is bought
almost entirely in state**: +21–50 % more standing links at 0.35.

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
*equal robustness* the choice also holds: M3's re-splits match or beat
M4's μ-budgets at 0.2 and 0.25 for 11–16 % less bandwidth (only M4's
fresh RF = 9 at 0.225 holds more headroom, at +19 % bandwidth).
Weighting robustness does not reopen the M3/M4 choice — it changes
which *split* of M3's budget to deploy. M3's bandwidth-minimal split
stays the family's most μ-brittle point at every μ_design; M1/M2 keep
the deepest collapse cushion; M5 remains best on no axis.

## 6. Adaptive eclipse cost (corruptions to strand a victim)

From each model's
[`adaptive_eclipse_cost.md`](m3/properties/adaptive_eclipse_cost.md): once the
epoch's draws are public, stranding a victim costs its honest degree on the
attacked side — **deafen** (cut honest in-edges, the victim misses some
publisher) or **mute** (cut honest out-edges, its publications reach nobody).
Coverage fails either way. Min-cut equals degree here: at branching factors
of 12.8–19.2 the depth-2 shell dwarfs the depth-1 shell, so Menger's
disjoint-path count saturates at the degree.

| model | parameters | deafen | mute | **A: chosen victim** | **B: any victim** | B via |
|---|---|---|---|---|---|---|
| **M3** | (12, 8) | 9.6 | 15.2 | **9.6** | **3.3** | deafen |
| M4 | RF = 8 | 12.8 | 12.8 | 12.8 | 3.6 | either |
| M5 | (9, 8) | 13.6 | 13.6 | 13.6 | 3.7 | joint |
| M1 | F = 24 | 19.2 | 19.2 | **19.2** | 4.6 | deafen |
| M2 | RF = 24 | 19.2 | 19.2 | **19.2** | 4.6 | mute |

**The two threat models rank the family differently, and the gap is 3–4×.**
A *chosen* victim pays its own draw, so M3 is the cheapest target and M1/M2
the dearest — this is the reading that "partially reverses the frontier",
since the bandwidth winner is the cheapest target. But an adversary content
with *any* victim shops the lower tail across 16 000 nodes and pays the
network minimum, which is 2.9–4.2× below the mean in every model. Against
an adversarial budget of μN = 4 000, **no model costs more than ~5
corruptions to break the δ guarantee somewhere.**

**Re-provisioning (§5) raises these prices in step with degree.** The cost
of a chosen victim is a degree read, so §5's hardened points are also
dearer to eclipse: M3's same-budget re-split (13, 7) moves its deafen cost
from 9.6 to 10.4, and M4's +1-notch RF = 9 moves its cost from 12.8 to
14.4. The ordering is unchanged — M3 stays the cheapest target — and M4's
margin over M3 on this axis widens (3.2 → 4.0 corruptions).

**Chosen links beat accepted links at equal mean.** M1 and M2 have identical
mean degree (19.2) on both sides and identical bandwidth, yet their
*directions* differ by 2.3×: a node **chooses** its own picks and always
holds exactly F of them, so only adversarial thinning applies and the law is
binomially concentrated (sd 1.96); it does **not** choose who picks *it*, so
the accepted side is a balls-in-bins draw with a Poisson lower tail (sd
4.38). M1 is therefore cheap to deafen (accepted in-side) and dear to mute;
M2 is the exact mirror. Both end at the same guarantee-breaking cost of 4.6,
so on this axis **M2 does not dominate M1** — it relocates the weakness from
the receiving side to the publishing side.

This also splits the two weak models by cause: **M3's problem is level**
(its in-degree is chosen and tightly concentrated at sd 1.39, simply low at
9.6, so more RF is the fix), while **M1's is spread** (high mean, fat tail,
so converting accepted in-links to chosen ones is the fix). Different
remedies for the same symptom.

**Correction.** [`candidate_properties.md`](candidate_properties.md)
previously estimated this property at "M3 9.6, M5 13.6, M1/M2 19.2, M4 25.6"
and placed M4 at the safe end. The other four figures transcribe measured
means from each model's `node_degrees.md`; M4's does not — that file has
measured 12.80 since the models were first published, and 25.6 is exactly
twice it, i.e. 4·RF(1−μ), consistent with the 2× in the closed form being
applied a second time. The figure appears in no script or table anywhere in
the repository. Read as the honest degree, which is what the property costs,
12.80 is the defensible value; with the order statistic then applied, M4
moves from most eclipse-resistant to second cheapest.

## 7. Bottom line

At P(bad) ≤ 10⁻⁴, N = 20 000, μ = 0.2: **M3 (RF = 12, s = 8) is the most
efficient model in bandwidth** — cheapest by 19–50 %, within ~1 hop
(~0.1–0.4 s) of the fastest, and continuously tunable toward the latency
corner without leaving the target. **M4 (RF = 8) is the most efficient in
per-node state** — 2.4× fewer standing links than M3 with a single mechanism
and one link type — at ~23 % more bandwidth and near-identical latency. The
practical choice is M3 if bandwidth is the binding resource, M4 if
connection count / simplicity is — a choice §5 shows is stable under
re-provisioning: M3 keeps the bandwidth lead at every analysed
μ_design ≤ 0.35, and its μ-brittleness (§4) is cured by the (13, 7)
re-split rather than by switching models. Note, however, that both
leaders are also the two cheapest to attack (§6), so a design that
weights adversarial cost alongside efficiency would reopen the choice.
Of the rest, M1 is weakly dominated by M2 on bandwidth, latency and
state — **but not on eclipse cost, where §6 finds them equal at 4.6**,
differing only in which side gives way (M1 is deafened, M2 muted); M2's
marginal latency win (0.2 hops) costs 2× the bandwidth and 3× the
standing links of the leaders; M5 is best on no measured axis.
