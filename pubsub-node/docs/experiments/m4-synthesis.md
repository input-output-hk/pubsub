# The gated + capped M4 recommendation — synthesis

> Reviewed and committed 2026-08-18. Eleven pre-registered cells
> (seeds 1139–1149), every one verified against its frozen
> registration; two coverage lines missed as recorded, corrections
> documented in §8.

The question: the CIP's M4 finalist runs ungated and uncapped — simple
to flood. What do the hash gate and the acceptance cap cost and buy at
the CIP's own operating shape (N = 20 000, K = 9, μ = 0.2), and what
parameter set should a gated deployment use? This pass composes the
measured results of E10 (gated selection fidelity), E12 (flooding under
the cap, directional), E18 (gated-symmetric coverage), and E19
(symmetric flooding under the admissions budget) through an
(N, K)-parameterised prediction ledger, and anchors the answer with
eleven pre-registered cells at CIP scale and at the CIP's pick count.

## Provenance

Instrument: `6d0385b` (the E19 instrument) plus one detail-only
instrument commit on this branch, `048457f` (the N-041 publisher slot
pair — spot-checked byte-identical on all seven baseline sweeps);
every other commit is configs/docs/python (byte chain in
`notes/experiments-baselines/`). Ledger:
[`m4_synthesis_predictions.py`](m4_synthesis_predictions.py) — the
E18/E19 forms with N and K lifted to parameters; it reproduces the
committed E19 ledger exactly at N = 4 000, K = 16 (both measured
capsweep anchors included) before any new number is quoted.

| cell | seed | manifest commit | runs |
|---|---|---|---|
| `parity-n20k-k10-b500-cap23` | 1139 | `546fb57` | 400 |
| `flood-n20k-k10-b500-cap23` | 1140 | `18b3368` | 400 |
| `parity-n20k-k9-b500-cap23` | 1141 | `79bbcff` | 400 |
| `cfloor-n4k-k9-b100-open` | 1142 | `79bbcff` | 2 000 |
| `cfloor-n4k-k9-b100-cap16` | 1143 | `79bbcff` | 400 |
| `cfloor-n4k-k9-b100-cap20` | 1144 | `79bbcff` | 1 000 |
| `m3-gated-n20k-k13-b1250` | 1145 | `93059f6` | 400 |
| `m4-twin-n20k-k10-b625` | 1146 | `93059f6` | 400 |
| `pubseam-n4k-k12-bp100-cap10` | 1147 | `0b8ac89` (instrument `048457f`) | 400 |
| `pubseam-mu40-n4k-k12-bp100-cap10` | 1148 | `304bc80` (instrument `048457f`) | 400 |
| `m3-oppoint-n20k-k13-b769` | 1149 | `69dab5b` (instrument `048457f`) | 400 |

Configs in `configs/experiments/m4-synthesis/`; per-cell predictions
frozen in the config comments before each cell ran; folds by
`summarise_symmetric_flooding_cell.py`. The ungated reference row is
the maintainer's `comparisons/m4-n20k-rf9.toml` measurement (seed 851,
200 runs — cited, not re-run).

## 1. The parameter recipe: what each knob owns

There is no unconstrained optimum in (B, C) — coverage and flood
resistance are monotone in B in opposite directions. The recipe that
does have a unique answer, given a reliability target and the CIP's
economics:

