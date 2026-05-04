# M2 eclipse analysis: RingCast pull mitigation + golden tier

## 1. Scope and goal

This report extends the golden-tier eclipse analysis to the **M2 model**: a RingCast variant in which the deterministic Harary backbone (d-links) is treated as compromised, regular honest nodes obtain forwarders by *pull* (the partitioning mitigation), and golden nodes still *push* to random targets. We derive the per-target eclipse probability, the corresponding adversary tolerance k_max(ε), and compare the result to RandCast/golden at equal fanout.

Companion files in this folder:
- `m2_eclipse_check.py` — numerical verification.
- `golden_tier_eclipse_calculator.html` — interactive calculator (model toggle).
- `golden_tier_eclipse_report.md` — RandCast/golden derivation; we cross-reference it here.

## 2. Model and parameters

Same node taxonomy as before — N = G + H + k — but the dissemination layer differs.

| Symbol | Meaning |
|---|---|
| N | total nodes |
| G | golden nodes (never fail / never corrupted) |
| H | regular honest nodes |
| k | adversarial nodes |
| F_g | golden push fanout (each golden picks F_g random push targets / round) |
| RF | regular pull request count (each regular j requests RF forwarders / round) |
| ε | tolerance: required upper bound on P(j eclipsed) |

### 2.1 Modelling assumptions

Eight assumptions, each agreed in the derivation walk-through:

1. **Uniform without replacement** sampling on both sides: each golden picks an F_g-subset of the N−1 others, each regular j picks an RF-subset of the N−1 others, all uniform on those subsets.
2. **Mutual independence** of all G push samples and all H pull samples; the two layers are independent.
3. **Adversaries are silent** — no useful contribution wherever they appear (push or pull); no active interference modelled.
4. **Honest nodes always serve** — regular honest fulfils any pull request; golden always pushes successfully. No `p_fail`.
5. **No grinding on the pull side** — j's RF forwarders are picked uniformly at random from the population (or from a peer-sampling cache populated by an honest sampling layer), so an adversary cannot bias its chance of being picked.
6. **Target j is regular honest** — golden don't need protection; adversaries don't care.
7. **Single-round analysis** — multi-round eclipse-forever probabilities follow as upper bounds.
8. **Per-target ε** is primary; whole-network bounds are derived by union bound.

### 2.2 Why we drop d-links

In RingCast, each honest node has 2 deterministic ring neighbours (d-links) from H(N, 2). The d-link is an attractive coverage mechanism *unless* the adversary can cheaply place itself adjacent to a target on the ring. Because Vicinity-style ID-based ring placement is grinding-vulnerable (an adversary can repeatedly hash/sign until its ID lands next to a chosen target), under an adaptive adversary the d-link layer cannot be relied on — the worst case is that both of j's ring neighbours are adversary-controlled. We therefore drop the d-link contribution and analyse only the random layer + golden push.

This is conservative: any real protection from d-links (e.g., anti-grinding mechanisms, identity costs that bound k) only improves on the bound we derive.

## 3. Exact per-target eclipse probability

### 3.1 Pull side: one slot

j picks one slot uniformly from the N−1 others. Of those, k are adversarial:

$$P(\text{single pull pick is adversarial}) = \frac{k}{N-1}.$$

### 3.2 Pull side: all RF picks adversarial (hypergeometric)

j picks an RF-subset uniformly from N−1; the layer fails for j iff the chosen subset lies inside the k-element adversary set:

$$P_{\text{pull-fail}}(j) \;=\; \frac{\binom{k}{RF}}{\binom{N-1}{RF}} \;=\; \prod_{i=0}^{RF-1} \frac{k - i}{N - 1 - i}.$$

Sanity checks: k < RF ⇒ 0 (cannot fill RF adversarial slots from fewer than RF Sybils); k = N−1 ⇒ 1; RF = 1 ⇒ k/(N−1).

### 3.3 Push side: no golden chose j

Identical to the RandCast case (Section 3 of the companion report):

$$P_{\text{push-fail}}(j) \;=\; \left(1 - \tfrac{F_g}{N-1}\right)^{\!G}.$$

