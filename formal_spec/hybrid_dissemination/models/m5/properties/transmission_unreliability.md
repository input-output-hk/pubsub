# M5 — transmission unreliability (per-message delivery under send loss)

**Verdict: HYBRID** — the validated μ-shift law re-read per message at
μ_eff = μ + (1−μ)p_fail with exact per-message accounting; loss-injected
MC at elevated cells; degree-mixture and p_fail = 0 anchors. Script (in
`../scripts/`): `sweep_m5_pfail.py`.

## 1. Property

What iid per-transmission loss does to the frozen operating point
(k_in, k_out) = (9, 8) (N = 20 000, μ = 0.2, δ = 10⁻⁴). Loss model:
every honest→honest send is dropped independently with probability
p_fail at send time; r per-link retries make the per-send failure
p_fail^(r+1); sends to adversaries never matter for coverage.
Correlated/bursty loss is out of scope (it reads as churn,
[`mu_shift_robustness.md`](mu_shift_robustness.md)).

**The semantics shift**: per-message loss randomness breaks the
per-epoch standing-structure dichotomy, so the headline quantities are
per-message — E[missed] and ε_msg = P(the message misses ≥ 1 honest
node) against the same δ; per-epoch and per-message numbers never share
a table without labels (motivating precedent: the PRISM study,
[`asym_report.md`](../../../prism/asym_report.md)).

## 2. Guiding formula

Per message an in-edge fails iff the pick is adversarial or the send is
lost — per-edge failure μ_eff = μ + (1−μ)p_fail, the μ-shift curve read
at the churn formula. Against churn, per message (i) H does not shrink
and (ii) the muted-publisher out-term loses its factor H. Exact
single-defect law (P = p_fail^(r+1)), both defect classes doubly
protected as at p_fail = 0:

$$q_{\text{in}} = \mathbb{E}[P^{D_{\text{in}}}]\,
\Bigl(1-(1-P)\tfrac{k_{\text{out}}}{N-1}\Bigr)^{H-1},\qquad
q_{\text{out}} = \mathbb{E}[P^{D_{\text{out}}}]\,
\Bigl(1-(1-P)\tfrac{k_{\text{in}}}{N-1}\Bigr)^{H-1},$$

$$\varepsilon_{\text{msg}} = 1-(1-q_{\text{out}})(1-q_{\text{in}})^{H-1},$$

with D_in (D_out) the honest count among the node's k_in (k_out) own
picks (exact hypergeometric). At p_fail = 0 these are the published
coverage-law terms ([`full_coverage.md`](full_coverage.md)). M5's
per-epoch budget at (9, 8) is out-dominated (E_out : E_in ≈ 69 : 31), so
correction (ii) more than doubles its loss tolerance relative to the
churn identity. Every node has ≈ 13.6 honest final-hop tries (7.2 chosen
+ 6.4 accepted); the chosen part carries the μ-floor loss sensitivity
(≈ 4 k_in log-slope), the accepted part the flatter ≈ 0.8 k_out.

## 3. Results — law and MC

`sweep_m5_pfail.py` (law, fast by default):

| p_fail | μ_eff | E churn-identity (per-epoch acct.) | E[missed]/msg | ε_msg |
|---|---|---|---|---|
| 0 | 0.200 | 4.4×10⁻⁵ | 4.4×10⁻⁵ | 1.3×10⁻⁵ |
| 0.001 | 0.2008 | 4.6×10⁻⁵ | 4.6×10⁻⁵ | 1.4×10⁻⁵ |
| 0.005 | 0.204 | 5.3×10⁻⁵ | 5.4×10⁻⁵ | 1.7×10⁻⁵ |
| 0.01 | 0.208 | 6.5×10⁻⁵ | 6.5×10⁻⁵ | 2.1×10⁻⁵ |
| 0.02 | 0.216 | 9.4×10⁻⁵ | 9.6×10⁻⁵ | 3.1×10⁻⁵ |
| 0.05 | 0.240 | 2.7×10⁻⁴ | 2.8×10⁻⁴ | 9.6×10⁻⁵ |
| 0.10 | 0.280 | 1.3×10⁻³ | 1.5×10⁻³ | 5.3×10⁻⁴ |

**Budgets**: churn-identity 2.18 % (the published ~2.2 %); exact
per-message **5.11 %** at r = 0 (the out-term's lost H-factor), 22.6 %
at r = 1, 37.1 % at r = 2.

MC (`--mc`, seed 20260813): **anchor** — exact equality with the plain
flood on 40/40 graphs; published costs reproduced (217 543 vs 217 562
msgs, −0.01 %; hops 5.03/3.98 vs 5.0/3.9). **Degree mixture** — the
hypergeometric ⊗ binomial in- and out-degree pmfs validated class by
class (100 graphs, measurable classes |z| ≤ 1.8). **Elevated cells**:

| p_fail | ε law | ε MC | bad/trials | z |
|---|---|---|---|---|
| 0.297 | 0.099 | 0.093 | 37/400 | −0.5 |
| 0.370 | 0.403 | 0.420 | 105/250 | +0.6 |

**Retry sweep**: attempted sends match ×(1−p^{r+1})/(1−p) within
±0.06 % everywhere; delivered/attempted = 1−p_fail; mean depth 3.96 →
4.12 at p_fail = 0.1, r = 0, restored by retries. Budgets are law-read
at the tail under the μ-shift convention (deep-tail factor measured
≈ 1.0,
[`tail-correction.md`](../../../../../pubsub-node/docs/experiments/tail-correction.md)).

## 4. Answer

**M5 tolerates ≈ 5.1 % per-message loss at the published point —
second-best in the family** (after M2's ≈ 32 %) — clearing the whole
WAN-realistic range and the 2 % cell with no repair; at 10 % loss one
retry restores ε_msg = 2.1×10⁻⁵ for +10 % bandwidth. As with μ-shift,
part of this headroom is margin, not structure: the cheapest integer
point (9, 8) lands 2.3× under δ at p_fail = 0. The notch alternative
(9, 9) (churn budget ~5.4 %, [`re_provisioning.md`](re_provisioning.md),
PR #151) costs +5.9 % bandwidth and +2 links — retries dominate it
except on transport simplicity. M5 remains best on no axis: its loss
budget trails M2's by 6× while costing 42 % more bandwidth than M3.

**Per-epoch reading**: structural floor ε_msg(0) = 1.35×10⁻⁵ — the
family's second-lowest — with headroom 5.6×10⁻⁵, the family's largest.
With R = 10³ messages/epoch, r = 1 holds the full per-epoch guarantee
to p_fail ≈ 0.99 % (r = 2: 4.6 %); at R = 10⁶, r = 1 covers 0.031 % and
r = 2 0.46 %. The floor is the μ-part of the law — provisioning
territory, not retransmission's.