1. **K sets the reliability floor.** The ungated law's own failure
   core is μ^K·e^{−K(1−μ)} ("all my picks landed adversarial, and
   nobody honest picked me"), and the gated form contains it at every
   B: as B → 1 the gated law recovers the ungated law exactly, from
   above. One pick buys ≈ one decade of floor at μ = 0.2: the per-pick
   factor is μ·e^{−(1−μ)} ≈ 1/11 — 0.7 decades from the pick arm and
   0.35 from the inbound arm. K is the only knob that buys
   reliability; it is paid in bandwidth (copies, standing links) — the
   CIP's economic axis.
2. **B is chosen maximal subject to the coverage target**: the largest
   gate width whose gated P(bad) meets the target. B is the flood
   divisor — per-identity admissibility 1/B, attack cost per victim
   edge ∝ B — so maximizing B maximizes the attacker's price.
3. **C is chosen minimal subject to coverage-neutrality**: the
   admissions budget (ADR 0042) sized at fresh-arrival load + c·√load,
   with the **K-dependent** headroom floor of §5 (c ≈ 2 at K = 16,
   c ≈ 3.5 at K = 9–10). Tighter is not safer (the composition
   channel); looser is inert.

## 2. The gate's cost at the CIP's pick count — and the one-pick fix

E18's "coverage-free at r ≳ 3" rule was measured at K = 16, where the
all-picks-adversarial channel (μ^K ≈ 10⁻¹¹) is invisible. At K = 9 its
μ^K factor grows to 0.2⁹ ≈ 5×10⁻⁷ — large enough that, multiplied by
the channel's other arm (no honest member picked me) and the 16 000
honest nodes, it re-enters the budget (the table's exact form) — and
the rule does not transfer — the B ladder at N = 20 000, K = 9 (ledger,
validated by the cells below):

| B | 250 | 400 | 500 | 625 | 740 (r = 3) |
|---|---|---|---|---|---|
| P(bad) | 1.4×10⁻⁵ | 2.5×10⁻⁵ | 3.6×10⁻⁵ | 6.0×10⁻⁵ | 1.0×10⁻⁴ |

against the ungated law's 6.1×10⁻⁶: the gate at K = 9 costs 2–17× in
reliability headroom depending on B, and no B recovers parity — the
floor argument of §1. One extra pick does: at K = 10, B = 500 the
gated law reads 5.1×10⁻⁶ — **parity with the ungated CIP point, with
the full flood divisor intact.**

Measured, all three rows at CIP scale, same instrument:

| row | P(bad) law | measured | degree |
|---|---|---|---|
| ungated K = 9 (the CIP op point, seed 851) | 6.1×10⁻⁶ | 200/200 good | 18.0 |
| gated + capped K = 9 (seed 1141) | 3.6×10⁻⁵ | 400/400 good | 15.975 |
| gated + capped K = 10 (seed 1139) | 5.1×10⁻⁶ | 400/400 good | 17.501 |

Geometry in both gated cells matched the ledger to the third decimal
(routes, the binomial-at-μ class split, refusals ≈ 0 by construction,
max degree exactly K + C). And the cost columns — extracted from the
same run rows, in the m4-comparison's own units — show the armor is
**free on every CIP axis but one**: the shared-pool geometry deflates
realised degree enough to cancel the extra pick, and the single
exception is mean first-receipt latency, ~1 % slower (3.95 vs 3.90
hops; full-coverage hops equal).

| quantity (the CIP's units) | ungated K = 9 (published) | gated K = 10, B = 500, C = 23 (measured, seed 1139) |
|---|---|---|
| P(bad), law | 6.1×10⁻⁶ | **5.1×10⁻⁶** |
| honest→honest sends per message | 214 345 | 207 990 |
| copies per honest node | 13.40 | **13.00** |
| standing links, mean | 18.0 | 17.5 |
| hops, full coverage / mean first receipt | 5.00 / 3.90 | 5.00 / 3.95 |
| churn budget to 10⁻⁴ | 7.43 % | **7.57 %** |
| flood divisor / per-identity admissibility | none | 500 / 1⁄500 |
| degree ceiling | none | 33 = K + C exact |

(Churn budgets read off the coverage law at the shifted adversarial
fraction, the churn-tolerance.md manner — the method reproduces the
published ungated 7.43 % at 7.44 %. The gated K = 9 row's churn budget
is 2.65 %: the reduced-headroom cost restated on the operational
axis.) The framing for the CIP: **the armor's total price is ~1 % of
mean latency — one extra pick buys the gate, the cap, slightly lower
bandwidth, and a slightly better tail and churn budget than the point
the CIP already chose.**

## 3. The flooding anchor: the E19 machinery at CIP scale

`flood-n20k-k10-b500-cap23` (seed 1140) runs the recommendation
coordinates under the wholesale flooder (all 4 000 adversaries dial
every admissible pair, uncapped, silent). Every registered column hit
(|z| ≤ 1.4): fresh loads 6.00 + 6.00 raced against C = 23 with
honest-class refusals at 0.0025/victim (40.6/run; the adversarial
class matches it, ~81/run over both classes), admitted 5.997/5.997, Sybil
occupancy 7.997 = the cap-blind own-pick floor 2.000 + the
gate-divided admitted route 5.997, crossings refused ≡ 0, max degree
33 = K + C exact, own-only Sybil ≡ 0 (the pair draw), coverage
400/400 at the composed law 1.25×10⁻⁵.

Attacker accounting at the recommended point, per honest victim: the
floor K·μ = 2.0 is admission-free (the victim's own picks at ambient
composition — no acceptance policy sees it); the admitted route is
bounded by min(fair-race share, C) and priced by B (fresh pressure
(S/B)(1−m) = 6.0 even with every adversarial identity flooding); the
budget's invariant degree ≤ K + C holds exactly. Attack cost scales
∝ B per victim edge; identities are deposit-priced, so B = 500 is the
defense's economic statement.

## 4. The cap trade-off (capsweep) at CIP scale

The ledger's `capsweep` walks the whole C ladder from the
grid-validated race law (the E19 §6 first-order composition form,
measured at both ends on the E19 branch and re-validated at K = 9 by
§5). At the recommendation shape (K = 10, B = 500, load = 12.0),
under wholesale flooding:

- C = 16 spends coverage measurably (ΔE_iso = 5.4×10⁻⁴ — two decades
  over the uncapped law); tighter caps escalate from there;
- C = 19 (c = 2) composes to 1.02×10⁻⁴ — **landing exactly on the CIP
  target with zero margin: the E12 c ≥ 2 floor does not transfer to
  small K**;
- **C = 23 (c ≈ 3.2)**: composed 1.25×10⁻⁵ — 8× inside the target
  (measured 400/400, seed 1140) — with the ambient contribution
  unmeasurable (the parity cell's 25 refusals over 6.4 M victim rows);
- C = 25 (the ceiling of the c ≈ 3.5 sizing; exactly c = 3.75) is
  strictly neutral even under attack (ΔE_iso ≤ ⅓ of the law); C ≥ 28
  is inert.

The criterion behind "coverage-neutral", pinned: ambient contribution
below measurement resolution AND the composed under-attack law inside
the CIP target with material margin. C = 23 is the smallest cap
meeting that; the §1/§9 headroom floor **c ≈ 3.5 is the stricter
figure** (ΔE ≪ E even under wholesale attack, C ≈ 25 here) — the
recommendation's C = 23 trades that last ~2× of composed under-attack
tail for a tighter admissions bound, and either choice is defensible.

## 5. The c-floor rehearsal: the composition form at K = 9, measured

Three points on the composition curve at the CIP pool geometry scaled
to N = 4 000 (K = 9, B = 100, μ = 0.4, λ = 40 — seeds 1142–1144):

| point | registered P(bad) | measured | z |
|---|---|---|---|
| open (baseline; the K = 9 two-channel law's first measurement) | 0.0102 | 24/2000 | +0.81 |
| C = 16 (strong composition, ρ = 0.123) | 0.1433 | 53/400 | −0.62 |
| C = 20 (mid-slope, ρ = 0.035) | 0.0346 | 20/1000 | −2.52 |

The race columns are exact in all three cells (every |z| ≤ 0.5;
crossings ≡ 0; max degree = K + C exact), so ρ — the form's input —
is measured precisely as predicted everywhere. The mid-slope point's
composition *increment* ran at ~40 % of the first-order prediction
(the point itself at 58 %): z = −2.52, inside
the program's |z| ≤ 2.6 convention but outside the Wilson interval,
and recorded here as a characterized bias rather than noise — **the
first-order composition form is conservative at mid-slope** (it
understated slightly at E19's deep-binding corner, was exact at the
quiet end, and overstates between). Consequences: the K-dependent
headroom floor c ≈ 3.5 stands and errs safe (caps sized by the form
overprotect slightly); the form is a design upper bound, not a
precision law; no CIP-scale c-cell is needed.

## 6. The recommendation (both rows measured; the choice is the CIP's)

- **K = 10, B = 500, C = 23** — equal or better than the ungated CIP
  point on every quoted axis except ~1 % of mean latency (§2's table:
  tail, copies, links, full-coverage hops, and churn budget all equal
  or better; mean first receipt 3.95 vs 3.90 hops), plus the flood
  divisor 500, admissions bounded at 23, and the degree ceiling 33.
  The extra pick is absorbed by the shared-pool degree deflation; net
  bandwidth is slightly *lower* (13.00 vs 13.40 copies/honest).
- **K = 9, B = 500, C = 23** — the CIP's own pick count: cheapest
  (11.78 copies/honest measured), headroom reduced 6× (3.6×10⁻⁵) and
  churn budget reduced to 2.65 %, all defense properties equal.
- Reference: ungated K = 9 — cheapest, and floodable at K
  concentration per victim with no admission control (E12's
  documented-not-simulated row).

## 7. M3 under the same armor: the pair draw buys twice the pool per
   unit of attack surface

The fair comparison is between armored models — the ungated comparison
graph answers a different question, since every ungated model is
floodable at will. The ledger's directional forms (`directional
isolation`: the deaf coin = all gated picks adversarial, hypergeometric
over the pool; the mute coin = no honest node picked me; independent
per direction) are validated against E10's measured points before use:
the ungated M2 law (computed 0.0086 vs 0.0088), gated picks at r = 2
(0.00859 vs the measured pooled 0.00872), the gate-only doubling at
B = 250 (0.0171 vs measured 0.0193, inside Wilson), the B = 235
+1-link compensation (0.0076 vs E10's 0.0079, measured 0.0085), the
r = 1 cliff (4.4× vs E10's ≈ 5×), and both published N = 20 000
ungated operating points (M2 K = 24 → 7.3×10⁻⁵; M3 K = 12, s = 8 →
7.8×10⁻⁵). M3's mechanics, per the fan-out seam itself
(`strategies/fanout/forward_to_relays.rs`): every node
holds s−1 seeding picks; own publications ride relay downstream ∪
seeds (so the mute channel is seed-rescued — the cross-seam product);
the deaf channel is **not** rescued (seeds carry only the seeder's own
publications, and a deaf node fails the every-publisher check for
every publisher that did not seed it directly); two independent gates
(B_relay, B_publisher); direction inversion (relay acceptor caps
downstream serving, publisher acceptor caps upstream seed intake).

**The normalization, argued — and the alternative, named.** The
table fixes attack surface per deposit-priced identity on the relay
seam: the number of victims one identity can approach in any role.
The pair draw's one coin covers both directions — surface = (N−1)/B.
The directional draw's two independent coins pay twice — surface =
2(N−1)/B_r (measured: the E19 ordered arm's ≈ 2/B admissibility).
Equal surface therefore means B_r = 2·B_pair, which runs the
directional pools at **half** the pair draw's pool size. The argument
for this choice: deposits price identities, not attack modes, and an
identity's marketable asset is its total reach — the being-picked
direction (deafening a victim) and the picking direction (occupying a
victim's serving slots, the harm E12 measured as starved honest
links) are both attacks. The natural alternative — equal cost to
deafen a chosen victim, i.e. B_r = B_pair — concedes the directional
attacker double the reach per deposit; under it, gated M3 at K = 13,
s = 7, B = 500 prices at 4.4×10⁻⁵ (the table's derived row):
feasible, ~9× behind gated M4's 5.1×10⁻⁶, at 19 picks against 10 and
surface 80 against 40. The infeasibility row below is therefore a
statement under the total-surface normalization; the dominance
direction is robust to either choice.

The ledger's `compare` mode (N = 20 000, μ = 0.2, target ≤ 10⁻⁴):

| configuration | P(bad) | surface | provenance |
|---|---|---|---|
| M4 ungated K = 9 (the CIP op point) | 6.1×10⁻⁶ | open | law; measured 200/200 |
| M4 gated K = 9, B = 500, C = 23 | 3.6×10⁻⁵ | 40 | measured 400/400 |
| **M4 gated K = 10, B = 500, C = 23** | **5.1×10⁻⁶** | **40** | measured 2× 400/400 |
| M3 ungated K = 12, s = 8 (op point) | 7.8×10⁻⁵ | open | law; published row |
| M3 gated K = 13, s = 7, B = 769 (r = 2 max) | 5.8×10⁻⁵ | 52 | measured 400/400 (seed 1149; degree 37.98 vs ~38.0, both seams' geometry on the ledger) |
| M3 gated K = 12, s = 8, B = 833 (r = 2) | 1.5×10⁻⁴ ✗ | 48 | derived |
| M3 gated K = 13, s = 7, B = 500 (equal deafen-cost) | 4.4×10⁻⁵ | 80 | derived; the alternative normalization — feasible, 19 picks vs 10 |
| M3 gated at M4-equal surface (B = 1000) | 1.8×10⁻³ ✗ | 40 | derived; **no pick count meets the target here** |

Three structural findings:

- **At M4-equal total attack surface, gated M3 is infeasible.**
  Surface 40 forces B_r = 1000 → pools of 20; the deaf channel is
  pool-limited and no K repairs it (K = 9…13 all land at
  2×10⁻³–10⁻² — raising K past the pool does nothing, lowering it
  re-opens μ^K; the K-independent floor is channel A's
  H·e^{−(1−μ)λ} ≈ 1.8×10⁻³ at λ = 20). The pair draw gets pools of 40
  at the same surface. This is a geometry statement, not a tuning
  gap — scoped to the total-surface normalization, per the paragraph
  above.
- **M3's best compliant gated point trades worse on both axes at
  once**: K = 13, s = 7, B = 769 carries 30 % more attack surface than
  gated M4 (52 vs 40) at 11× worse reliability (5.8×10⁻⁵ vs
  5.1×10⁻⁶) — while spending 13 relay picks + 7 seeds against M4's 10
  picks in bandwidth.
- **M3 armors two surfaces.** Its publisher seam adds its own contact
  surface 2(N−1)/B_p (seed-intake attacks — a different threat, not
  summed into the table), measured capped for the first time by §8's
  cells (N-041), which found the seed-rescue coupling that seam's cap
  must be sized against; M4's reciprocity collapses deaf/mute into one
  channel with one gate, one cap, and the measured budget semantics.

**The equal-surface separation, measured.** The one point where both
curves are testable is attack surface 32, and the pre-registered pair
(seeds 1145–1146, the first gated two-seam configuration ever run)
measured it:

| cell | registered | measured |
|---|---|---|
| gated M3, K = 13, s = 7, both seams B = 1 250 | P(bad) = 0.0431 → 17.2 bad/400; ≥ 99.8 % deaf-class | **17/400 bad (z = −0.06); 17 of 17 deaf-class** (every bad run at min-publisher-coverage 19/20 — one or two unreachable receivers, missed_hist {1: 16, 2: 1}; zero stranded publishers — mute seed-rescued as predicted); standing degree 37.0 |
| gated M4, K = 10, B = 625, C = 23 | 9.7×10⁻⁶ → 400/400 good | **400/400**; d = 16.876 vs 16.87; routes 6.876/3.124/6.876 vs 6.87/3.13/6.87; 13 refusals/6.4 M rows; crossings ≡ 0; max degree 33 = K + C exact |

At identical attack surface, the directional design fails at a
measured 4.25 % per run while the pair draw measures zero failures in
400 — a direct count separation bounded below by the Wilson intervals
alone (M3 ≥ 0.0267 vs M4 ≤ 0.0095), and at the laws both measurements
are consistent with, a factor of ≈ 4 400. The pool-limited deaf
channel — the mechanism behind the comparison's infeasibility row — is
now a measured fact at CIP scale, failure mode included.

Verdict for the CIP debate: **under armor, M4's dominance widens, and
its central mechanism is measured** — the ungated comparison already
favored M4 on cost; the gated one adds that at equal total reach per
identity M3 cannot reach M4's attack-cost-per-reliability point at
all (and remains ~9× behind at 19 picks vs 10 under the alternative
normalization), with the equal-surface gap
demonstrated by the cliff pair, the best compliant M3 row anchored at
its own coordinates (seed 1149), and the publisher seam's cap measured
by §8. Every feasible row of the table above is measured; the two
infeasible rows are derived from the same forms the cliff pair
anchored.

## 8. The publisher seam under its cap: the seed-rescue coupling

The first capped publisher-seam experiments in the program (N-041's
named surface, run at the commit landing its detail slot pair —
`downstream_publisher_honest`/`_adversarial`, all seven baseline
sweeps byte-identical). The seam is inverted: the victim's cap governs
its seed **intake**, so refusals starve the **dialers'** first-hop
reach — the mute-side harm.

- **The race transfers exactly.** Both cells' refusal columns landed
  on their registrations to 0.1 % (seed 1147: 10 223/run vs 10 216;
  seed 1148: 23 060 vs 23 063), and the new slot columns folded on
  their registered values with run-clustered SEs — seed 1147:
  publisher downstream h/a = 3.5991 ± 0.0019 / 1.1995 ± 0.0008 vs the
  registered ~3.60/1.20; seed 1148: 1.8320 ± 0.0022 / 2.4010 ± 0.0012
  vs ~1.83/2.40 (the summariser folds the pair; the summaries preserve
  them). The publisher cap-sizing rule is the same fresh-arrival +
  headroom form with the intake load (s−1)-driven.
- **The registered coverage line missed, and the miss is the
  finding.** Seed 1147 measured 2 mute-class bad runs against a
  registered ~0.01: the class-blind rescue model was wrong because the
  seam's refusals hit exactly the rescuing seeds — an adversarial seed
  target accepts and sits silent, an honest one refuses at ρ_p — so a
  seed fails at **f = μ + (1−μ)·ρ_p** per pick and rescue-failure
  compounds as f^(s−1): a binding seed-intake cap reaches coverage
  through the mute channel, the inverted-seam analogue of E19 §6's
  composition term. Design rule: **size C_p to clear the intake load
  or the armor eats the first-hop rescue.**
- **The corrected form, tested out-of-sample where it is powered
  (seed 1148, μ = 0.4, ρ_p = 0.49), was itself exceeded — by the
  instrument, quantifiably.** Measured 188/400 bad (181 mute / 7
  deaf) against the corrected independent-order form's 84.8 and the
  refuted class-blind form's 18. The rank dissection (regenerated
  detail) reproduces N-042's signature exactly: seed-dial losses are a
  step function of within-run rank (bottom deciles lose ~0 of ~3.6
  honest seed dials, top deciles ~all; honest seed targets kept fall
  3.58 → 0.04; every sampled stranding in the top four rank deciles;
  ~2× the independent-order stranding count). This cell is the first
  to sit in N-042's trigger condition — a per-node tail under a
  saturated budget — and fires it: real decorrelated networks sit at
  the corrected form (~0.21 here); the canonical order's 0.47 is the
  instrument's amplified upper bound. The parked per-victim-seeded-
  order fix now has two falsifiable consumers.

## 9. The gated closed forms

The gated analogues of the formal folder's ungated laws, as this
program has derived and measured them. Two epistemic grades appear,
and the table says which is which: **compact** forms (closed
expressions in the formal team's style, exact or first-order as
noted) and **exact-arithmetic** forms (finite enumerations over the
pool distributions — deterministic arithmetic with no simulation in
it, the same evidentiary standing as a closed form, just not
compressible to one line). Everything below lives executably in
[`m4_synthesis_predictions.py`](m4_synthesis_predictions.py); B = 1
recovers each model's published ungated law (verified: M4 K = 9 →
6.07×10⁻⁶; M2 K = 24 → 7.3×10⁻⁵; M3 K = 12, s = 8 → 7.8×10⁻⁵).

Symbols, used in both tables:

| symbol | meaning |
|---|---|
| N, S, H = N − S, μ = S/N | population, adversarial count, honest count, adversarial fraction |
| B, λ = (N−1)/B, r = λ/K | gate width, pool mean, pool headroom per pick |
| K, C, c | pick count, admissions budget, cap headroom (C = load + c·√load) |
| h ~ Bin(H−1, 1/B), a ~ Bin(S, 1/B), n | honest / adversarial pool parts; n the pool size one draw sees |
| m = E[min(K, n)/n] | member-pick probability (marginal: a given pool member is picked) |
| mm = min(K, n)/n | the same rate, conditional on one pool realisation |
| ρ, σ_ρ | honest refusal share under a binding cap; per-member live-link probability at that share (σ_0 = the uncapped value) |
| s, B_p, C_p, m_p, ρ_p | seed count (s − 1 seed picks) and the publisher seam's width, budget, pick probability, refusal share |
| m_d | the directional dial-pick probability (relay seam) |
| p, p_max, μ_eff = μ + p(1−μ) | honest downtime fraction, its budget, the shifted fraction ([churn-tolerance.md](churn-tolerance.md)) |
| δ | the reliability target (P(bad) ≤ δ) |

**M4 (symmetric, unordered pair draw)** — one seam, one coin per pair:

| law | form | grade | measured |
|---|---|---|---|
| realised degree | d = λ·m·(2−m) | compact, exact | E18 to 3 digits across B = 10–500; this pass at N = 20 000 to the 3rd decimal |
| isolation (two channels) | E_iso = H·Σ_{h,a} P(h)P(a)·(1−m)^h·[h = 0 or all min(K, n) picks adversarial (hypergeometric)] | exact-arithmetic; channel A compactly ≈ H·e^{−(1−μ)λ} | E18 (B = 250/500, μ-axis); the K = 9 law first measured by the c-floor baseline (z = +0.81); every anchor here |
| pair components (the reduction's leading correction) | E_pair: a mutually linked pair, both pools honest-dead otherwise (ledger `paircomp`) | exact-arithmetic | ≤ 3.3×10⁻⁴·E_iso at every shape in this report, ≤ 1.7×10⁻⁸ at the CIP candidates; the powered cells show zero pair excess |
| pool-floor rule | (N−1)/B ≥ ln(H/δ)/(1−μ) | compact, from channel A | E18's design rule |
| pick-decade rule | one pick ≈ one decade of tail at μ = 0.2: per-pick factor μ·e^{−(1−μ)} ≈ 1/11 (0.7 decades from the pick arm, 0.35 from the inbound arm) | compact, first-order | §2's ladder; K = 9/10 rows measured |
| admissions race | without-replacement pick split (hypergeometric) + fresh Bin arrivals, proportional refusals | exact-arithmetic | E19: zero flags across 19 cells; this pass at N = 20 000 |
| cap composition | ΔE_iso = H·Σ P(h,a)[(1−σ_ρ)^h − (1−σ_0)^h], σ = m·mm + (mm(1−m)+m(1−mm))(1−ρ) | first-order (upper bound; conservative at mid-slope) | E19 §6 both ends; the three-point K = 9 rehearsal |
| cap rule | C ≥ load + c·√load, load = fresh arrivals; c ≈ 2 at K = 16, **c ≈ 3.5 at K = 9–10** | compact, calibrated | E12 (K = 16); §5 (K = 9) |
| churn budget | read P(bad) at μ_eff = μ + p(1−μ), solve for the downtime budget p_max | compact composition | reproduces the published ungated 7.43 %; §2's gated budgets |

**M3 (directional relay + per-node seeding)** — two seams, two coins
per directed pair; deaf and mute are cross-seam products (hearing =
own relay picks ∪ inbound seeds; being-heard = inbound relay picks ∪
own seeds):

| law | form | grade | measured |
|---|---|---|---|
| deaf channel | E_deaf = H·E_{h,a}[C(a, k)/C(h+a, k)], k = min(K, h+a) | exact-arithmetic; compactly μ^K when pools ≫ K, **pool-limited when λ ≲ 2K** (floor H·e^{−(1−μ)λ}, K-independent) | E10 (r-ladder, K = 16); the cliff cell (17/400, all deaf, z = −0.06) |
| mute channel, unrescued | E = H·(1−m_d/B)^{H−1} ≈ H·e^{−(1−μ)·E[picks]} | compact, exact | E10's gate-only doubling and B = 235 compensation; M2's op point |
| seed rescue | × P(all min(s−1, pub-pool) seeds fail); per-seed failure **f = μ + (1−μ)·ρ_p** under a binding intake cap | exact-arithmetic pool part; f compact, first-order | §8: the class-blind form refuted, f measured directionally, magnitude instrument-bounded (N-042) |
| seed-intake race | wave-order tail E[(A − C_p)⁺], A = Bin(H−1, m_p/B_p) + Bin(S, 1/B_p) | exact-arithmetic | §8: refusals within 0.1 % at two shapes |
| headroom rule | r = (N−1)/(B·K) ≥ 2 for law-exact selection | compact | E10 (measured cliff at r = 1) |
| per-identity surface | directional ≈ 2/B per pair vs the pair draw's 1/B | compact | the E19 ordered arm; §7's normalization |

**The isolated-vertex reduction, scoped.** Every P(bad) above reads
1 − e^{−E}: the expectations count singly isolated vertices, so
failures via detached components of two or more nodes sit outside the
forms. The formal folder measured that under-count at ~1.1× in the
deep tail for the ungated laws, and both columns of §2's parity table
share the same reduction, so the comparison is unaffected by it. For
the pair draw the leading correction is enumerated (the E_pair row):
≤ 3.3×10⁻⁴ of E_iso at every shape in this report and ~10⁻⁸ at the
CIP candidates. The powered gated cells agree — across E18's B = 250
μ-axis cells and the c-floor rehearsal, every multi-victim bad run is
Poisson-consistent with independent single strandings (μ = 0.4,
B = 250: 8 runs at missed = 2 against 5.9 expected doubles), with no
pair-component excess anywhere. The directional op points inherit the
formal folder's own ~1.1× standing, unmeasured here. The remaining
independence assumptions — the two isolation arms treated as
independent, per-member pick marginals, the seed-failure product
f^{s−1} — have the same standing as in the ungated laws and are used
as assumptions, not results.

What this is **not**, stated plainly: a formal-methods derivation
document. The forms above were derived inside the experiments program
(first-order arguments + exact enumeration), validated against E10/
E18/E19/this pass's measurements and against the formal folder's
ungated laws at the B = 1 limit. An independent re-derivation from
the mechanism, with no shared code, has since reproduced every number
this report quotes from them (the PR's formal review) — which
upgrades their standing, but a derivation document with stated
assumptions and proofs in the formal folder's style remains the
hardening step and is listed in §11.

## 10. Scope

- **The attack unit**: the wholesale flooder (every adversarial
  identity floods) is the conservative convention throughout;
  deposit-priced attacker sizes refine every flooding statement
  linearly (identities are the costed unit — the E12/view-size
  framing). The CIP's deposit parameters would make the flooding rows
  concrete.
- **The composition form's bias** is characterized, not eliminated: it
  understated ~20 % at E19's deep-binding corner, was exact at the
  quiet end, and overstates at mid-slope (§5's z = −2.52) — used
  strictly as a design upper bound.
- **Per-node tails under saturated budgets are instrument-bounded, not
  point-measured** (N-042, whose trigger the §8 cells fired): the
  canonical arrival order amplifies them (measured ~2× at seed 1148's
  shape); real decorrelated networks sit at the independent-order
  forms. Every class-level column in this report is exact under either
  order. The fix is chartered as the next instrument pass with two
  frozen falsifiable re-runs.
- **Cells at CIP scale are law-consistency anchors**, not tail
  measurements: 400 runs resolve nothing below ~10⁻²·⁵ — the tail
  claims are the ledger's, anchored where the curves are powered (the
  cliff pair, the c-floor rehearsal, E18/E19's deep-tail cells).
- **All tail forms are isolated-vertex reductions**, shared with the
  formal folder's laws; the leading pair-component correction is
  enumerated and bounded negligible in §9, larger components are
  higher-order still.
- **One shape per finding**: the seed-rescue coupling is measured at
  (B_p = 100, C_p ∈ {10}, μ ∈ {0.2, 0.4}); the cliff at surface 32;
  the c-floor at one (B, μ) per K. Functional forms carry the rest.
- **Out of scope entirely**: small networks (deferred);
  retry/rotation and adaptive victims (N-037); the incentive/chain
  layer; view sampling (the E12-derived sizing note stands as
  exploration).

## 11. Next

- **A derivation document** for §9's forms in the formal folder's
  style. The PR's formal review re-derived them independently and
  reproduced every quoted number, and §9 now states the assumptions;
  the proofs document is what remains.
- **The instrument pass** (charter in
  `notes/m4-synthesis-followups.md`): per-victim seeded arrival order
  + the failure-severity batch, one re-baseline generation, validated
  by the two frozen re-runs (seed 1148 → ~84.8/400; the E19 ordered
  flooder → ≈ 400/400).
- This branch's PR (E19 is merged; the E20 program entry is landed);
  the comparison page's `web/experiments/` port proposed for review.
- Decisions this report presents but does not make: K = 10 gated at
  full parity vs K = 9 gated with priced headroom (§2/§6 carry both
  rows measured); the deposit-priced attacker size.