### 3.4 Combine (independence — Assumption 2)

Eclipse of j is the conjunction of the two layer failures, and they are independent:

$$\boxed{\; P_{\text{exact}}(j \text{ eclipsed}) \;=\; \left(1 - \tfrac{F_g}{N-1}\right)^{\!G} \cdot \frac{\binom{k}{RF}}{\binom{N-1}{RF}}. \;}$$

Sanity checks:
- k = 0 ⇒ pull factor 0 ⇒ P = 0 (no adversaries means at least one pull pick is honest). ✓
- G = 0 ⇒ pure pull C(k, RF)/C(N−1, RF). ✓
- F_g = 0 ⇒ push factor 1 ⇒ pure pull. ✓
- RF = 0 ⇒ pull factor 1 ⇒ pure-golden push (1 − F_g/(N−1))^G. ✓

The factorisation is **multiplicative** in the two layers — fundamentally different from RandCast, where everything sits in a single exponent.

## 4. Approximation

Two approximations stack, each on its own layer.

**Approx A (push side)**: same as in RandCast. (1 − F_g/(N−1))^G ≈ exp(−G F_g / N). Relative error ≈ G F_g² / (2 N²); conservative direction (overestimates eclipse).

**Approx B (pull side)**: hypergeometric → power.

$$\frac{\binom{k}{RF}}{\binom{N-1}{RF}} \;\approx\; \left(\frac{k}{N}\right)^{\!RF}.$$

Relative error: ratio of true to approximation ≈ 1 − RF(RF−1)/(2k) for k ≪ N. Sharp when **RF² ≪ k**; anti-conservative direction (true value is slightly smaller than the approximation, so the approximation overestimates eclipse — also conservative).

### 4.1 Combined approximation

$$\boxed{\; P(j \text{ eclipsed}) \;\approx\; e^{-\lambda_{\text{push}}} \cdot \mu^{RF}, \qquad \lambda_{\text{push}} = \tfrac{G F_g}{N}, \;\; \mu = \tfrac{k}{N}. \;}$$

Validity regime: F_g²·G / (2N²) ≪ 1 **and** RF² ≪ k. Both hold in operating regimes of interest. Numerical verification (Section 7) shows ≤ 1.3% relative error for k ≥ 100 in the running example.

## 5. Adversary tolerance

Requiring P(j eclipsed) ≤ ε:

$$e^{-\lambda_{\text{push}}} \cdot \mu^{RF} \;\le\; \varepsilon \;\;\Leftrightarrow\;\; \mu \;\le\; \big(\varepsilon \cdot e^{\lambda_{\text{push}}}\big)^{1/RF}.$$

Multiplying through by N:

$$\boxed{\; k_{\max}(\varepsilon) \;\approx\; N \cdot \varepsilon^{1/RF} \cdot \exp\!\left(\frac{G F_g}{N \cdot RF}\right). \;}$$

### 5.1 Reading the formula

- **Polynomial in ε^(1/RF)**, not exponential in ln(1/ε). Tightening ε from 10⁻² to 10⁻⁶ multiplies the (k/N) ratio by 10⁻⁴/^RF — a large drop at low RF.
- **Golden tier is a multiplicative bonus** of factor exp(λ_push / RF), independent of ε.
- **No feasibility floor**: ε_min = 0. At k = 0 the pull factor is exactly zero, so any ε > 0 is achievable (in stark contrast to RandCast/golden).
- **RF is the dominant lever**: each unit of RF takes a finer root of ε. Doubling RF takes the square root of the previous tolerance.

### 5.2 Whole-network bound

Apply the union bound over H ≤ N regular honest targets, replacing ε by ε_net / N (with H ≈ N as in the RandCast case):

$$k_{\max}(\varepsilon_{\text{net}}) \;\approx\; N \cdot \left(\frac{\varepsilon_{\text{net}}}{N}\right)^{\!1/RF} \cdot \exp\!\left(\frac{G F_g}{N \cdot RF}\right).$$

