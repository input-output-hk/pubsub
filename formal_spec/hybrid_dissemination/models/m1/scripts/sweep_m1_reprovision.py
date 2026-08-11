#!/usr/bin/env python3
"""M1 re-provisioning: cheapest F vs mu_design and the +1-notch at mu = 0.2.

For each design adversarial fraction mu_design in the grid, invert the
validated coverage law (../properties/full_coverage.md):

    P_bad ~ 1 - e^-E,   E = H [ (1 - F/(N-1))^(H-1) + C(k,F)/C(N-1,F) ]

for the smallest F with P(bad) <= 1e-4 at N = 20000, then report costs
(closed forms; --mc-costs cross-checks with the flood simulator of
sweep_m1_cost.py) and the mu-shift robustness of each new design point
(budget / collapse, as in sweep_m1_mu_shift.py).  --mc-law validates each
new frozen design at 2 elevated mu_eff cells (strong-connectivity check).

Backs ../properties/re_provisioning.md.

Usage: python3 sweep_m1_reprovision.py [--mc-costs] [--mc-law]
                                       [--trials T] [--seed SEED]
"""

from __future__ import annotations

import argparse
import math
import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from m1_model import M1Params, M1Graph, sample_bad   # noqa: E402
from sweep_m1_cost import flood                      # noqa: E402

N = 20_000
DELTA = 1e-4
MU0 = 0.20
GRID = [0.20, 0.225, 0.25, 0.30, 0.35]


def k_of(mu: float) -> int:
    return int(round(mu * N))


def law(F: int):
    """P(bad) of the frozen design F as a function of mu_eff."""
    return lambda mu: M1Params(N=N, k=k_of(mu), F=F).p_bad()


def search(mu: float) -> int:
    """Smallest F with P(bad) <= DELTA at mu."""
    F = 1
    while law(F)(mu) > DELTA:
        F += 1
    return F


def frac_F(mu: float) -> float:
    """Fractional law crossing: real F with E(F) = -ln(1-DELTA)
    (continuous continuation e^{(H-1) ln(1-F/(N-1))} + mu^F)."""
    H = N - k_of(mu)
    target = -math.log1p(-DELTA)

    def E(F: float) -> float:
        return H * (math.exp((H - 1) * math.log1p(-F / (N - 1))) + mu ** F)

    lo, hi = 1.0, 60.0
    for _ in range(80):
        mid = (lo + hi) / 2
        if E(mid) <= target:
            hi = mid
        else:
            lo = mid
    return hi


def crossing(pb, target: float, lo: float, hi: float = 0.9) -> float:
    """mu_eff at which the (increasing) law crosses `target`."""
    if pb(lo) > target:
        return float("nan")
    for _ in range(60):
        mid = (lo + hi) / 2
        if pb(mid) >= target:
            hi = mid
        else:
            lo = mid
    return hi


def slope_lnE(pb, mu: float, h: float = 5e-4) -> float:
    """Numeric d ln E / d mu at mu (E = -ln(1 - P_bad))."""
    e = lambda m: -math.log1p(-pb(m))
    return (math.log(e(mu + h)) - math.log(e(mu - h))) / (2 * h)


def msgs_closed(mu: float, F: int) -> float:
    """Honest->honest transmissions per message: each of the H honest
    nodes fires once, pushing to its honest targets (F picks thinned by
    (H-1)/(N-1))."""
    H = N - k_of(mu)
    return H * F * (H - 1) / (N - 1)


