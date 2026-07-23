#!/usr/bin/env python3
"""
Numerical verification of the M2 (RingCast pull + golden push) eclipse formula.

Model: d-links ignored (assumed adversary-controlled via grinding); regular
honest nodes pull RF forwarders uniformly without replacement; golden nodes
push to F_g random targets (same as in RandCast/golden tier).

Per-target eclipse probability under M2 (exact):

    P_exact_M2(j eclipsed) = (1 - F_g/(N-1))^G  *  C(k, RF) / C(N-1, RF)

Power-law approximation:

    P_approx_M2(j eclipsed) = exp(-G*F_g/N) * (k/N)^RF

Adversary tolerance:

    k_max_M2(eps) ≈ N * eps^(1/RF) * exp(G*F_g / (N*RF))

Pointwise inequality (proved analytically): at equal fanout F = RF,

    P_M2 / P_RandCast = (mu * exp(1-mu))^F  with  mu = k/N,

and (mu * exp(1-mu)) <= 1 on [0, 1] with equality only at mu = 1, so
P_M2 <= P_RandCast everywhere.

This script:
  (1) Compares P_exact_M2 vs P_approx_M2 across k for the running example.
  (2) Compares analytical k_max_M2 vs bisection on P_exact_M2.
  (3) Compares M2 vs RandCast k_max at equal fanout (F = RF).
  (4) Verifies the P_M2 <= P_RandCast pointwise inequality numerically.

Coverage-distribution checks (single-round Monte-Carlo, properties #4, #10 of
../../models/closed_form_analysis.md):
  (5),(6) Closed-form mean/variance of the eclipsed-node count M vs Monte-Carlo
      at two under-dispersion levels (~7%, ~11%).  The negative push-side
      covariance makes M UNDER-dispersed vs the independent union-bound estimate
      H p (1-p); MC variance tracks the CORRECTED closed form -- the new claim.
      (Under-dispersion ~ (H/N) p lambda_push; negligible at operating-grade p.)
  (7) Heterogeneous per-node RF: closed-form E[M] vs Monte-Carlo.

Sections (5)-(7) accept --trials and --seed (defaults reproduce the report).

No dependencies beyond the standard library.
"""

import argparse
import math
import random
from collections import Counter


def p_exact_m2(N: int, G: int, Fg: int, k: int, RF: int) -> float:
    """Exact M2 per-target eclipse probability."""
    if k < RF:
        return 0.0
    pull = 1.0
    for i in range(RF):
        pull *= (k - i) / (N - 1 - i)
    push = (1 - Fg / (N - 1)) ** G
    return push * pull


def p_approx_m2(N: int, G: int, Fg: int, k: int, RF: int) -> float:
    """Power-law approximation of M2 eclipse probability."""
    push = math.exp(-G * Fg / N)
    pull = (k / N) ** RF
    return push * pull


def p_approx_randcast(N: int, G: int, Fg: int, k: int, F: int) -> float:
    """Exponential approximation of RandCast/golden eclipse probability."""
    return math.exp(-(G * Fg + (N - G - k) * F) / N)


def k_max_m2_analytical(N: int, G: int, Fg: int, RF: int, eps: float) -> float:
    return N * eps ** (1 / RF) * math.exp(G * Fg / (N * RF))


def k_max_m2_exact(N: int, G: int, Fg: int, RF: int, eps: float) -> int:
    """Largest k in [0, N-G-1] with P_exact_M2(k) <= eps. -1 if none."""
    if p_exact_m2(N, G, Fg, 0, RF) > eps:
        return -1
    lo, hi = 0, N - G - 1
    while lo < hi:
        mid = (lo + hi + 1) // 2
        if p_exact_m2(N, G, Fg, mid, RF) <= eps:
            lo = mid
        else:
            hi = mid - 1
    return lo


def k_max_randcast_analytical(N: int, G: int, Fg: int, F: int, eps: float) -> float:
    return N * (1 - math.log(1 / eps) / F) + G * (Fg - F) / F


