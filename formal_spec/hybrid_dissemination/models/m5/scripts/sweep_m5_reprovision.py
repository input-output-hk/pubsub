#!/usr/bin/env python3
"""M5 re-provisioning: cheapest (k_in, k_out) vs mu_design and the +1-notch.

For each design adversarial fraction mu_design in the grid, invert the
validated coverage law (../properties/full_coverage.md):

    P_bad ~ 1 - e^-E,
    E = H [ q(k_in) e^-k_out(1-mu) + q(k_out) e^-k_in(1-mu) ],  q ~ mu^picks

for the smallest total budget B = k_in + k_out with a split meeting
P(bad) <= 1e-4 at N = 20000, most-balanced split per the model's
documented rule (all feasible splits reported: badness is exactly
symmetric in the swap, so splits are listed as k_in >= k_out).  Costs are
closed forms (--mc-costs cross-checks with the flood simulator of
sweep_m5_cost.py); robustness budget / collapse per design point as in
sweep_m5_mu_shift.py; --mc-law validates each new frozen design at 2
elevated mu_eff cells (strong-connectivity check).

Backs ../properties/re_provisioning.md.

Usage: python3 sweep_m5_reprovision.py [--mc-costs] [--mc-law]
                                       [--trials T] [--seed SEED]
"""

from __future__ import annotations

import argparse
import math
import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from m5_model import M5Params, M5Graph, sample_bad   # noqa: E402
from sweep_m5_cost import flood                      # noqa: E402

N = 20_000
DELTA = 1e-4
MU0 = 0.20
GRID = [0.20, 0.225, 0.25, 0.30, 0.35]


def k_of(mu: float) -> int:
    return int(round(mu * N))


def law(a: int, b: int):
    """P(bad) of the frozen design (k_in, k_out) = (a, b) vs mu_eff."""
    return lambda mu: M5Params(N=N, k=k_of(mu), k_in=a, k_out=b).p_bad()


