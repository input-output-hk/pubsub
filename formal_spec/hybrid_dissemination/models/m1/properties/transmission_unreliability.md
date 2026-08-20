# M1 — transmission unreliability (per-message delivery under send loss)

**Verdict: HYBRID** — the validated μ-shift law re-read per message at
μ_eff = μ + (1−μ)p_fail with exact per-message accounting; loss-injected
MC at elevated cells; degree-mixture and p_fail = 0 anchors. Script (in
`../scripts/`): `sweep_m1_pfail.py`.

## 1. Property

What iid per-transmission loss does to the frozen operating point F = 25
(N = 20 000, μ = 0.2, δ = 10⁻⁴). Loss model: every honest→honest send is
dropped independently with probability p_fail at send time; r per-link
retries make the per-send failure p_fail^(r+1); sends to adversaries
never matter for coverage. Correlated/bursty loss is out of scope (it
reads as churn, [`mu_shift_robustness.md`](mu_shift_robustness.md)).

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
single-defect law (P = p_fail^(r+1)):

$$q_{\text{in}} = \Bigl(1-(1-P)\tfrac{F}{N-1}\Bigr)^{H-1},\qquad
q_{\text{out}} = \mathbb{E}[P^{D}],$$

$$\varepsilon_{\text{msg}} = 1-(1-q_{\text{out}})(1-q_{\text{in}})^{H-1},$$

with D the honest count among the publisher's own F pushes (exact
hypergeometric). Reception in M1 rides entirely on *accepted* links
(other nodes' pushes, honest by construction), which carry no μ floor
per link: log-sensitivity d ln q_in/dp_fail = F(1−μ)(1−p)⁻¹·… ≈ 0.8 F ≈
20 — modest per link next to the ≈ 4m slope of chosen-side reception in
the pull models. M1's in-term dominates at the operating point, so
correction (ii) buys almost nothing and (i) slightly *tightens* the
budget: the loss identity is essentially the churn curve, H-corrected.
At p_fail = 0 the law reduces to the published coverage terms
([`full_coverage.md`](full_coverage.md)); churn/loss divergence on
multi-defect terms and the uninformed-sender interior correction are
bounded by the elevated MC cells.

## 3. Results — law and MC

`sweep_m1_pfail.py` (law, fast by default):

| p_fail | μ_eff | E churn-identity (per-epoch acct.) | E[missed]/msg | ε_msg |
|---|---|---|---|---|
| 0 | 0.200 | 3.3×10⁻⁵ | 3.3×10⁻⁵ | 3.3×10⁻⁵ |
| 0.001 | 0.2008 | 3.3×10⁻⁵ | 3.3×10⁻⁵ | 3.3×10⁻⁵ |
| 0.005 | 0.204 | 3.6×10⁻⁵ | 3.6×10⁻⁵ | 3.6×10⁻⁵ |
| 0.01 | 0.208 | 3.9×10⁻⁵ | 4.0×10⁻⁵ | 4.0×10⁻⁵ |
| 0.02 | 0.216 | 4.8×10⁻⁵ | 4.9×10⁻⁵ | 4.9×10⁻⁵ |
| 0.05 | 0.240 | 8.4×10⁻⁵ | 8.9×10⁻⁵ | 8.9×10⁻⁵ |
| 0.10 | 0.280 | 2.2×10⁻⁴ | 2.4×10⁻⁴ | 2.4×10⁻⁴ |

**Budgets**: churn-identity 5.91 % (the published ~5.9 %); exact
per-message **5.60 %** at r = 0 — the family's only *downward*
correction of any size, since M1's defect budget is almost purely
in-term — then 23.7 % at r = 1 and 38.3 % at r = 2. The δ-cheapest
F = 24 reads 1.67 % at r = 0 on the same law.

MC (`--mc`, seed 20260813): **anchor** — exact equality with the plain
flood on 40/40 graphs; published costs reproduced (319 943 vs 319 974
msgs, −0.01 %; hops 5.00/3.57 vs 4.92/3.56). **Degree mixture** — pusher
binomial and push-target hypergeometric pmfs validated class by class
(100 graphs, measurable classes |z| ≤ 2.2). **Elevated cells**:

| p_fail | ε law | ε MC | bad/trials | z |
|---|---|---|---|---|
| 0.404 | 0.101 | 0.100 | 40/400 | −0.0 |
| 0.483 | 0.403 | 0.452 | 113/250 | +1.6 |

**Retry sweep**: attempted sends match ×(1−p^{r+1})/(1−p) within
±0.04 % everywhere; delivered/attempted = 1−p_fail; mean depth 3.57 →
3.66 at p_fail = 0.1, r = 0, restored by retries. Budgets are law-read
at the tail under the μ-shift convention (deep-tail factor measured
≈ 1.0,
[`tail-correction.md`](../../../../../pubsub-node/docs/experiments/tail-correction.md)).

## 4. Answer

**M1 tolerates ≈ 5.6 % per-message loss at the published point** and
needs nothing at WAN-realistic 0.1–1 %, retry-free across the whole
grid to 5 % loss. Its cushion is what its bandwidth already bought:
20.0 honest pushers per node, each an independent final-hop try, plus
the margin rule's headroom under δ — the δ-cheapest F = 24 reads
1.67 %, so the +4.2 % bandwidth of F = 25 multiplies the r = 0 loss
budget ×3.4. At 10 % loss one retry restores ε_msg ≤ δ (+10 %
bandwidth, +0.09 timeouts per delivered send); r = 1 covers loss to
23.7 %.

**Per-epoch reading**: structural floor ε_msg(0) = 3.26×10⁻⁵ with
headroom 6.7×10⁻⁵. With R = 10³ messages/epoch, r = 1 holds the full
per-epoch guarantee to p_fail ≈ 1.0 % (r = 2: 4.7 %); at R = 10⁶,
r = 1 covers 3.2×10⁻⁴ and r = 2 0.47 %. The floor is the μ-part of the
law — provisioning territory, not retransmission's.
