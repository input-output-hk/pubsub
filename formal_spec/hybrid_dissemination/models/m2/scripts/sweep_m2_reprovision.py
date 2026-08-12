#!/usr/bin/env python3
"""M2 re-provisioning: cheapest RF vs mu_design and the +1-notch at mu = 0.2.

For each design adversarial fraction mu_design in the grid, invert the
validated coverage law (../properties/full_coverage.md, mean-field form as
in sweep_m2_mu_shift.py):

    P_bad ~ 1 - e^-E,   E = H [ (1 - rho_f) + u ]

(rho_f = ignition/branching survival at RF(1-mu), u = eclipse-floor fixed
point) for the smallest RF with P(bad) <= 1e-4 at N = 20000, then report
costs (closed forms; --mc-costs cross-checks with the flood simulator of
sweep_m2_cost.py) and the mu-shift robustness of each new design point
(budget / collapse, as in sweep_m2_mu_shift.py).  --mc-law validates each
new frozen design at 2 elevated mu_eff cells (strong-connectivity check).

Backs ../properties/re_provisioning.md.

Usage: python3 sweep_m2_reprovision.py [--mc-costs] [--mc-law]
                                       [--trials T] [--seed SEED]
"""

from __future__ import annotations

import argparse
import math
import os
import random
import sys

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _HERE)
sys.path.insert(0, os.path.join(_HERE, "..", "..", "m3", "scripts"))

from m2_model import M2Params                          # noqa: E402
from m3_model import M3Graph, rho_giant, u_iterate     # noqa: E402
from sweep_m2_cost import flood, strongly_connected    # noqa: E402

N = 20_000
DELTA = 1e-4
MU0 = 0.20
GRID = [0.20, 0.225, 0.25, 0.30, 0.35]


def k_of(mu: float) -> int:
    return int(round(mu * N))


def law(RF: int):
    """P(bad) of the frozen design RF as a function of mu_eff (mean-field,
    as in sweep_m2_mu_shift.py)."""
    def pb(mu: float) -> float:
        H = N - k_of(mu)
        rho = rho_giant(RF * (1 - mu))
        u = u_iterate(mu, RF)
        return 1 - math.exp(-H * ((1 - rho) + u))
    return pb


def search(mu: float) -> int:
    """Smallest RF with P(bad) <= DELTA at mu."""
    RF = 2
    while law(RF)(mu) > DELTA:
        RF += 1
    return RF


def frac_RF(mu: float) -> float:
    """Fractional law crossing: real RF with
    H [e^{-RF(1-mu)} + mu^RF] = -ln(1-DELTA) (exponential-tail forms)."""
    H = N - k_of(mu)
    target = -math.log1p(-DELTA)

    def E(RF: float) -> float:
        return H * (math.exp(-RF * (1 - mu)) + mu ** RF)

    lo, hi = 2.0, 60.0
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
    """Honest->honest transmissions per message: each honest node fires
    once toward its honest requesters (H*RF picks thinned by (H-1)/(N-1))."""
    H = N - k_of(mu)
    return H * RF * (H - 1) / (N - 1)


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
    ap.add_argument("--trials", type=int, default=40,
                    help="graphs per cost cell (default 40)")
    ap.add_argument("--seed", type=int, default=20260806)
    args = ap.parse_args()

    print(f"M2 re-provisioning -- N = {N}, delta = {DELTA:g}, grid {GRID}")

    # -- 1. cheapest RF vs mu_design ------------------------------------------
    print("\n(1) cheapest RF per mu_design")
    designs = []
    for mu in GRID:
        RF = search(mu)
        print(f"mu_design = {mu:.3f}  (k = {k_of(mu)}, H = {N - k_of(mu)}) "
              f"-- fractional RF* = {frac_RF(mu):.2f}")
        describe(mu, RF, "best")
        designs.append((mu, RF))

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
            k = k_of(mu)
            H = N - k
            p = M2Params(N=N, k=k, RF=RF)
            sends = maxd = meand = 0.0
            for _ in range(T):
                g = M3Graph(p, rng)
                sd, mx, mn, _r = flood(g.adjacency(), 0, H)
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
        print(f"\n(4) law vs MC at elevated mu_eff -- strong-connectivity "
              f"check (seed {args.seed})")
        print(f"  {'design':>7} {'mu_eff':>7} {'pred':>8} {'MC':>8} "
              f"{'bad/trials':>12} {'z':>6}")
        seen = set()
        for mu, RF in designs:
            if RF in seen or RF == 24:
                continue          # RF = 24 validated in mu_shift_robustness.md
            seen.add(RF)
            pb = law(RF)
            for target, T in ((0.10, 400), (0.40, 250)):
                cell = round(crossing(pb, target, mu) * 200) / 200
                pred = pb(cell)
                k = k_of(cell)
                params = M2Params(N=N, k=k, RF=RF)
                bad = 0
                for _ in range(T):
                    g = M3Graph(params, rng)
                    if not strongly_connected(g.adjacency(), N - k):
                        bad += 1
                mc = bad / T
                se = math.sqrt(max(mc, 1 / T) * (1 - mc) / T)
                z = (mc - pred) / se
                print(f"  RF={RF:>2} {cell:>7.3f} {pred:>8.4f} {mc:>8.4f} "
                      f"{bad:>6}/{T:<5} {z:>+6.2f}")


if __name__ == "__main__":
    main()