def feasible_splits(mu: float, B: int):
    """All (a, b), a >= b >= 1, a + b = B, P(bad) <= DELTA at mu,
    most-balanced first."""
    return [(a, B - a) for a in range((B + 1) // 2, B)
            if B - a >= 1 and law(a, B - a)(mu) <= DELTA]


def search(mu: float):
    """Smallest budget B and its feasible splits (most-balanced first)."""
    for B in range(2, 80):
        fs = feasible_splits(mu, B)
        if fs:
            return B, fs
    raise RuntimeError("no feasible budget found")


def frac_balanced(mu: float) -> float:
    """Fractional law crossing: balanced real b with
    2 H mu^b e^{-b(1-mu)} = -ln(1-DELTA); fractional budget B* = 2 b*."""
    H = N - k_of(mu)
    target = -math.log1p(-DELTA)

    def E(b: float) -> float:
        return 2 * H * mu ** b * math.exp(-b * (1 - mu))

    lo, hi = 1.0, 40.0
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


def msgs_closed(mu: float, a: int, b: int) -> float:
    """Honest->honest transmissions per message: every honest node fires
    once on its honest propagation out-edges (own k_out picks + honest
    in-picks of others), H*(k_in+k_out) picks thinned by (H-1)/(N-1)."""
    H = N - k_of(mu)
    return H * (a + b) * (H - 1) / (N - 1)


def describe(mu: float, a: int, b: int, tag: str) -> None:
    p = M5Params(N=N, k=k_of(mu), k_in=a, k_out=b)
    pb = law(a, b)
    H = p.H
    mumax = crossing(pb, DELTA, mu)
    msgs = msgs_closed(mu, a, b)
    print(f"  {tag:>10} ({a:>2},{b:>2}) B={a + b:>2} "
          f"E_in {H * p.p_in_isolated():>8.2e} "
          f"E_out {H * p.p_out_isolated():>8.2e} "
          f"P(bad) {pb(mu):>8.2e}  msgs {msgs:>9,.0f} ({msgs / H:>5.2f}/node)"
          f"  links 2B={2 * (a + b):>2}  mu_max {mumax:.4f} "
          f"(d {mumax - mu:+.4f}, churn {(mumax - mu) / (1 - mu) * 100:4.1f}%)"
          f"  collapse {crossing(pb, 0.5, mu):.3f} "
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

    print(f"M5 re-provisioning -- N = {N}, delta = {DELTA:g}, grid {GRID}")

    # -- 1. cheapest (k_in, k_out) vs mu_design -------------------------------
    print("\n(1) cheapest parameters per mu_design (documented rule: "
          "most-balanced split of the smallest budget)")
    designs = []
    for mu in GRID:
        B, fs = search(mu)
        bfrac = frac_balanced(mu)
        print(f"mu_design = {mu:.3f}  (k = {k_of(mu)}, H = {N - k_of(mu)}) "
              f"-- smallest budget B = {B}; feasible splits {fs}; "
              f"fractional balanced b* = {bfrac:.2f}, B* = {2 * bfrac:.2f}")
        describe(mu, *fs[0], "balanced")
        designs.append((mu, *fs[0]))
        for a, b in fs[1:]:
            describe(mu, a, b, "alt")

    # -- 2. +1 notch at mu = 0.2 ---------------------------------------------
    B0, fs0 = search(MU0)
    print(f"\n(2) +1 notch at mu = {MU0} (budget {B0} -> {B0 + 1}, "
          f"every feasible split)")
    describe(MU0, *fs0[0], "base")
    for a, b in feasible_splits(MU0, B0 + 1):
        describe(MU0, a, b, "+1-budget")
    fs1 = feasible_splits(MU0, B0 + 1)
    designs.append((MU0, *fs1[0]))

    if not (args.mc_costs or args.mc_law):
        return
    rng = random.Random(args.seed)

    # -- 3. cost cross-check --------------------------------------------------
    if args.mc_costs:
        T = args.trials
        print(f"\n(3) cost cross-check -- {T} graphs/cell (seed {args.seed})")
        print(f"  {'mu':>6} {'(a,b)':>8} {'msgs MC':>10} {'closed':>10} "
              f"{'diff%':>6} {'hops max':>8} {'hops mean':>9}")
        for mu, a, b in designs:
            p = M5Params(N=N, k=k_of(mu), k_in=a, k_out=b)
            sends = maxd = meand = 0.0
            for _ in range(T):
                g = M5Graph(p, rng)
                sd, mx, mn, _r = flood(g.adj, 0)
                sends += sd
                maxd += mx
                meand += mn
            sends /= T
            closed = msgs_closed(mu, a, b)
            print(f"  {mu:>6.3f} ({a:>2},{b:>2}) {sends:>10,.0f} "
                  f"{closed:>10,.0f} {100 * (sends / closed - 1):>+6.2f} "
                  f"{maxd / T:>8.2f} {meand / T:>9.2f}")

    # -- 4. law validation at elevated mu_eff (new designs only) --------------
    if args.mc_law:
        print(f"\n(4) law vs MC at elevated mu_eff -- strong-connectivity "
              f"check (seed {args.seed})")
        print(f"  {'design':>8} {'mu_eff':>7} {'pred':>8} {'MC':>8} "
              f"{'bad/trials':>12} {'z':>6}")
        seen = set()
        for mu, a, b in designs:
            if (a, b) in seen or (a, b) == (9, 8):
                continue        # (9, 8) validated in mu_shift_robustness.md
            seen.add((a, b))
            pb = law(a, b)
            for target, T in ((0.10, 400), (0.40, 250)):
                cell = round(crossing(pb, target, mu) * 200) / 200
                pred = pb(cell)
                params = M5Params(N=N, k=k_of(cell), k_in=a, k_out=b)
                bad = sample_bad(params, T, rng)
                mc = bad / T
                se = math.sqrt(max(mc, 1 / T) * (1 - mc) / T)
                z = (mc - pred) / se
                print(f"  ({a:>2},{b:>2}) {cell:>7.3f} {pred:>8.4f} "
                      f"{mc:>8.4f} {bad:>6}/{T:<5} {z:>+6.2f}")


if __name__ == "__main__":
    main()