# ---------------------------------------------------------------------------
# Coverage distribution: closed-form mean/variance of the eclipsed honest-node
# count M, and single-round Monte-Carlo validation (properties #4, #10).
# ---------------------------------------------------------------------------

def q_push_exact(N: int, G: int, Fg: int) -> float:
    """P(no golden pushed to a given target) = (1 - Fg/(N-1))^G."""
    return (1 - Fg / (N - 1)) ** G


def q_pull_exact(N: int, k: int, RF: int) -> float:
    """P(all RF pulled forwarders adversarial) = C(k,RF)/C(N-1,RF)."""
    if k < RF:
        return 0.0
    v = 1.0
    for i in range(RF):
        v *= (k - i) / (N - 1 - i)
    return v


def count_moments_closed(N: int, G: int, Fg: int, k: int, RF: int):
    """Closed-form mean and variance of M = # eclipsed honest nodes (1 round).

    Returns (H, p, mean, var_closed, var_indep).

    Eclipse of distinct targets is correlated ONLY through the shared golden
    push pool -- each honest node pulls its own forwarders independently, so the
    pull layer contributes no cross-target covariance.  The pairwise covariance
    is

        Cov(X_i, X_j) = q_pull^2 * (q_pp2 - q_push^2),
        q_pp2 = [ (N-1-Fg)(N-2-Fg) / ((N-1)(N-2)) ]^G   (no golden hits i or j),

    and q_pp2 < q_push^2, so Cov < 0: M is *under-dispersed* relative to the
    independent (union-bound) estimate Var_indep = H p (1-p).
    """
    H = N - G - k
    qp = q_push_exact(N, G, Fg)
    ql = q_pull_exact(N, k, RF)
    p = qp * ql
    mean = H * p
    var_indep = H * p * (1 - p)
    a = (N - 1 - Fg) / (N - 1)                                    # avoid-one
    per2 = ((N - 1 - Fg) * (N - 2 - Fg)) / ((N - 1) * (N - 2))    # avoid-two
    # (q_pp2 - q_push^2) via expm1 to avoid catastrophic cancellation:
    diff = qp * qp * math.expm1(G * math.log(per2 / (a * a)))
    cov_pair = ql * ql * diff
    var_closed = var_indep + H * (H - 1) * cov_pair
    return H, p, mean, var_closed, var_indep


def _pull_fail(j: int, N: int, RF: int, is_adv, rng) -> bool:
    """One honest node's pull test: True iff all RF forwarders adversarial.

    Forwarders are RF distinct nodes drawn uniformly from {0..N-1} \\ {j}
    (index-shift excludes self)."""
    Nm1 = N - 1
    if RF == 1:
        r = rng.randrange(Nm1)
        return is_adv[r if r < j else r + 1]
    for r in rng.sample(range(Nm1), RF):
        if not is_adv[r if r < j else r + 1]:
            return False
    return True


def mc_eclipse_counts(N, G, Fg, k, RF, trials, rng, rf_of=None):
    """Monte-Carlo samples of M (eclipsed honest-node count), one per trial.

    Node layout: [0,G) golden, [G, G+H) regular honest, [G+H, N) adversary.
    A honest node is eclipsed iff no golden pushed to it AND all its pulled
    forwarders are adversarial.  rf_of: optional callable j -> RF_j for a
    heterogeneous per-node pull budget (property #10); defaults to uniform RF.
    """
    H = N - G - k
    adv_start = G + H
    is_adv = [i >= adv_start for i in range(N)]
    pool = range(N - 1)
    counts = []
    for _ in range(trials):
        covered = bytearray(N)
        for g in range(G):                       # golden push
            for r in rng.sample(pool, Fg):
                covered[r if r < g else r + 1] = 1
        M = 0
        for j in range(G, adv_start):            # regular honest pull + test
            if covered[j]:
                continue                         # push-covered -> not eclipsed
            rf_j = RF if rf_of is None else rf_of(j)
            if _pull_fail(j, N, rf_j, is_adv, rng):
                M += 1
        counts.append(M)
    return counts


