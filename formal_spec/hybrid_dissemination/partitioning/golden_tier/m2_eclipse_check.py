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

No dependencies beyond the standard library.
"""

import math


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


def main() -> None:
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


if __name__ == "__main__":
    main()