The whole-network bound costs a factor N^(1/RF) inside the polynomial — at RF = 2 with N = 20 000 that is ≈ 141, severely shrinking k_max relative to the per-target case.

## 6. Running example

N = 20 000, G = 50, F_g = 200 ⇒ λ_push = 0.5. Golden multiplier exp(λ_push / RF) is 1.649, 1.284, 1.181, 1.133, 1.105 for RF = 1, 2, 3, 4, 5.

### 6.1 Per-target k_max

| RF | ε = 10⁻² | ε = 10⁻³ | ε = 10⁻⁴ | ε = 10⁻⁶ |
|---|---|---|---|---|
| 1 | 330 (1.6%) | 33 (0.16%) | 3 (0.016%) | infeasible |
| 2 | 2 569 (12.8%) | 812 (4.1%) | 257 (1.3%) | 26 (0.13%) |
| 3 | 5 096 (25.5%) | 2 362 (11.8%) | 1 095 (5.5%) | 236 (1.2%) |
| 4 | 7 166 (35.8%) | 4 030 (20.2%) | 2 266 (11.3%) | 716 (3.6%) |
| 5 | 8 770 (43.9%) | 5 535 (27.7%) | 3 492 (17.5%) | 1 390 (6.9%) |

### 6.2 Whole-network k_max (ε_net via union bound)

| RF | ε_net = 10⁻² | ε_net = 10⁻⁴ | ε_net = 10⁻⁶ |
|---|---|---|---|
| 1 | 0.02 (infeasible) | — | — |
| 2 | 18 (0.09%) | 2 (≈0) | infeasible |
| 3 | 236 (1.2%) | 50 (0.25%) | 11 (0.05%) |
| 4 | 716 (3.6%) | 226 (1.1%) | 71 (0.35%) |
| 5 | 1 390 (6.9%) | 553 (2.8%) | 220 (1.1%) |

Practical conclusion: **at the canonical RingCast operating point of RF = 1 or 2**, whole-network adversary tolerance is essentially zero (≤ 20 adversaries in 20 000 nodes). To get meaningful tolerance one must lift RF to 4–5.

## 7. Numerical verification (`m2_eclipse_check.py`)

Four checks, all passing:

