# M2 — transmission unreliability (per-message delivery under send loss)

**Verdict: HYBRID** — the validated μ-shift law re-read per message at
μ_eff = μ + (1−μ)p_fail with exact per-message accounting; loss-injected
MC at elevated cells; degree-mixture and p_fail = 0 anchors. Script (in
`../scripts/`): `sweep_m2_pfail.py`.

## 1. Property

What iid per-transmission loss does to the frozen operating point
RF = 24 (N = 20 000, μ = 0.2, δ = 10⁻⁴). Loss model: every
honest→honest send is dropped independently with probability p_fail at
send time; r per-link retries make the per-send failure p_fail^(r+1);
sends to adversaries never matter for coverage. Correlated/bursty loss
is out of scope (it reads as churn,
[`mu_shift_robustness.md`](mu_shift_robustness.md)).

**The semantics shift**: per-message loss randomness breaks the
per-epoch standing-structure dichotomy, so the headline quantities are
per-message — E[missed] and ε_msg = P(the message misses ≥ 1 honest
node) against the same δ; per-epoch and per-message numbers never share
a table without labels (motivating precedent: the PRISM study,
[`asym_report.md`](../../../prism/asym_report.md)). For M2 this shift is
not a relabelling — it moves the budget by more than an order of
magnitude, because M2's per-epoch weakness is an *out*-class event.

## 2. Guiding formula

Per message an in-edge fails iff the pick is adversarial or the send is
lost — per-edge failure μ_eff = μ + (1−μ)p_fail, the μ-shift curve read
at the churn formula. Against churn, per message (i) H does not shrink
and (ii) **the muted-publisher out-term loses its factor H** — a message
has one publisher, while under churn every honest node is a potential
requester-less victim. Exact single-defect law (P = p_fail^(r+1)):

$$q_{\text{in}} = \mathbb{E}[P^{D}],\qquad
q_{\text{out}} = \Bigl(1-(1-P)\tfrac{RF}{N-1}\Bigr)^{H-1},$$

$$\varepsilon_{\text{msg}} = 1-(1-q_{\text{out}})(1-q_{\text{in}})^{H-1},$$

with D the honest count among the node's RF pulls (exact
hypergeometric; binomial approximation μ_eff^RF). At p_fail = 0 these
are the isolated-vertex terms of the published law
([`full_coverage.md`](full_coverage.md); its branching refinements
(1−ρ, u) differ by < 0.1 % at RF = 24 and serve as the per-epoch churn
reference). Correction (ii) is decisive here: per epoch M2's E is
dominated by H·q_out (any of H publishers may be requester-less); per
message only *this* publisher's q_out ≈ e^{−RF(1−μ)(1−p)} ≈ 4.6×10⁻⁹
counts, and the budget is set instead by the in-term (H−1)·μ_eff^24 —
the family's deepest in-exponent (19.2 honest pulls per node).

## 3. Results — law and MC

`sweep_m2_pfail.py` (law, fast by default):

| p_fail | μ_eff | E churn-identity (per-epoch acct.) | E[missed]/msg | ε_msg |
|---|---|---|---|---|
| 0 | 0.200 | 7.3×10⁻⁵ | 7.3×10⁻⁵ | 4.5×10⁻⁹ |
| 0.001 | 0.2008 | 7.5×10⁻⁵ | 7.4×10⁻⁵ | 4.6×10⁻⁹ |
| 0.005 | 0.204 | 8.0×10⁻⁵ | 8.0×10⁻⁵ | 5.0×10⁻⁹ |
| 0.01 | 0.208 | 8.8×10⁻⁵ | 8.8×10⁻⁵ | 5.5×10⁻⁹ |
| 0.02 | 0.216 | 1.1×10⁻⁴ | 1.1×10⁻⁴ | 6.7×10⁻⁹ |
| 0.05 | 0.240 | 1.8×10⁻⁴ | 1.9×10⁻⁴ | 1.2×10⁻⁸ |
| 0.10 | 0.280 | 4.5×10⁻⁴ | 5.0×10⁻⁴ | 3.2×10⁻⁸ |

(E[missed] tracks the churn curve because a mute event — rare per
message — costs all H−1 nodes at once; ε_msg is what the per-message
guarantee reads.) **Budgets**: churn-identity 1.70 % (the published
~1.7 %); exact per-message **31.9 %** at r = 0, 56 % at r = 1, 68 % at
r = 2 — the family's largest per-message tolerance by 6×, and the
family's largest correction to the identity.

MC (`--mc`, seed 20260813): **anchor** — exact equality with the plain
flood on 40/40 graphs; published costs reproduced (307 131 vs 307 153
msgs, −0.01 %; hops 4.83/3.60 vs 4.8/3.6). **Degree mixture** — pull
hypergeometric and requester binomial pmfs validated class by class
(100 graphs, measurable classes |z| ≤ 1.8). **Elevated cells** (note
the p_fail values these sit at):

| p_fail | ε law | ε MC | bad/trials | z |
|---|---|---|---|---|
| 0.510 | 0.099 | 0.080 | 32/400 | −1.4 |
| 0.562 | 0.399 | 0.424 | 106/250 | +0.8 |

**Retry sweep**: attempted sends match ×(1−p^{r+1})/(1−p) within
±0.04 % everywhere; delivered/attempted = 1−p_fail; mean depth 3.60 →
3.76 at p_fail = 0.1, r = 0. Budgets are law-read at the tail under the
μ-shift convention (deep-tail factor measured ≈ 1.0,
[`tail-correction.md`](../../../../../pubsub-node/docs/experiments/tail-correction.md)).

## 4. Answer

**Per-message, M2 is the family's loss-tolerance winner by an order of
magnitude: budget ≈ 32 % at r = 0, no repair needed anywhere on the
0.1–10 % grid** (ε_msg ≤ 3.2×10⁻⁸ throughout). The reversal against its
mid-pack ~1.7 % churn budget is pure semantics: churn's binding defect —
some publisher of H drawing zero honest requesters — is per message a
single node's ~4.6×10⁻⁹ event, and what remains is reception through 24
own pulls, the deepest in-side redundancy in the family (a cushion
bought by M2's 2× bandwidth). The identity's per-epoch accounting
mispredicts M2's loss tolerance by 19×; the corrected law is
MC-confirmed at both elevated cells.

**Per-epoch reading** — where the H-factors return: structural floor
ε_msg(0) = 4.5×10⁻⁹ (four orders below the other models), P(bad graph)
= 7.3×10⁻⁵, headroom 2.7×10⁻⁵. **With R = 10³ messages/epoch M2 holds
the full per-epoch guarantee to p_fail ≈ 10 % with no retries at all**
(r = 1: 32 %); at R = 10⁶, r = 0 covers 0.031 % and r = 1 1.8 %. No
other model reaches a tenth of this: M2 is the only design whose
standing structure makes the *epoch-level* guarantee survivable under
realistic loss without transport repair, at the usual price — 2× the
bandwidth and 3× the standing links of the frontier models, and a muted
side that this property's twin, the eclipse analysis
([`adaptive_eclipse_cost.md`](adaptive_eclipse_cost.md)), already
flagged.
