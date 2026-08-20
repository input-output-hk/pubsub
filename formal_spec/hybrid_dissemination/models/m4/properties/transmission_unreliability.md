# M4 — transmission unreliability (per-message delivery under send loss)

**Verdict: HYBRID** — the validated μ-shift law re-read per message at
μ_eff = μ + (1−μ)p_fail with exact per-message accounting; loss-injected
MC at elevated cells (the law-vs-MC residual is the interior effect);
degree-mixture and p_fail = 0 anchors. Script (in `../scripts/`):
`sweep_m4_pfail.py`.

## 1. Property

What iid per-transmission loss does to the frozen operating point RF = 9
(N = 20 000, μ = 0.2, δ = 10⁻⁴ under the disturbance-margin rule; the
δ-cheapest point is RF = 8), and **what M4's bidirectionality actually
buys under it** — the backlog asserts "M4's doubled edge redundancy
should win this axis"; this analysis derives rather than assumes. Loss
model: every honest→honest send is dropped independently
with probability p_fail at send time; r per-link retries make the
per-send failure p_fail^(r+1); sends to adversaries never matter for
coverage. Correlated/bursty loss is out of scope (it reads as churn,
[`mu_shift_robustness.md`](mu_shift_robustness.md)).

**The semantics shift**: per-message loss randomness breaks the
per-epoch standing-structure dichotomy, so the headline quantities are
per-message — E[missed] and ε_msg = P(the message misses ≥ 1 honest
node) against the same δ; per-epoch and per-message numbers never share
a table without labels (motivating precedent: the PRISM study,
[`asym_report.md`](../../../prism/asym_report.md), found p_fail more
damaging than node loss at n = 6).

**Counting rule at p_fail > 0**: attempted sends keep the p_fail = 0
convention — fire once, send on every incident honest link except the
arrival link, duplicates included. An undirected link can now carry the
message once in each direction when the first crossing failed to deliver
(or was itself a duplicate); the arrival-link skip spares only the link
of the *first successful* delivery. Attempted and delivered copies are
counted separately.

## 2. Guiding formula

Per message the isolation event splits by direction: a node misses the
message when every delivery *into* it fails; the publisher is muted when
every send *out of* it fails. The two directions of an undirected link
are independent Bernoullis, so with P = p_fail^(r+1):

$$q_{\text{dir}} = \mathbb{E}[P^{D}]\;
\Bigl(1-(1-P)\tfrac{RF}{N-1}\Bigr)^{H-1},\qquad
\varepsilon_{\text{msg}} = 1-(1-q_{\text{dir}})^{H},$$

with D the honest count among the node's own RF picks (exact
hypergeometric; the binomial approximation gives the identity's
μ_eff^RF e^{−RF(1−μ_eff)} with μ_eff = μ + (1−μ)p_fail). At p_fail = 0
both factors reduce to the published p_isolated
([`full_coverage.md`](full_coverage.md)). Against the churn reading, H
does not shrink (a lossy node needs covering regardless), and the H
count stays H per message ((H−1) in-cuts + 1 publisher out-cut) — unlike the
two-class directed models, M4 keeps its per-epoch multiplicity almost
exactly, so correction (ii) buys it little.

**Where the redundancy actually sits.** At the final hop every model
gets one try per informed honest neighbour — an uninformed node never
fires, so the reverse direction of an undirected link is *not* a second
chance for it. M4's redundancy is a **degree effect**, already inside
the identity: all ~14.4 honest incident links deliver into a node (9 own
picks thinned by μ, plus ~7.2 accepted) — above M5's 13.6 and M3's
δ-cheapest 9.6 chosen-side pulls, below M1/M2's δ-cheapest 19.2. The
*interior pair-retry*
(B→A succeeding after a failed A→B once B is informed via elsewhere) is
a multi-defect/bulk correction with no single-defect analogue: the
elevated MC cells measure it as the law-vs-MC residual rather than
gating on it.

## 3. Results — law and MC

`sweep_m4_pfail.py` (law, fast by default):

| p_fail | μ_eff | E churn-identity (per-epoch acct.) | E[missed]/msg | ε_msg |
|---|---|---|---|---|
| 0 | 0.200 | 6.1×10⁻⁶ | 1.2×10⁻⁵ | 6.1×10⁻⁶ |
| 0.001 | 0.2008 | 6.3×10⁻⁶ | 1.3×10⁻⁵ | 6.3×10⁻⁶ |
| 0.005 | 0.204 | 7.5×10⁻⁶ | 1.5×10⁻⁵ | 7.5×10⁻⁶ |
| 0.01 | 0.208 | 9.2×10⁻⁶ | 1.9×10⁻⁵ | 9.3×10⁻⁶ |
| 0.02 | 0.216 | 1.4×10⁻⁵ | 2.8×10⁻⁵ | 1.4×10⁻⁵ |
| 0.05 | 0.240 | 4.3×10⁻⁵ | 9.0×10⁻⁵ | 4.5×10⁻⁵ |
| 0.10 | 0.280 | 2.3×10⁻⁴ | 5.2×10⁻⁴ | **2.6×10⁻⁴** |

