#!/usr/bin/env python3
"""M4 re-provisioning: cheapest RF vs mu_design and the +1-notch at mu = 0.2.

For each design adversarial fraction mu_design in the grid, invert the
validated coverage law (../properties/full_coverage.md):

    P_bad ~ 1 - e^-E,   E = H * C(k,RF)/C(N-1,RF) * (1 - RF/(N-1))^(H-1)

for the smallest RF with P(bad) <= 1e-4 at N = 20000, then report costs
(closed forms; --mc-costs cross-checks with the flood simulator of
sweep_m4_cost.py) and the mu-shift robustness of each new design point
(budget / collapse, as in sweep_m4_mu_shift.py).  --mc-law validates each
new frozen design at 2 elevated mu_eff cells (connectivity check).

Backs ../properties/re_provisioning.md.

Usage: python3 sweep_m4_reprovision.py [--mc-costs] [--mc-law]
                                       [--tail-check]
                                       [--trials T] [--seed SEED]

--tail-check (LONG, ~8 min; never run by CI) measures the
small-component tail factor in the high-mu regime the new grid points
live in (cf. RF = 11 at mu_design = 0.35): N = 4000, mu = 0.35, RF = 8
at E ~ 3e-3, 60 000 graphs, connectivity check.
"""

from __future__ import annotations

import argparse
import math
import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from m4_model import M4Params, M4Graph, sample_bad   # noqa: E402
from sweep_m4_cost import flood                      # noqa: E402

N = 20_000
DELTA = 1e-4
MU0 = 0.20
GRID = [0.20, 0.225, 0.25, 0.30, 0.35]


def k_of(mu: float) -> int:
    return int(round(mu * N))


def law(RF: int):
    """P(bad) of the frozen design RF as a function of mu_eff."""
    return lambda mu: M4Params(N=N, k=k_of(mu), RF=RF).p_bad()


def search(mu: float) -> int:
    """Smallest RF with P(bad) <= DELTA at mu."""
    RF = 1
    while law(RF)(mu) > DELTA:
        RF += 1
    return RF


def frac_RF(mu: float) -> float:
    """Fractional law crossing: real RF with
    H mu^RF e^{(H-1) ln(1 - RF/(N-1))} = -ln(1-DELTA)."""
    H = N - k_of(mu)
    target = -math.log1p(-DELTA)

    def E(RF: float) -> float:
        return H * mu ** RF * math.exp((H - 1) * math.log1p(-RF / (N - 1)))

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


def msgs_closed(mu: float, RF: int) -> float:
    """Honest->honest transmissions per message: flood over the honest
    subgraph's ~ H*RF*(H-1)/(N-1) undirected links, 2 sends per link minus
    the H-1 tree links used once (no resend on the arrival link)."""
    H = N - k_of(mu)
    return 2 * H * RF * (H - 1) / (N - 1) - (H - 1)


