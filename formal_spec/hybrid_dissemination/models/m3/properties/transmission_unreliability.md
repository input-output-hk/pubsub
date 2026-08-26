# M3 — transmission unreliability (per-message delivery under send loss)

**Verdict: HYBRID** — the validated μ-shift law re-read per message at
μ_eff = μ + (1−μ)p_fail with exact per-message accounting; loss-injected
MC at elevated cells; degree-mixture and p_fail = 0 anchors. Script (in
`../scripts/`): `sweep_m3_pfail.py`.

## 1. Property

What iid per-transmission loss does to the frozen operating point
(RF, s) = (13, 7) (N = 20 000, μ = 0.2, δ = 10⁻⁴). Loss model: every
honest→honest send is dropped independently with probability p_fail at
send time; r per-link retries make the per-send failure p_fail^(r+1)
(each attempt a fresh Bernoulli); sends to adversaries never matter for
coverage. Correlated or bursty loss is out of scope — a failing peer's
whole link set going dark reads as churn, the adjacent, already-analysed
property ([`mu_shift_robustness.md`](mu_shift_robustness.md)).

**The semantics shift.** The per-epoch criterion ("every message of every
honest publisher covers") is a property of standing structure alone;
per-message loss randomness breaks that dichotomy — with unbounded
messages per epoch, P(some message somewhere loses a send it needed) → 1
without repair. The headline quantities are therefore **per-message**:

- **E[missed]** — expected honest nodes that miss one message;
- **ε_msg** — P(the message misses ≥ 1 honest node), read against the
  same δ = 10⁻⁴.

Per-epoch and per-message numbers never share a table below without
labels. Prior in-repo evidence that per-forwarding loss is the sharper
failure mode: the PRISM asymmetry study at n = 6
([`asym_report.md`](../../../prism/asym_report.md)) found p_fail more
damaging than pre-dissemination node loss.

## 2. Guiding formula

Per message, an in-edge fails iff the pick is adversarial or the send is
lost — per-edge failure **μ_eff = μ + (1−μ)p_fail**, i.e. the μ-shift
curve read at the churn formula with p = p_fail. Two accounting
corrections separate loss from churn:

1. **H does not shrink** — a node behind lossy links still needs the
   message; under churn a down node leaves the coverage requirement.
2. **The muted-publisher out-term loses its factor H** — a message has
   one publisher; under churn every honest node is a potential victim.

The exact single-defect law (P = p_fail^(r+1)):

$$q_{\text{in}} = \mathbb{E}[P^{D}],\qquad
q_{\text{out}} = \Bigl(1-(1-P)\tfrac{RF}{N-1}\Bigr)^{H-1}\,\mathbb{E}[P^{D'}],$$

$$\varepsilon_{\text{msg}} = 1-(1-q_{\text{out}})(1-q_{\text{in}})^{H-1},\qquad
E[\text{missed}] = (H-1)(q_{\text{in}}+q_{\text{out}}-q_{\text{in}}q_{\text{out}}),$$

where D (D′) is the honest count among the RF pulls (s−1 initiation
targets), an exact hypergeometric expectation — the binomial approximation
of E[P^D] is μ_eff^RF, the identity's form. At p_fail = 0 both terms
reduce to the published coverage law
([`full_coverage.md`](full_coverage.md)). Where the identity is *not*
exact: churn kills a node's link set together while loss is iid per edge
(they agree on the single-defect term, diverge on multi-defect ones), and
an in-neighbour that itself missed the message never fires (an interior
correction) — the elevated MC cells bound both. Initiation links attack
only the out-term, exactly as at p_fail = 0.

Loss sensitivity is where M3 pays for pull reception: a chosen-side link
fails per message with a μ floor, log-slope d ln q_in/dp_fail =
RF(1−μ)/μ_eff ≈ 52 at the operating point, vs m′(1−μ) ≈ 0.8 m′ for
accepted-side links — the family's steepest, as with μ-shift.

## 3. Results — law and MC

`sweep_m3_pfail.py` (law, fast by default):

| p_fail | μ_eff | E churn-identity (per-epoch acct.) | E[missed]/msg | ε_msg |
|---|---|---|---|---|
| 0 | 0.200 | 4.4×10⁻⁵ | 4.4×10⁻⁵ | 1.3×10⁻⁵ |
| 0.001 | 0.2008 | 4.6×10⁻⁵ | 4.6×10⁻⁵ | 1.4×10⁻⁵ |
| 0.005 | 0.204 | 5.3×10⁻⁵ | 5.3×10⁻⁵ | 1.7×10⁻⁵ |
| 0.01 | 0.208 | 6.4×10⁻⁵ | 6.5×10⁻⁵ | **2.2×10⁻⁵** |
| 0.02 | 0.216 | 9.4×10⁻⁵ | 9.6×10⁻⁵ | 3.5×10⁻⁵ |
| 0.05 | 0.240 | 2.8×10⁻⁴ | 2.9×10⁻⁴ | 1.4×10⁻⁴ |
| 0.10 | 0.280 | 1.5×10⁻³ | 1.7×10⁻³ | 1.0×10⁻³ |

**Budgets** (largest p_fail with ε_msg ≤ δ): churn-identity reading
2.17 % (the published ~2.2 % of
[`mu_shift_robustness.md`](mu_shift_robustness.md) §4); exact per-message
**4.25 %** at r = 0 (the out-term's lost H-factor buys the difference —
71 % of the defect budget sits there), 20.6 % at r = 1, 34.9 % at r = 2.

MC (`--mc`, seed 20260813): **anchor** — the loss-injected flood equals
the plain flood exactly on 40/40 graphs at p_fail = 0 and reproduces the
published costs (166 365 vs 166 400 msgs, −0.02 %; hops 5.47/4.22 vs
5.5/4.2). **Degree mixture** — the law is Σ_d H·pmf(d)·P^d, so MC
validates the honest-degree pmfs class by class (100 graphs): all
measurable classes of the pull hypergeometric, requester binomial and
initiation hypergeometric within |z| ≤ 3 (worst +2.42); unmeasurable
classes are exact combinatorics. **Elevated cells** (exact law vs full
loss-injected floods):

| p_fail | ε law | ε MC | bad/trials | z |
|---|---|---|---|---|
| 0.249 | 0.099 | 0.105 | 42/400 | +0.4 |
| 0.314 | 0.402 | 0.432 | 108/250 | +1.0 |

**Retry sweep** (20 graphs/cell): attempted sends match the closed form
×(1−p^{r+1})/(1−p) within ±0.08 % at every (p_fail, r) cell;
delivered/attempted = 1−p_fail throughout; mean depth grows 4.20 → 4.40
at p_fail = 0.1, r = 0 (longer detours) and is restored by r ≥ 1 (the
retry price is timeouts, below, not hops). The budgets are law-read at
the δ tail; the deep-tail factor is measured at ≈ 1.0
([`tail-correction.md`](../../../../../pubsub-node/docs/experiments/tail-correction.md),
0.994 ± 0.021) — carrying the older conservative ×1.11 instead would read
the r = 0 budget at ≈ 4.0 % and ε_msg(1 %) at 2.4×10⁻⁵; the conclusion
at 1 % loss does not flip either way.

## 4. Answer

**M3's per-message loss tolerance at the operating point is ≈ 4.3 %,
clearing WAN-realistic loss with no repair**: 1 % loss reads
ε_msg = 2.2×10⁻⁵ (~4.6× under δ), 2 % reads 3.5×10⁻⁵. The headroom is
what the margin selection buys on this axis: the δ-cheapest split
(12, 8) tolerates ≈ 0.9 %, so 1 % loss exceeds *its* budget — per-message
loss is a second reading of the same μ_eff brittleness the disturbance
margin prices, and the re-split
([`re_provisioning.md`](re_provisioning.md) notch A) multiplies the loss
budget ~4.6× along with the churn budget. The corrected accounting
roughly doubles the churn-identity's 2.2 % prediction because 71 % of
M3's defect budget sits in the out-term, which per message is a single
publisher's event.

**The price of repair beyond the budget**: at 5 % loss one retry
(r = 1) restores ε_msg = 1.5×10⁻⁵ ≤ δ for **+5.0 % bandwidth** and
+0.048 timeouts per delivered send (≈ 5 % of hops pay one
RTT·timeout-factor); at 10 %, r = 1 reads 2.2×10⁻⁵ for +10 %. Retries
repair only the loss part — the p_fail = 0 floor is the μ-part of the
law (below) — and M3 keeps the family's bandwidth lead at every cell
with loss ≤ 10 % and r ≤ 2.

**Per-epoch reading.** ε_msg has a structural floor at p_fail = 0
(1.3×10⁻⁵, graph randomness shared by every message of the epoch), so
the naive union bound ε_msg ≤ δ/R is unreachable for any R ≥ 8. The
correct split is graph-once + loss-fresh: P(bad epoch) ≤ P(structural
defect) + R·[ε_msg(p) − ε_msg(0)], with δ's headroom over the published
P(bad) (5.6×10⁻⁵) funding the loss term. At R = 10³ messages/epoch,
r = 0 covers only p_fail ≈ 8×10⁻⁵; r = 1 holds the full per-epoch
guarantee up to p_fail ≈ 0.91 % (r = 2: 4.4 %) — sustained 1 % loss at
R = 10³ needs r = 2. At R = 10⁶, r = 1 covers 2.9×10⁻⁴ and r = 2
0.44 %. Retries cannot buy back the structural floor: it is the μ-part
of the law, and a lost epoch draw is repaired by re-provisioning, not
by resending.