def describe(mu: float, F: int, tag: str) -> None:
    pb = law(F)
    H = N - k_of(mu)
    mumax = crossing(pb, DELTA, mu)
    msgs = msgs_closed(mu, F)
    print(f"  {tag:>6} F={F:>2}  P(bad) {pb(mu):>8.2e}  msgs {msgs:>9,.0f} "
          f"({msgs / H:>5.2f}/node)  links 2F={2 * F:>2}  "
          f"mu_max {mumax:.4f} (d {mumax - mu:+.4f}, "
          f"churn {(mumax - mu) / (1 - mu) * 100:4.1f}%)  "
          f"collapse {crossing(pb, 0.5, mu):.3f} "
          f"slope {slope_lnE(pb, mu):.0f}")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--mc-costs", action="store_true",
                    help="flood-simulate msgs/hops at each design point")
    ap.add_argument("--mc-law", action="store_true",
                    help="MC-validate each new frozen design at 2 elevated "
                         "mu_eff cells")
    ap.add_argument("--trials", type=int, default=40,
                    help="graphs per cost cell (default 40)")
    ap.add_argument("--seed", type=int, default=20260806)
    args = ap.parse_args()

    print(f"M1 re-provisioning -- N = {N}, delta = {DELTA:g}, grid {GRID}")

    # -- 1. cheapest F vs mu_design ------------------------------------------
    print("\n(1) cheapest F per mu_design")
    designs = []
    for mu in GRID:
        F = search(mu)
        print(f"mu_design = {mu:.3f}  (k = {k_of(mu)}, H = {N - k_of(mu)}) "
              f"-- fractional F* = {frac_F(mu):.2f}")
        describe(mu, F, "best")
        designs.append((mu, F))

    # -- 2. +1 notch at mu = 0.2 ---------------------------------------------
    F0 = search(MU0)
    print(f"\n(2) +1 notch at mu = {MU0}")
    describe(MU0, F0, "base")
    describe(MU0, F0 + 1, "+1")
    designs.append((MU0, F0 + 1))

    if not (args.mc_costs or args.mc_law):
        return
    rng = random.Random(args.seed)

    # -- 3. cost cross-check --------------------------------------------------
    if args.mc_costs:
        T = args.trials
        print(f"\n(3) cost cross-check -- {T} graphs/cell (seed {args.seed})")
        print(f"  {'mu':>6} {'F':>3} {'msgs MC':>10} {'closed':>10} "
              f"{'diff%':>6} {'hops max':>8} {'hops mean':>9}")
        for mu, F in designs:
            p = M1Params(N=N, k=k_of(mu), F=F)
            sends = maxd = meand = 0.0
            for _ in range(T):
                g = M1Graph(p, rng)
                sd, mx, mn, _r = flood(g.adj, 0)
                sends += sd
                maxd += mx
                meand += mn
            sends /= T
            closed = msgs_closed(mu, F)
            print(f"  {mu:>6.3f} {F:>3} {sends:>10,.0f} {closed:>10,.0f} "
                  f"{100 * (sends / closed - 1):>+6.2f} "
                  f"{maxd / T:>8.2f} {meand / T:>9.2f}")

    # -- 4. law validation at elevated mu_eff (new designs only) --------------
    if args.mc_law:
        print(f"\n(4) law vs MC at elevated mu_eff -- strong-connectivity "
              f"check (seed {args.seed})")
        print(f"  {'design':>7} {'mu_eff':>7} {'pred':>8} {'MC':>8} "
              f"{'bad/trials':>12} {'z':>6}")
        seen = set()
        for mu, F in designs:
            if F in seen or F == 24:
                continue          # F = 24 validated in mu_shift_robustness.md
            seen.add(F)
            pb = law(F)
            for target, T in ((0.10, 400), (0.40, 250)):
                cell = round(crossing(pb, target, mu) * 200) / 200
                pred = pb(cell)
                params = M1Params(N=N, k=k_of(cell), F=F)
                bad = sample_bad(params, T, rng)
                mc = bad / T
                se = math.sqrt(max(mc, 1 / T) * (1 - mc) / T)
                z = (mc - pred) / se
                print(f"  F={F:>3} {cell:>7.3f} {pred:>8.4f} {mc:>8.4f} "
                      f"{bad:>6}/{T:<5} {z:>+6.2f}")


if __name__ == "__main__":
    main()