def _mean_var(xs):
    n = len(xs)
    m = sum(xs) / n
    v = sum((x - m) ** 2 for x in xs) / (n - 1)
    return m, v


def report_count_regime(title, N, G, Fg, k, RF, trials, rng):
    """Print closed-form vs Monte-Carlo mean/variance for one regime."""
    H, p, mean_c, var_c, var_i = count_moments_closed(N, G, Fg, k, RF)
    mean_mc, var_mc = _mean_var(mc_eclipse_counts(N, G, Fg, k, RF, trials, rng))
    sigma = var_mc * math.sqrt(2 / (trials - 1))          # ~1-sigma on MC var
    print("=" * 78)
    print(title)
    print("=" * 78)
    print(f"  params: N={N} G={G} F_g={Fg} RF={RF} k={k}  "
          f"(mu={k/N:.2f}, lambda_push={G*Fg/N:.2f}, H={H})")
    print(f"  per-target eclipse p    closed {p:.6e}   MC {mean_mc/H:.6e}")
    print(f"  E[M]                    closed {mean_c:11.3f}   MC {mean_mc:11.3f}")
    print(f"  Var(M) exact (corr.)    closed {var_c:11.3f}   MC {var_mc:11.3f}"
          f"  (MC 1-sigma +/- {sigma:.3f})")
    print(f"  Var(M) independent est. {var_i:11.3f}   "
          f"under-dispersion {100 * (1 - var_c / var_i):.2f}%")
    print()


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Numerical verification of the M2 eclipse formula and "
                    "eclipsed-count distribution.")
    parser.add_argument("--trials", type=int, default=20000,
                        help="Monte-Carlo trials for sections (5)-(7) "
                             "(default 20000).")
    parser.add_argument("--seed", type=int, default=12345,
                        help="RNG seed for reproducibility (default 12345).")
    args = parser.parse_args()
    rng = random.Random(args.seed)

    N, G, Fg = 20000, 50, 200

    print(f"Parameters: N = {N}, G = {G}, F_g = {Fg}")
    print()

    # (1) P_exact vs P_approx for M2 (RF = 2)
    print("=" * 78)
    print("(1) M2: P_exact vs. power-law P_approx (RF = 2)")
    print("=" * 78)
    RF = 2
    print(f"{'k':>6} {'P_exact':>14} {'P_approx':>14} {'ratio':>10} {'rel. err':>10}")
    for k in [10, 100, 500, 1000, 2000, 5000, 10000]:
        if k < RF:
            continue
        pe = p_exact_m2(N, G, Fg, k, RF)
        pa = p_approx_m2(N, G, Fg, k, RF)
        ratio = pa / pe
        rel = (pa - pe) / pe
        print(f"{k:>6d} {pe:>14.6e} {pa:>14.6e} {ratio:>10.5f} {rel:>9.3%}")
    print()

    # (2) k_max analytical vs bisection on P_exact
    print("=" * 78)
    print("(2) M2: k_max analytical vs. exact bisection (RF = 2)")
    print("=" * 78)
    print(f"{'eps':>10} {'k_max (anal.)':>15} {'k_max (exact)':>15} {'diff':>8}")
    for eps in [1e-2, 1e-3, 1e-4, 1e-6]:
        ka = k_max_m2_analytical(N, G, Fg, RF, eps)
        ke = k_max_m2_exact(N, G, Fg, RF, eps)
        diff = ke - int(round(ka))
        print(f"{eps:>10.0e} {ka:>15.1f} {ke:>15d} {diff:>+8d}")
    print()

    # (3) M2 vs RandCast k_max at equal fanout
    print("=" * 78)
    print("(3) M2 vs. RandCast k_max at equal fanout F = RF (analytical)")
    print("=" * 78)
    print(f"{'F':>4} {'eps':>10} {'k_max RandCast':>16} {'k_max M2':>12}")
    for F in [2, 5, 10, 20]:
        for eps in [1e-2, 1e-6]:
            ka_rc = k_max_randcast_analytical(N, G, Fg, F, eps)
            ka_m2 = k_max_m2_analytical(N, G, Fg, F, eps)
            rc_str = f"{ka_rc:.0f}" if ka_rc > 0 else "infeasible"
            print(f"{F:>4d} {eps:>10.0e} {rc_str:>16} {ka_m2:>12.0f}")
    print()

    # (4) Verify P_M2 <= P_RandCast pointwise at equal fanout
    print("=" * 78)
    print("(4) P_M2 <= P_RandCast at equal fanout (F = 20), and ratio matches")
    print("    the closed-form (mu * exp(1-mu))^F.")
    print("=" * 78)
    F = 20
    print(f"{'mu':>6} {'k':>6} {'P_RandCast':>14} {'P_M2':>14} {'ratio':>11}"
          f" {'(g(mu))^F':>14}")
    for mu in [0.05, 0.10, 0.30, 0.50, 0.80]:
        k = int(round(mu * N))
        prc = p_approx_randcast(N, G, Fg, k, F)
        pm2 = p_approx_m2(N, G, Fg, k, F)
        ratio = pm2 / prc
        g_mu_F = (mu * math.exp(1 - mu)) ** F
        print(f"{mu:>6.2f} {k:>6d} {prc:>14.6e} {pm2:>14.6e}"
              f" {ratio:>11.3e} {g_mu_F:>14.3e}")
    print()

    # (5) Eclipsed-count M: the negative push-side covariance makes M
    #     under-dispersed vs the independent union-bound estimate.  MC variance
    #     tracks the CORRECTED closed form, not the independent one (~7%).
    report_count_regime(
        "(5) Eclipsed-count M: closed form vs Monte-Carlo (under-dispersion ~7%)",
        N=2000, G=30, Fg=100, k=600, RF=1, trials=args.trials, rng=rng)

    # (6) Stronger regime: RF=1, mu=0.5, lambda_push=1 -- larger under-dispersion
    #     (~11%), same conclusion.  This is the substantive new claim (#4).
    report_count_regime(
        "(6) Eclipsed-count M: closed form vs Monte-Carlo (under-dispersion ~11%)",
        N=2000, G=20, Fg=100, k=1000, RF=1, trials=args.trials, rng=rng)

    # (7) Heterogeneous per-node RF (property #10): E[M] = q_push * sum_j
    #     q_pull(RF_j).  RF cycles 1,2,3 across honest nodes.
    print("=" * 78)
    print("(7) Heterogeneous per-node RF: closed-form E[M] vs Monte-Carlo")
    print("=" * 78)
    Nh, Gh, Fgh, kh = 2000, 40, 100, 400
    Hh = Nh - Gh - kh
    def rf_of(j):
        return 1 + ((j - Gh) % 3)                       # RF in {1, 2, 3}
    qp_h = q_push_exact(Nh, Gh, Fgh)
    rf_counts = Counter(rf_of(j) for j in range(Gh, Gh + Hh))
    mean_het = qp_h * sum(cnt * q_pull_exact(Nh, kh, rf)
                          for rf, cnt in sorted(rf_counts.items()))
    counts_h = mc_eclipse_counts(Nh, Gh, Fgh, kh, RF=1, trials=args.trials,
                                 rng=rng, rf_of=rf_of)
    mean_h_mc, _ = _mean_var(counts_h)
    print(f"  params: N={Nh} G={Gh} F_g={Fgh} k={kh} "
          f"(mu={kh/Nh:.2f}, lambda_push={Gh*Fgh/Nh:.2f}, H={Hh})")
    print(f"  RF distribution across honest nodes: "
          f"{dict(sorted(rf_counts.items()))}")
    for rf in sorted(rf_counts):
        print(f"    RF={rf}: per-target eclipse "
              f"{qp_h * q_pull_exact(Nh, kh, rf):.6e}  x {rf_counts[rf]} nodes")
    print(f"  E[M]   closed {mean_het:.3f}   MC {mean_h_mc:.3f}")
    print()


if __name__ == "__main__":
    main()
