#!/usr/bin/env python3
"""Adversary tolerance k_max(eps): validation.

    k_max(eps) = max{ k : P_ecl(k) <= eps },   P_ecl = C(k,RF)/C(N-1,RF).

Exact evaluation is bisection on the exact hypergeometric P (monotone in k --
validated in check_p01).  The explicit design formula is the inversion of the
mu^RF approximation:

    k_max(eps)     ~ N * eps^(1/RF)          (per-target)
    k_max(eps_net) ~ N * (eps_net/N)^(1/RF)  (whole-network)

Checks:

  (1) Boundary property of the exact bisection (deterministic):
      P(k_max) <= eps < P(k_max + 1) across an (RF, eps) grid.

  (2) Explicit formula vs exact bisection (deterministic):
      diff = k_exact - floor(k_analytic) must satisfy 0 <= diff (conservative:
      the formula never over-promises tolerance) and diff <= max(5, 1% of
      k_exact) (sharp).  Regime flags where RF^2 << k_max fails.

  (3) Whole-network variant: numeric inversion of H(k)*P_exact(k) <= eps_net
      (first crossing; no-re-entry spot-checked) vs the analytic formula;
      same conservativeness + sharpness acceptance.

  (4) Monte-Carlo, operational meaning of the tolerance at the boundary:
      (a) per-target: at k = k_max(1e-2) in the running example, the empirical
          eclipse frequency matches P(k_max) (|z| <= 4) and stays <= eps
          within 4 SE;
      (b) whole-network: at k = k_max_net(eps_net = 0.05) in a small regime,
          the fraction of rounds with >= 1 eclipsed node is <= eps_net within
          4 SE (union-bound guarantee holds operationally).

Exit code 0 iff all checks pass.
"""

import argparse
import math
import random
import sys

from m2_model import M2Params, mean_var, sample_graphs


# ---------------------------------------------------------------------------
# k_max evaluations
# ---------------------------------------------------------------------------

def p_exact(N: int, RF: int, k: int) -> float:
    return M2Params(N=N, k=k, RF=RF).p_eclipse()


def k_max_exact(N: int, RF: int, eps: float) -> int:
    """Largest k in [0, N-1] with P_exact(k) <= eps (monotone bisection).
    Returns -1 if even k = 0 violates eps (cannot happen: P(0) = 0)."""
    lo, hi = 0, N - 1
    if p_exact(N, RF, lo) > eps:
        return -1
    while lo < hi:
        mid = (lo + hi + 1) // 2
        if p_exact(N, RF, mid) <= eps:
            lo = mid
        else:
            hi = mid - 1
    return lo


def k_max_analytic(N: int, RF: int, eps: float) -> float:
    return N * eps ** (1 / RF)


def whole_net_expected(N: int, RF: int, k: int) -> float:
    """Union-bound whole-network quantity H(k) * P_exact(k)."""
    return (N - k) * p_exact(N, RF, k)