def describe(mu: float, RF: int, tag: str) -> None:
    pb = law(RF)
    H = N - k_of(mu)
    mumax = crossing(pb, DELTA, mu)
    msgs = msgs_closed(mu, RF)
    print(f"  {tag:>6} RF={RF:>2}  P(bad) {pb(mu):>8.2e}  msgs {msgs:>9,.0f} "
          f"({msgs / H:>5.2f}/node)  links 2RF={2 * RF:>2}  "
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
    ap.add_argument("--tail-check", action="store_true",
                    help="measure the deep-tail factor in the high-mu "
                         "regime (LONG: 60k graphs at N=4000)")
    ap.add_argument("--trials", type=int, default=40,
                    help="graphs per cost cell (default 40)")
    ap.add_argument("--seed", type=int, default=20260806)
    args = ap.parse_args()

    if args.tail_check:
        rng = random.Random(args.seed)
        p = M4Params(N=4000, k=1400, RF=8)
        T = 60_000
        pred = p.p_bad()
        print(f"M4 deep-tail factor, high-mu regime -- N=4000, mu=0.35, "
              f"RF=8, {T} graphs (seed {args.seed})")
        bad = sample_bad(p, T, rng)
        mc = bad / T
        se = math.sqrt(max(mc, 1 / T) * (1 - mc) / T)
        print(f"  pred {pred:.4g}  MC {mc:.4g}  ({bad}/{T})  "
              f"ratio x{mc / pred:.2f}  z={(mc - pred) / se:+.2f}")
        return

    print(f"M4 re-provisioning -- N = {N}, delta = {DELTA:g}, grid {GRID}")

    # -- 1. cheapest RF vs mu_design ------------------------------------------
    print("\n(1) cheapest RF per mu_design")
    designs = []
    for mu in GRID:
        RF = search(mu)
        print(f"mu_design = {mu:.3f}  (k = {k_of(mu)}, H = {N - k_of(mu)}) "
              f"-- fractional RF* = {frac_RF(mu):.2f}")
        describe(mu, RF, "best")
        designs.append((mu, RF))
        if 1.1 * law(RF)(mu) > DELTA:
            # the ~1.1x small-component tail correction (full_coverage.md
            # par.3) pushes this point over target: show the safe choice too
            print(f"         (x1.1 tail-corrected P(bad) "
                  f"{1.1 * law(RF)(mu):.2e} > delta -- corrected choice:)")
            describe(mu, RF + 1, "corr")

    # -- 2. +1 notch at mu = 0.2 ---------------------------------------------
    RF0 = search(MU0)
    print(f"\n(2) +1 notch at mu = {MU0}")
    describe(MU0, RF0, "base")
    describe(MU0, RF0 + 1, "+1")
    designs.append((MU0, RF0 + 1))

    if not (args.mc_costs or args.mc_law):
        return
    rng = random.Random(args.seed)

    # -- 3. cost cross-check --------------------------------------------------
    if args.mc_costs:
        T = args.trials
        print(f"\n(3) cost cross-check -- {T} graphs/cell (seed {args.seed})")
        print(f"  {'mu':>6} {'RF':>3} {'msgs MC':>10} {'closed':>10} "
              f"{'diff%':>6} {'hops max':>8} {'hops mean':>9}")
        for mu, RF in designs:
            p = M4Params(N=N, k=k_of(mu), RF=RF)
            sends = maxd = meand = 0.0
            for _ in range(T):
                g = M4Graph(p, rng)
                sd, mx, mn, _r = flood(g.adj, 0)
                sends += sd
                maxd += mx
                meand += mn
            sends /= T
            closed = msgs_closed(mu, RF)
            print(f"  {mu:>6.3f} {RF:>3} {sends:>10,.0f} {closed:>10,.0f} "
                  f"{100 * (sends / closed - 1):>+6.2f} "
                  f"{maxd / T:>8.2f} {meand / T:>9.2f}")

    # -- 4. law validation at elevated mu_eff (new designs only) --------------
    if args.mc_law:
        print(f"\n(4) law vs MC at elevated mu_eff -- connectivity check "
              f"(seed {args.seed})")
        print(f"  {'design':>7} {'mu_eff':>7} {'pred':>8} {'MC':>8} "
              f"{'bad/trials':>12} {'z':>6}")
        seen = set()
        for mu, RF in designs:
            if RF in seen or RF == 8:
                continue          # RF = 8 validated in mu_shift_robustness.md
            seen.add(RF)
            pb = law(RF)
            for target, T in ((0.10, 400), (0.40, 250)):
                cell = round(crossing(pb, target, mu) * 200) / 200
                pred = pb(cell)
                params = M4Params(N=N, k=k_of(cell), RF=RF)
                bad = sample_bad(params, T, rng)
                mc = bad / T
                se = math.sqrt(max(mc, 1 / T) * (1 - mc) / T)
                z = (mc - pred) / se
                print(f"  RF={RF:>2} {cell:>7.3f} {pred:>8.4f} {mc:>8.4f} "
                      f"{bad:>6}/{T:<5} {z:>+6.2f}")


if __name__ == "__main__":
    main()