(E[missed] runs at ≈ 2× ε_msg because a mute event costs all H−1 nodes
at once — rare, but heavy.) Every grid cell up to 5 % loss sits under δ
at r = 0; the 10 % cell is the first to breach it. **Budgets**:
churn-identity 7.43 % (the μ-shift reading, [`mu_shift_robustness.md`](mu_shift_robustness.md));
exact per-message **7.21 %** at r = 0 (slightly *tighter* — H unshrunk
outweighs the small out-term relief), 26.9 % at r = 1, 41.6 % at r = 2.
The δ-cheapest RF = 8 point has per-message budget 1.03 % (churn-identity
1.07 %).

MC (`--mc`, seed 20260813): **anchor** — exact equality with the plain
flood on 40/40 graphs; published costs reproduced (214 272 vs 214 433
msgs, −0.08 %; hops 5.00/3.91 vs 5.0/3.9). **Degree mixture** — the
hypergeometric ⊗ binomial honest-degree pmf validated class by class
(100 graphs, measurable classes |z| ≤ 1.9). **Elevated cells** — the
interior-effect measurement, with the δ-cheapest RF = 8 cells (same
seed and trial counts) as the analytical reference:

| point | p_fail | ε law | ε MC | bad/trials | z |
|---|---|---|---|---|---|
| RF = 9 | 0.321 | 0.099 | 0.118 | 47/400 | +1.2 |
| RF = 9 | 0.393 | 0.399 | 0.424 | 106/250 | +0.8 |
| δ-cheapest RF = 8 | 0.259 | 0.100 | 0.093 | 37/400 | −0.5 |
| δ-cheapest RF = 8 | 0.334 | 0.400 | 0.348 | 87/250 | −1.7 |

The interior pair-retry predicts MC *below* the law; the δ-cheapest
cells sit in that direction (≈ 13 % relief on ε_msg at p_fail ≈ 0.33),
while the RF = 9 cells sit slightly above it — no cell exceeds
|z| = 1.7, so at the operating point the interior effect is below the
sampling noise of this trial budget, a bulk-regime correction at most
~1σ deep. At the δ-tail the multi-defect mass the retry acts on is
negligible either way, so the identity is the right law where the
budgets are read. **Retry sweep**: attempted sends match
×(1−p^{r+1})/(1−p) within ±0.03 % everywhere; delivered/attempted =
1−p_fail; mean depth 3.92 → 4.07 at p_fail = 0.1, r = 0, restored by
retries. Budgets are law-read at the tail under the same convention as
the μ-shift property (deep-tail factor measured ≈ 1.0,
[`tail-correction.md`](../../../../../pubsub-node/docs/experiments/tail-correction.md)).

## 4. Answer

**The backlog's claim fails on the mechanism: bidirectionality is not
doubled loss protection.** Its real content under loss is (a) the
degree term — 14.4 independent final-hop tries per node from a 9-link
budget, the family's best redundancy *per held link*, which is exactly
what the μ_eff identity already prices; and (b) an interior pair-retry
that resolves only at the δ-cheapest cells (z = −1.7, ≈ 13 % of ε_msg
deep in the bulk) and not at the RF = 9 cells, invisible at the
guarantee tail either way. The like-for-like mechanism read — every
model at its δ-cheapest point — puts M4 second-worst on this axis
(per-message budgets: M2 ≈ 32 %, M5 5.1 %, M1 1.7 %, M4 1.03 %,
M3 0.92 %).

**At the operating point RF = 9 no repair is needed at WAN-realistic
loss**: the per-message budget is **7.21 %** at r = 0, so 0.1 % and 1 %
loss pass with an order of magnitude to spare (ε_msg = 9.3×10⁻⁶ at
1 %), 2 % passes at 1.4×10⁻⁵ and 5 % at 4.5×10⁻⁵; the first repair
point is ~7 % loss, where r = 1 lifts the budget to 26.9 %. The
δ-cheapest RF = 8 (per-message budget 1.03 %) clears 1 % only by a hair
(ε_msg = 9.9×10⁻⁵, within 1 % of δ) and needs r = 1 at 2 % — the
selection's +13.6 % bandwidth ([`re_provisioning.md`](re_provisioning.md))
carries a 7× loss budget on this axis as a side effect of the μ-margin.

**Per-epoch reading**: structural floor ε_msg(0) = 6.1×10⁻⁶, headroom
δ − P(bad) = 9.4×10⁻⁵. With R = 10³ messages/epoch, r = 0 holds the
full per-epoch guarantee to p_fail ≈ 0.036 %, r = 1 to 1.9 %
(r = 2: 7.1 %); at R = 10⁶, r = 1 covers 5.9×10⁻⁴ and r = 2 0.71 %.
The floor itself is the μ-part of the law — repairable by provisioning,
not by resending.