def k_max_net_exact(N: int, RF: int, eps_net: float) -> int:
    """Largest k before the first crossing of H(k)*P_exact(k) > eps_net.

    H(k)*P(k) is increasing at the crossing (P grows ~k^RF, H shrinks
    linearly); we scan to the first violation and spot-check no re-entry."""
    k = 0
    while k <= N - 1 and whole_net_expected(N, RF, k) <= eps_net:
        k += 1
    k_max = k - 1
    # no-re-entry spot check on a coarse grid beyond the crossing
    for kk in range(k, N - 1, max(1, (N - k) // 20)):
        if whole_net_expected(N, RF, kk) <= eps_net:
            raise AssertionError(
                f"H(k)*P(k) re-enters <= eps_net at k={kk}; scan invalid")
    return k_max


def k_max_net_analytic(N: int, RF: int, eps_net: float) -> float:
    return N * (eps_net / N) ** (1 / RF)


# ---------------------------------------------------------------------------
# Checks
# ---------------------------------------------------------------------------

def check_boundary(N) -> list:
    """(1) P(k_max) <= eps < P(k_max + 1)."""
    results = []
    for RF in (1, 2, 3, 5):
        for eps in (1e-2, 1e-3, 1e-4, 1e-6):
            ke = k_max_exact(N, RF, eps)
            ok = (ke >= 0
                  and p_exact(N, RF, ke) <= eps
                  and (ke == N - 1 or p_exact(N, RF, ke + 1) > eps))
            results.append((f"boundary RF={RF} eps={eps:.0e} (k_max={ke})",
                            ok, ""))
    return results


def check_formula_vs_exact(N) -> list:
    """(2) analytic formula vs exact bisection: conservative and sharp."""
    results = []
    print(f"  {'RF':>3} {'eps':>7} {'k_analytic':>11} {'k_exact':>8} "
          f"{'diff':>5} {'RF^2/k':>7}")
    for RF in (1, 2, 3, 5):
        for eps in (1e-2, 1e-3, 1e-4, 1e-6):
            ka = math.floor(k_max_analytic(N, RF, eps))
            ke = k_max_exact(N, RF, eps)
            diff = ke - ka
            tol = max(5, math.ceil(0.01 * ke))
            ok = 0 <= diff <= tol
            regime = RF * RF / ke if ke > 0 else float("inf")
            flag = "  <-- outside RF^2<<k regime" if regime > 0.2 else ""
            print(f"  {RF:>3} {eps:>7.0e} {ka:>11} {ke:>8} {diff:>+5} "
                  f"{regime:>7.3f}{flag}")
            results.append(
                (f"formula RF={RF} eps={eps:.0e}", ok,
                 f"diff={diff:+d} (tol [0,{tol}])"))
    return results


def check_whole_network(N) -> list:
    """(3) whole-network analytic vs numeric inversion of H(k)*P(k)."""
    results = []
    print(f"  {'RF':>3} {'eps_net':>8} {'k_analytic':>11} {'k_exact':>8} "
          f"{'diff':>5}")
    for RF in (2, 3, 4, 5):
        for eps_net in (1e-2, 1e-4, 1e-6):
            ka = math.floor(k_max_net_analytic(N, RF, eps_net))
            ke = k_max_net_exact(N, RF, eps_net)
            diff = ke - ka
            # sharpness budget: 1% approximation error + the analytic
            # formula's documented H ~ N slack, ~ ke*ke/(N*RF) nodes
            # (it demands N*p <= eps_net instead of H(k)*p <= eps_net)
            tol = max(5, math.ceil(ke * (0.01 + ke / (N * RF))))
            ok = 0 <= diff <= tol
            print(f"  {RF:>3} {eps_net:>8.0e} {ka:>11} {ke:>8} {diff:>+5}")
            results.append(
                (f"whole-net RF={RF} eps_net={eps_net:.0e}", ok,
                 f"diff={diff:+d} (tol [0,{tol}])"))
    return results


def check_mc_boundary(trials_override, seed) -> list:
    """(4) operational MC checks at the tolerance boundary."""
    rng = random.Random(seed)
    results = []

    # (a) per-target, running example, eps = 1e-2
    N, RF, eps = 20000, 2, 1e-2
    ke = k_max_exact(N, RF, eps)
    params = M2Params(N=N, k=ke, RF=RF)
    p_b = params.p_eclipse()
    trials = trials_override or 60
    counts = [g.eclipsed_count() for g in sample_graphs(params, trials, rng)]
    m, v = mean_var(counts)
    p_mc = m / params.H
    se = math.sqrt(v / trials) / params.H
    z = (p_mc - p_b) / se if se > 0 else float("inf")
    ok = abs(z) <= 4 and p_mc <= eps + 4 * se
    print(f"  (a) per-target boundary: k_max({eps:.0e}) = {ke}  "
          f"[P(k_max) = {p_b:.4e}]")
    print(f"      p_MC = {p_mc:.4e}  (z = {z:+.2f}, eps = {eps:.0e})  "
          f"{'ok' if ok else 'FAIL'}")
    results.append(("MC per-target boundary", ok, f"z={z:+.2f}"))

    # (b) whole-network, small regime, eps_net = 0.05
    N, RF, eps_net = 2000, 2, 0.05
    ke = k_max_net_exact(N, RF, eps_net)
    params = M2Params(N=N, k=ke, RF=RF)
    bound = whole_net_expected(N, RF, ke)
    trials = trials_override or 4000
    hits = sum(1 for g in sample_graphs(params, trials, rng)
               if g.eclipsed_count() > 0)
    frac = hits / trials
    se = math.sqrt(max(frac, 1 / trials) * (1 - min(frac, 1 - 1 / trials))
                   / trials)
    ok = frac <= eps_net + 4 * se and frac <= bound + 4 * se
    print(f"  (b) whole-network boundary: k_max_net({eps_net}) = {ke}  "
          f"[H*p = {bound:.4f}]")
    print(f"      P(>=1 eclipsed) MC = {frac:.4f} over {trials} rounds  "
          f"(union bound {bound:.4f}, eps_net {eps_net})  "
          f"{'ok' if ok else 'FAIL'}")
    results.append(("MC whole-network boundary", ok,
                    f"frac={frac:.4f} vs eps_net={eps_net}"))
    return results


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--trials", type=int, default=None,
                    help="override MC trial counts in check (4)")
    ap.add_argument("--seed", type=int, default=12345)
    args = ap.parse_args()

    N = 20000
    print("Adversary tolerance k_max(eps): validation")
    print(f"running example: N={N}, pure pull")
    print("=" * 74)

    print("(1) boundary property P(k_max) <= eps < P(k_max+1)  [exact]")
    r1 = check_boundary(N)
    bad = [n for n, ok, _ in r1 if not ok]
    print(f"  {len(r1) - len(bad)}/{len(r1)} grid cells ok"
          + (f"; FAIL: {bad}" if bad else ""))
    print()

    print("(2) analytic formula vs exact bisection  [exact]")
    r2 = check_formula_vs_exact(N)
    print()

    print("(3) whole-network: analytic vs numeric inversion of H(k)*P(k)")
    r3 = check_whole_network(N)
    print()

    print("(4) Monte-Carlo at the tolerance boundary")
    r4 = check_mc_boundary(args.trials, args.seed)
    print()

    failures = [(n, d) for n, ok, d in r1 + r2 + r3 + r4 if not ok]
    print("=" * 74)
    if failures:
        print(f"RESULT: {len(failures)} FAILURE(S):")
        for n, d in failures:
            print(f"  - {n}  {d}")
        return 1
    print("RESULT: PASS -- k_max boundary exact; formula conservative and "
          "sharp; tolerance holds operationally")
    return 0


if __name__ == "__main__":
    sys.exit(main())
