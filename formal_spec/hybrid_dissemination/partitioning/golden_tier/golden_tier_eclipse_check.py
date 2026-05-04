#!/usr/bin/env python3
"""
Numerical verification of the golden-tier eclipse formula.

For a RandCast-style overlay with G never-failing golden nodes (fanout F_g),
H regular honest nodes (fanout F), and k adversarial nodes (N = G + H + k),
the per-target eclipse probability of a regular honest node j is exactly

    P_exact(j eclipsed) = (1 - F_g/(N-1))^G * (1 - F/(N-1))^(H-1)

under the modelling assumptions of the report (uniform without-replacement
sampling, independent forwarders, adversaries contribute nothing).

The exponential approximation is

    P_approx(j eclipsed) = exp(-lambda_j),
    lambda_j             = (G*F_g + (N - G - k)*F) / N.

This script:
  1. Compares P_exact and P_approx at varying k, for the running example
     N = 20000, G = 50, F_g = 200, F = 20.
  2. Compares the analytical adversary tolerance
        k_max(eps) = N*(1 - ln(1/eps)/F) + G*(F_g - F)/F
     against the largest k for which P_exact(k) <= eps (bisection).
  3. Reports the leading correction delta (Approx. A magnitude).

No dependencies beyond the standard library.
"""

import math


def p_eclipse_exact(N: int, G: int, F_g: int, k: int, F: int) -> float:
    """Exact per-target eclipse probability (Section 3 of report)."""
    H = N - G - k
    return (1 - F_g / (N - 1)) ** G * (1 - F / (N - 1)) ** (H - 1)


def lambda_j(N: int, G: int, F_g: int, k: int, F: int) -> float:
    """Expected honest in-degree at target j (Section 4)."""
    return (G * F_g + (N - G - k) * F) / N


def p_eclipse_approx(N: int, G: int, F_g: int, k: int, F: int) -> float:
    """Exponential approximation: exp(-lambda_j)."""
    return math.exp(-lambda_j(N, G, F_g, k, F))


def delta_correction(N: int, G: int, F_g: int, k: int, F: int) -> float:
    """Leading x^2/2 correction (Approx. A)."""
    H = N - G - k
    return (G * F_g ** 2 + (H - 1) * F ** 2) / (2 * (N - 1) ** 2)


def k_max_analytical(N: int, G: int, F_g: int, F: int, eps: float) -> float:
    """Analytical adversary tolerance (Section 5 of report)."""
    return N * (1 - math.log(1 / eps) / F) + G * (F_g - F) / F


def k_max_exact(N: int, G: int, F_g: int, F: int, eps: float) -> int:
    """Largest integer k in [0, N-G-1] with P_exact(k) <= eps. -1 if infeasible.

    P_exact is increasing in k (each additional adversary removes one regular
    honest forwarder), so bisection is sound.
    """
    if p_eclipse_exact(N, G, F_g, 0, F) > eps:
        return -1
    lo, hi = 0, N - G - 1
    while lo < hi:
        mid = (lo + hi + 1) // 2
        if p_eclipse_exact(N, G, F_g, mid, F) <= eps:
            lo = mid
        else:
            hi = mid - 1
    return lo


def main() -> None:
    N, G, F_g, F = 20000, 50, 200, 20

    print(f"Parameters: N = {N}, G = {G}, F_g = {F_g}, F = {F}")
    print()

    print("=" * 78)
    print("(1) P(j eclipsed): exact vs. exponential approximation")
    print("=" * 78)
    print(f"{'k':>6} {'k/N':>8} {'P_exact':>14} {'P_approx':>14}"
          f" {'P_appr/P_ex':>13} {'rel. err':>10}")
    for k in [0, 1000, 5000, 10000, 13000, 15000, 16000, 18000]:
        if k > N - G - 1:
            continue
        pe = p_eclipse_exact(N, G, F_g, k, F)
        pa = p_eclipse_approx(N, G, F_g, k, F)
        ratio = pa / pe
        rel = (pa - pe) / pe
        print(f"{k:>6d} {k / N:>8.2%} {pe:>14.6e} {pa:>14.6e}"
              f" {ratio:>13.5f} {rel:>9.2%}")
    print()

    print("=" * 78)
    print("(2) k_max(eps): analytical formula vs. exact bisection")
    print("=" * 78)
    print(f"{'eps':>10} {'ln(1/eps)':>11} {'k_max (analyt.)':>17}"
          f" {'k_max (exact)':>15} {'diff':>8}")
    for eps in [1e-2, 1e-3, 1e-4, 1e-6, 1e-8, 1e-9]:
        kma = k_max_analytical(N, G, F_g, F, eps)
        kme = k_max_exact(N, G, F_g, F, eps)
        if kme < 0:
            print(f"{eps:>10.0e} {math.log(1 / eps):>11.4f}"
                  f" {kma:>17.1f} {'infeasible':>15} {'—':>8}")
        else:
            diff = kme - int(round(kma))
            print(f"{eps:>10.0e} {math.log(1 / eps):>11.4f}"
                  f" {kma:>17.1f} {kme:>15d} {diff:>+8d}")
    print()

    print("=" * 78)
    print("(3) Approx. A magnitude: delta and predicted relative error")
    print("=" * 78)
    for k in [0, 10000, 16000]:
        if k > N - G - 1:
            continue
        d = delta_correction(N, G, F_g, k, F)
        pe = p_eclipse_exact(N, G, F_g, k, F)
        pa = p_eclipse_approx(N, G, F_g, k, F)
        observed = (pa - pe) / pe
        print(f"  k = {k:>6d} : delta = {d:.6f}"
              f"  predicted = {d:.4%}  observed = {observed:.4%}")


if __name__ == "__main__":
    main()