1. **P_exact vs. P_approx** at RF = 2: relative error ≤ 1.3% for k ≥ 100; degrades to ~11% at k = 10 (where RF² / k = 0.4 violates Approx B's regime, but the absolute P is so small operationally irrelevant). Direction: conservative.
2. **k_max analytical vs. exact bisection on P_exact**: differ by 0–3 nodes across ε ∈ {10⁻², 10⁻³, 10⁻⁴, 10⁻⁶} at RF = 2. Analytical is conservative (smaller k_max).
3. **M2 vs. RandCast at equal fanout**: M2 wins every comparison cell; RandCast is infeasible in 5/8 cells.
4. **Pointwise inequality**: ratio P_M2 / P_RandCast at equal fanout F = 20 matches the closed form `(μ · e^(1−μ))^F` to within ~5% (small residual is the dropped G/N term).

## 8. Comparison with RandCast at equal fanout

This is the structurally interesting result.

### 8.1 Side-by-side

At equal fanout F = RF (same per-node bandwidth):

$$P_{\text{RandCast}} \;\approx\; e^{-\lambda_{\text{push}}} \cdot e^{-(1 - \mu - G/N) F}, \qquad P_{\text{M2}} \;\approx\; e^{-\lambda_{\text{push}}} \cdot \mu^F.$$

The push factors are identical. The regular layers differ.

### 8.2 The ratio

$$\frac{P_{\text{M2}}}{P_{\text{RandCast}}} \;\approx\; \big(\mu \cdot e^{1-\mu}\big)^{F}.$$

Define g(μ) := μ · e^(1−μ). Then g'(μ) = (1 − μ) e^(1−μ) ≥ 0 on [0, 1] with g'(1) = 0; g(0) = 0, g(1) = 1. So g(μ) ≤ 1 on [0, 1] with equality only at μ = 1. Therefore

$$P_{\text{M2}}(\text{eclipse}) \;\le\; P_{\text{RandCast}}(\text{eclipse}) \quad \text{for every } \mu \in [0, 1],$$

and the gap widens as g(μ)^F → 0 with growing F.

### 8.3 Mechanism

The difference is the **variance of j's honest in-degree**:
- **RandCast (push)**: j's in-degree is binomial-ish with mean ~F·(1−μ); even when the mean is high, the random graph occasionally leaves j with 0 in-edges, contributing a Poisson-tail e^(−F·(1−μ)) to the eclipse probability.
- **M2 (pull)**: j's in-degree is *deterministically* RF (j actively picks). Eclipse requires every one of those RF picks to land on the adversary subset — a polynomial μ^RF.

Pull eliminates the random-graph "unlucky j" failure mode entirely.

### 8.4 Numerical comparison

| F = RF | ε | k_max RandCast | k_max M2 |
|---|---|---|---|
| 2 | 10⁻² | infeasible | 2 569 |
| 2 | 10⁻⁶ | infeasible | 26 |
| 5 | 10⁻² | 3 530 | 8 800 |
| 5 | 10⁻⁶ | infeasible | 1 395 |
| 10 | 10⁻² | 11 740 | 13 268 |
| 10 | 10⁻⁶ | infeasible | 5 282 |
| 20 | 10⁻² | 15 845 | 16 289 |
| 20 | 10⁻⁶ | 6 634 | 10 282 |

The sharpest differences appear where RandCast is below its feasibility floor (ε < e^(−λ_j(0)) ≈ e^(−F)) — there M2 is still feasible by orders of magnitude.

### 8.5 Caveats

What this comparison does **not** say:

1. **Pull requires forwarders to hold the message.** RandCast push naturally propagates the rumour as a side effect; pull only delivers if the requested forwarder has already received it. The single-round analysis assumes the rumour has reached the potential helpers.
2. **Pull requires an honest peer-sampling layer.** Assumption 5 (no grinding on pull) depends on the sampling cache being populated honestly. Cache poisoning attacks would degrade pull toward d-link-style vulnerability.
3. **Multi-round dynamics differ.** Both models can be re-run across rounds; the round-to-round independence and refresh structure is different and not analysed here.
4. **The constant-factor improvement plateaus near μ = 1.** As k → N, both models converge to certain eclipse; M2's advantage shrinks at very high adversary fractions.

## 9. Caveats and scope (overall)

- **Single-round, static-adversary, honest-cache assumptions** — see §2.1.
- **No regular-node failure** — adding `p_fail` substitutes k → k + p_fail · H in the pull factor.
- **Worst-case adversary contribution = 0** — see Assumption 3.
- **Whole-network bound** uses a union bound over H ≈ N targets; relative tightness is O(ε_net / 2).

## 10. Summary

In the M2 setting (d-links assumed compromised, regulars pull RF forwarders, golden push F_g random targets), the per-target eclipse probability factors multiplicatively into a push factor and a pull factor:

$$P(j \text{ eclipsed}) \;\approx\; e^{-G F_g / N} \cdot (k/N)^{RF}.$$

The corresponding adversary tolerance is

$$k_{\max}(\varepsilon) \;\approx\; N \cdot \varepsilon^{1/RF} \cdot \exp\!\left(\frac{G F_g}{N \cdot RF}\right).$$

There is no feasibility floor, but the polynomial dependence on ε^(1/RF) makes low-RF operating points fragile at security-grade ε. RF is the dominant design lever; the golden tier provides a multiplicative bonus of exp(λ_push / RF).

**At equal fanout, M2 is strictly better than RandCast** for every adversary fraction μ ∈ [0, 1), with ratio (μ · e^(1−μ))^F. The structural reason is that pull deterministically gives j exactly RF in-edges, whereas push leaves the in-degree random — so RandCast has a Poisson-tail failure mode that M2 does not.

In design terms, given RF as the budget, **pull is uniformly preferable to push for eclipse resistance** — provided the messages are reaching potential helpers (the assumption that pull does not solve dissemination by itself), and the peer-sampling layer that supplies pull candidates is honest.
