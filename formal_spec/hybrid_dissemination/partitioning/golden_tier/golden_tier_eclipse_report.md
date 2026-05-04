# Golden-tier eclipse analysis for RandCast

## 1. Scope and goal

We analyse a RandCast-style pure-gossip overlay augmented with a small *golden* tier of super-trusted nodes that never fail and may forward at higher fanout than regular peers. The question we answer is:

> Given the network parameters, how many adversarial nodes can the system tolerate before the eclipse probability of a fixed honest target exceeds a chosen tolerance ε?

The unit of analysis is the **eclipse of a single regular honest target** j: the event that no honest node (golden or regular) has j among its forwarding targets in one dissemination round.

## 2. Model and parameters

Three disjoint node classes, with N = G + H + k.

| Symbol | Meaning |
|---|---|
| N | total nodes |
| G | golden nodes — never fail, never corrupted |
| H | regular honest nodes — honest forwards |
| k | adversarial nodes |
| F | fanout of a regular honest node |
| F_g | fanout of a golden node |
| ε | tolerance: required upper bound on P(j eclipsed) |

Six modelling assumptions (see derivation below):

1. Each forwarder picks its fanout targets uniformly without replacement from the N − 1 other nodes.
2. Adversaries contribute nothing toward j's coverage — equivalent to absent.
3. Single-round analysis: multi-round only helps the defender, so single-round is a sound upper bound.
4. The target j is a regular honest node (golden nodes need not be protected — they cannot fail).
5. Forwarder picks are independent across forwarders.
6. We report per-target P(j eclipsed); whole-network bounds follow by union bound.

## 3. Exact per-target eclipse probability

**Single forwarder.** A forwarder of fanout f, choosing f distinct targets uniformly at random from N − 1 others, misses a fixed target j with probability

$$P(\text{miss}) = \frac{\binom{N-2}{f}}{\binom{N-1}{f}} = 1 - \frac{f}{N-1}.$$

**Counting forwarders that can pick j.** j itself does not forward to itself, so:

| Class | Forwarders that can pick j | fanout |
|---|---|---|
| Golden | G | F_g |
| Regular honest | H − 1 | F |
| Adversarial | (k, ignored) | — |

**Independence (assumption 5)** gives the exact formula

$$\boxed{\; P_{\text{exact}}(j \text{ eclipsed}) \;=\; \left(1 - \tfrac{F_g}{N-1}\right)^{\!G} \left(1 - \tfrac{F}{N-1}\right)^{\!H-1}. \;}$$

Sanity checks:

- G = 0, k = 0, H = N reduces to (1 − F/(N−1))^(N−1), the classical "j has no in-edge in a random F-out digraph". ✓
- F_g = F reduces to (1 − F/(N−1))^(G + H − 1), as if the golden tier had no special status. ✓

## 4. Exponential approximation

Using ln(1 − x) = −x − x²/2 − O(x³),

$$\ln P_{\text{exact}} \;=\; -\lambda_{\text{exact}} \;-\; \delta \;-\; O(x^3),$$

where

$$\lambda_{\text{exact}} \;=\; \frac{G F_g + (H-1) F}{N-1}, \qquad \delta \;=\; \frac{1}{2(N-1)^2}\!\left[G F_g^2 + (H-1) F^2\right].$$

Dropping δ (Approx. A) and the O(1/N) corrections in the −1's (Approx. B) yields the working approximation

$$\boxed{\; P(j \text{ eclipsed}) \;\approx\; e^{-\lambda_j}, \qquad \lambda_j \;=\; \frac{G F_g + (N-G-k) F}{N}. \;}$$

The exponent λ_j is the **expected number of honest in-edges arriving at j** in one round.

### 4.1 Validity of the approximation

| Source | Effect | Sign | Magnitude (N=20000, G=50, F_g=200, F=20) |
|---|---|---|---|
| A: (1−x)^n → e^(−nx) | drops x²/2 term, factor e^(−δ) | overestimates P (conservative) | δ ≈ 0.0125 → ~1.3% relative |
| B: −1 in numerator/denominator | shifts λ by O(1/N) | parameter-dependent | < 4 × 10⁻⁵ relative |

The approximation is sharp whenever both fanouts are sparse:

$$\frac{F^2}{N} \ll 1, \qquad \frac{G F_g^2}{N^2} \ll \lambda_j.$$

For our running parameters the combined relative error is ≈ 1%, and Approx. A errs on the conservative side. The exact form (Section 3) should be used for small N (e.g., the N = 6 PRISM models) where the O(1/N) and O(x²) corrections are no longer negligible.

## 5. Adversary tolerance

Requiring P(j eclipsed) ≤ ε is equivalent to λ_j ≥ ln(1/ε). Substituting and solving for k:

$$\boxed{\; k_{\max}(\varepsilon) \;=\; N\!\left(1 - \frac{\ln(1/\varepsilon)}{F}\right) \;+\; G \cdot \frac{F_g - F}{F}. \;}$$

Two terms with clear interpretations:

- **Bulk term** N · (1 − ln(1/ε)/F) — what regular honest nodes alone (G = 0) would yield. Positive iff F > ln(1/ε).
- **Golden bonus** G · (F_g − F)/F — linear in G; one golden node buys (F_g − F)/F regular-honest equivalents.

### 5.1 Whole-network bound

So far ε is per-target. To control instead the probability that **at least one** regular honest node is eclipsed, take a union bound over the H = N − G − k regular honest targets:

$$P\!\left(\exists\, j \text{ eclipsed}\right) \;\le\; H \cdot P(j \text{ eclipsed}) \;\approx\; H \cdot e^{-\lambda_j}.$$

A slightly less conservative form via the Poisson / approximate-independence approximation:

$$P(\text{none eclipsed}) \;\approx\; (1 - p)^{H} \;\approx\; e^{-H p}, \qquad p = e^{-\lambda_j}.$$

When H · p ≪ 1 (the regime of interest), both agree to first order. The two approximations make explicit the only nontrivial assumption: the H eclipse events are taken as **approximately independent**. They are not exactly independent — distinct forwarder picks share information — but the per-pair correlation is O(F/N) and negligible in our regime. Inclusion-exclusion gives the formal error bound

$$P(E_{\text{net}}) \;=\; H p - O\!\big((H p)^2\big),$$

so the union bound overestimates by at most a relative (Hp)/2.

**Corrected adversary tolerance.** Requiring H · e^(−λ_j) ≤ ε_net gives λ_j ≥ ln(H/ε_net). Approximating H ≈ N (slight conservatism, |relative error| ≤ k/N inside a logarithm):

$$\boxed{\; k_{\max}(\varepsilon_{\text{net}}) \;\approx\; N\!\left(1 - \frac{\ln(N/\varepsilon_{\text{net}})}{F}\right) \;+\; G \cdot \frac{F_g - F}{F}. \;}$$

Compared to the per-target formula, only the logarithm changes: ln(1/ε) becomes ln(N/ε_net) — i.e., we pay an extra **ln N** nats of fanout-slack budget (≈ 9.9 nats for N = 20 000).

**Feasibility floor.** The smallest achievable whole-network tolerance is

$$\varepsilon_{\text{net},\min} \;=\; N \cdot \varepsilon_{\min} \;=\; N \cdot e^{-\lambda_j(k=0)}.$$

For the running parameters: ε_net,min ≈ 20 000 · 1.3 × 10⁻⁹ ≈ **2.6 × 10⁻⁵**. Whole-network ε_net = 10⁻² is comfortable; ε_net = 10⁻⁶ is infeasible at F = 20 and requires raising F to at least about 24.

| ε_net | k_max(ε_net) | k_max / N |
|---|---|---|
| 10⁻² | 5 941 | 29.7% |
| 10⁻³ | 3 639 | 18.2% |
| 10⁻⁴ | 1 336 | 6.7% |
| 10⁻⁵ | −966 | infeasible |

## 6. Feasibility

k_max can be negative. The achievable tolerance is bounded below by

$$\varepsilon_{\min} \;=\; \exp\!\left(-\,\frac{G F_g + (N-G) F}{N}\right) \;=\; e^{-\lambda_j(k=0)}.$$

For the running parameters λ_j(k=0) = 20.45, hence ε_min ≈ 1.3 × 10⁻⁹. Any tolerance stricter than ε_min is infeasible at this F regardless of k; the only fix is to raise F (most leveraged) or G·F_g.

## 7. Running example: N = 20 000, G = 50, F_g = 200, F = 20

Golden bonus G(F_g − F)/F = 50 · 180 / 20 = **450 adversary slots**, ε-independent.

| ε (per-target) | ln(1/ε) | bulk term | golden bonus | k_max | k_max / N |
|---|---|---|---|---|---|
| 10⁻² | 4.605 | 15 395 | 450 | 15 845 | 79.2% |
| 10⁻³ | 6.908 | 13 092 | 450 | 13 542 | 67.7% |
| 10⁻⁶ | 13.816 | 6 184 | 450 | 6 634 | 33.2% |
| 10⁻⁸ | 18.421 | 1 579 | 450 | 2 029 | 10.1% |
| 10⁻⁹ | 20.723 | −723 | 450 | −273 | infeasible |

Marginal substitution rate: at first order, only the **product** G · F_g enters λ_j (since F_g ≪ N), so doubling G at fixed F_g and doubling F_g at fixed G have the same effect on tolerance. The fanout F of regular nodes is much more leveraged: each +1 to F adds ~ N(1 − ln(1/ε)/F²) to k_max, dwarfing the golden bonus across the whole feasible ε range.

## 8. Caveats and scope

- **Single-target eclipse only.** A whole-network safety claim follows by union bound P(any j eclipsed) ≤ N · P(j eclipsed), i.e., replace ε by ε/N. The structural form of k_max is unchanged.
- **Static adversary.** Adaptive adversaries (who can choose k after seeing the random graph) require a different analysis; the bound here is for fixed adversary placement.
- **Worst-case adversary contribution = 0.** We do not model adversaries actively dropping or rewriting messages; we only assume their forwards do not help j. This is the conservative direction.
- **No cascading.** In RandCast above the connectivity threshold (F well above ln N), eclipse events are essentially singleton; the conditional severity E[#eclipsed | ≥ 1] ≈ 1. Per-target × N (union bound) is tight up to constants. Near the threshold this no longer holds and a recursive reachability analysis is needed.
- **No churn / failures of regular honest nodes.** The model treats regular honest nodes as alive and honest. Adding per-node failure probability p_fail re-uses the same algebra with H replaced by an effective (1 − p_fail) H.

## 9. Summary

For a RandCast network with a small never-failing golden tier, the per-round per-target eclipse probability admits a clean exponential form

$$P(j \text{ eclipsed}) \approx e^{-\lambda_j}, \qquad \lambda_j = \frac{G F_g + (N - G - k) F}{N},$$

valid up to ~1% relative error whenever fanouts are sparse (F²/N ≪ 1, G F_g² ≪ N² λ_j). The corresponding adversary tolerance decomposes as a bulk term governed by F vs. ln(1/ε), plus a constant golden bonus of G(F_g − F)/F. The smallest achievable ε is e^(−λ_j(k=0)); tolerances below that require lifting F (high leverage) or G · F_g (additive, lower leverage).
