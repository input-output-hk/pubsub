# M4 — transmission unreliability (per-message delivery under send loss)

**Verdict: HYBRID** — the validated μ-shift law re-read per message at
μ_eff = μ + (1−μ)p_fail with exact per-message accounting; loss-injected
MC at elevated cells (the law-vs-MC residual is the interior effect);
degree-mixture and p_fail = 0 anchors. Script (in `../scripts/`):
`sweep_m4_pfail.py`.

## 1. Property

What iid per-transmission loss does to the frozen operating point RF = 8
(N = 20 000, μ = 0.2, δ = 10⁻⁴), and **what M4's bidirectionality
actually buys under it** — the backlog asserted "M4's doubled edge
redundancy should win this axis"; this analysis derives rather than
assumes. Loss model: every honest→honest send is dropped independently
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
does not shrink (lossy nodes still need covering), and the H count stays
H per message ((H−1) in-cuts + 1 publisher out-cut) — unlike the
two-class directed models, M4 keeps its per-epoch multiplicity almost
exactly, so correction (ii) buys it little.

**Where the redundancy actually sits.** At the final hop every model
gets one try per informed honest neighbour — an uninformed node never
fires, so the reverse direction of an undirected link is *not* a second
chance for it. M4's redundancy is a **degree effect**, already inside
the identity: all ~12.8 honest incident links deliver into a node (8 own
picks thinned by μ, plus ~6.4 accepted), vs M3's 9.6 chosen-side pulls —
but fewer than M5's 13.6 and M1/M2's 19.2. The *interior pair-retry*
(B→A succeeding after a failed A→B once B is informed via elsewhere) is
a multi-defect/bulk correction with no single-defect analogue: the
elevated MC cells measure it as the law-vs-MC residual rather than
gating on it.

## 3. Results — law and MC

`sweep_m4_pfail.py` (law, fast by default):

| p_fail | μ_eff | E churn-identity (per-epoch acct.) | E[missed]/msg | ε_msg |
|---|---|---|---|---|
| 0 | 0.200 | 6.8×10⁻⁵ | 1.4×10⁻⁴ | 6.8×10⁻⁵ |
| 0.001 | 0.2008 | 7.0×10⁻⁵ | 1.4×10⁻⁴ | 7.0×10⁻⁵ |
| 0.005 | 0.204 | 8.1×10⁻⁵ | 1.6×10⁻⁴ | 8.2×10⁻⁵ |
| 0.01 | 0.208 | 9.8×10⁻⁵ | 2.0×10⁻⁴ | **9.9×10⁻⁵** |
| 0.02 | 0.216 | 1.4×10⁻⁴ | 2.8×10⁻⁴ | 1.4×10⁻⁴ |
| 0.05 | 0.240 | 3.8×10⁻⁴ | 8.0×10⁻⁴ | 4.0×10⁻⁴ |
| 0.10 | 0.280 | 1.7×10⁻³ | 3.8×10⁻³ | 1.9×10⁻³ |

(E[missed] runs at ≈ 2× ε_msg because a mute event costs all H−1 nodes
at once — rare, but heavy.) **Budgets**: churn-identity 1.07 % (the
published ~1.1 %); exact per-message **1.03 %** at r = 0 (slightly
*tighter* — H unshrunk outweighs the small out-term relief), 10.2 % at
r = 1, 21.8 % at r = 2.

MC (`--mc`, seed 20260813): **anchor** — exact equality with the plain
flood on 40/40 graphs; published costs reproduced (188 776 vs 188 795
msgs, −0.01 %; hops 5.10/4.11 vs 5.1/4.1). **Degree mixture** — the
hypergeometric ⊗ binomial honest-degree pmf validated class by class
(100 graphs, measurable classes |z| ≤ 1.9). **Elevated cells** — the
interior-effect measurement:

| p_fail | ε law | ε MC | bad/trials | z |
|---|---|---|---|---|
| 0.259 | 0.100 | 0.093 | 37/400 | −0.5 |
| 0.334 | 0.400 | 0.348 | 87/250 | −1.7 |

MC sits *below* the law at both cells — the direction the interior
pair-retry predicts, and the only model in the family with a consistent
negative residual (M1/M2/M3/M5 straddle zero). The size: ≈ 13 % relief
on ε_msg at p_fail ≈ 0.33, equivalent to ≈ 0.7 pp of extra p_fail
tolerance *in the bulk regime*. At the δ-tail the multi-defect mass the
retry acts on is negligible, so the identity is the right law there.
**Retry sweep**: attempted sends match ×(1−p^{r+1})/(1−p) within
±0.06 % everywhere; delivered/attempted = 1−p_fail; mean depth 4.11 →
4.24 at p_fail = 0.1, r = 0, restored by retries. Budgets are law-read
at the tail under the same convention as the μ-shift property
(deep-tail factor measured ≈ 1.0,
[`tail-correction.md`](../../../../../pubsub-node/docs/experiments/tail-correction.md)).

## 4. Answer

**The backlog's claim is killed: M4 does not win this axis — it is
second-worst** (per-message budgets: M2 ≈ 32 %, M5 5.1 %, M1 1.7 %,
M4 1.03 %, M3 0.92 %). Bidirectionality's real content under loss is
(a) the degree term — 12.8 independent final-hop tries per node from an
8-link budget, the family's best redundancy *per held link*, which is
exactly what the μ_eff identity already prices; and (b) a measured
interior pair-retry worth ≈ 13 % of ε_msg only deep in the bulk regime
(z = −1.7), invisible at the guarantee tail. What keeps M4's budget
small is the same thing that makes it cheap in state: the total link
budget is 8, so the final-hop exponent (12.8) trails M5 (13.6) and
M1/M2 (19.2).

**At WAN-realistic loss M4 needs no repair**: its 1.03 % budget clears
0.1 % trivially and 1 % by a hair (ε_msg = 9.9×10⁻⁵ — within 1 % of δ,
so operationally r = 1 is the prudent setting at exactly 1 % loss). At
2 % loss one retry restores ε_msg = 6.9×10⁻⁵ for +2 % bandwidth; the
notch alternative RF = 9 ([`re_provisioning.md`](re_provisioning.md),
PR #151) has per-message loss budget 7.2 % for +13.6 % bandwidth and +2
links — retries dominate it on every axis except transport simplicity.

**Per-epoch reading**: structural floor ε_msg(0) = 6.8×10⁻⁵, headroom
3.2×10⁻⁵. With R = 10³ messages/epoch, r = 1 holds the full per-epoch
guarantee to p_fail ≈ 0.35 % (r = 2: 2.3 %); at R = 10⁶, r = 1 covers
1.1×10⁻⁴ and r = 2 0.23 %. The floor itself is the μ-part of the law —
repairable by provisioning, not by resending.
