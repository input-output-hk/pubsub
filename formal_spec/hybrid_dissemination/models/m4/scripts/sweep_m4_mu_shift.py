#!/usr/bin/env python3
"""M4 mu-shift robustness: coverage degradation at frozen parameters.

The deployed operating point (RF = 9, selected at N = 20000, mu = 0.2 under
the disturbance-margin rule; the delta-cheapest point is RF = 8) is frozen
while the effective adversarial fraction mu_eff
rises: N stays 20000, k = round(mu_eff * N), H = N - k.  Reports the law
curve P(bad) vs mu_eff, the budget (largest mu_eff with P(bad) <= delta),
the collapse point (P(bad) = 1/2), and Monte-Carlo spot checks at elevated
mu_eff (honest-subgraph connectivity check, as in sim_m4_coverage.py).

Usage: python3 sweep_m4_mu_shift.py [--quick] [--seed SEED]
"""

from __future__ import annotations

import argparse
import math
import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from m4_model import M4Params, sample_bad  # noqa: E402

N = 20_000
RF = 9
DELTA = 1e-4
MU0 = 0.20

LAW_GRID = [0.20, 0.25, 0.30, 0.35, 0.40, 0.45, 0.50, 0.55, 0.60, 0.65]
MC_CELLS = [(0.45, 800), (0.50, 500), (0.55, 300)]   # (mu_eff, trials)


def p_bad(mu: float) -> float:
    return M4Params(N=N, k=int(round(mu * N)), RF=RF).p_bad()


def crossing(target: float, lo: float = MU0, hi: float = 0.9) -> float:
    """mu_eff at which the (increasing) law crosses `target`."""
    for _ in range(40):
        mid = (lo + hi) / 2
        if p_bad(mid) >= target:
            hi = mid
        else:
            lo = mid
    return hi


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--quick", action="store_true", help="law only, no MC")
    ap.add_argument("--seed", type=int, default=20260711)
    args = ap.parse_args()

    print(f"M4 mu-shift robustness -- frozen RF = {RF}, N = {N}, "
          f"delta = {DELTA:g}")
    print(f"  {'mu_eff':>6} {'P(bad) law':>12}")
    for mu in LAW_GRID:
        print(f"  {mu:>6.2f} {p_bad(mu):>12.4g}")
    print(f"  budget:   P(bad) <= {DELTA:g} up to mu_eff ~ "
          f"{crossing(DELTA):.4f}")
    print(f"  collapse: P(bad) = 1/2 at mu_eff ~ {crossing(0.5):.4f}")

    if args.quick:
        return
    rng = random.Random(args.seed)
    print(f"  {'mu_eff':>6} {'pred':>8} {'MC':>8} {'bad/trials':>12} {'z':>6}")
    for mu, T in MC_CELLS:
        pred = p_bad(mu)
        bad = sample_bad(M4Params(N=N, k=int(round(mu * N)), RF=RF), T, rng)
        mc = bad / T
        se = math.sqrt(max(mc, 1 / T) * (1 - mc) / T)
        z = (mc - pred) / se
        print(f"  {mu:>6.2f} {pred:>8.4f} {mc:>8.4f} "
              f"{bad:>6}/{T:<5} {z:>+6.2f}")


if __name__ == "__main__":
    main()
