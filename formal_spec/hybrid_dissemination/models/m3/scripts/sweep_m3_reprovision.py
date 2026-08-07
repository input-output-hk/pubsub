#!/usr/bin/env python3
"""M3 re-provisioning: cheapest (RF, s) vs mu_design and the notch points.

For each design adversarial fraction mu_design in the grid, invert the
validated coverage law (../properties/full_coverage.md):

    P_bad ~ 1 - e^-E,   E = H [ mu^RF + mu^(s-1) e^-RF(1-mu) ]

for the smallest total budget B = RF + (s-1) with a split meeting
P(bad) <= 1e-4 at N = 20000, then pick the bandwidth-minimal split
(smallest RF) per the model's documented rule -- and ADDITIONALLY the
robustness-optimal split at the same budget (largest mu-shift budget),
since shifting budget from s-1 toward RF buys mu-headroom at bandwidth
cost.  Costs are closed forms (--mc-costs cross-checks with the seeded
flood simulator of sweep_m3_cost.py); robustness budget / collapse per
design point as in sweep_m3_mu_shift.py; --mc-law validates each NEW
frozen design at 2 elevated mu_eff cells (exact every-publisher check).

Two notch flavours at mu = 0.2 (they are distinct):
  A. re-split at fixed budget B = 19 toward the mu-sensitive in-term
     ((13, 7): same 19 links, +0.8 relay copies/node);
  B. +1 total budget (B = 20), every feasible split.

Backs ../properties/re_provisioning.md.

Usage: python3 sweep_m3_reprovision.py [--mc-costs] [--mc-law]
                                       [--tail-check]
                                       [--trials T] [--seed SEED]

--tail-check (LONG, ~10 min; never run by CI) measures the
small-component tail factor in the out-term-dominated regime the new
grid points live in (cf. (17, 7) at mu_design = 0.3): N = 4000,
mu = 0.3, (RF, s) = (12, 5) -- same ~3.5:1 out:in defect mix -- at
E ~ 7e-3, 40 000 graphs, exact every-publisher check.
"""

from __future__ import annotations

import argparse
import math
import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from m3_model import M3Params, M3Graph, sample_strict_bad   # noqa: E402
from sweep_m3_cost import flood_seeded                       # noqa: E402

N = 20_000
DELTA = 1e-4
MU0 = 0.20
GRID = [0.20, 0.225, 0.25, 0.30, 0.35]


def k_of(mu: float) -> int:
    return int(round(mu * N))


def law(RF: int, s: int):
    """P(bad) of the frozen design (RF, s) as a function of mu_eff."""
    return lambda mu: M3Params(N=N, k=k_of(mu), RF=RF, s=s).p_bad()


def feasible_splits(mu: float, B: int):
    """All (RF, s) with RF + (s-1) = B and P(bad) <= DELTA at mu."""
    return [(RF, B - RF + 1) for RF in range(1, B + 1)
            if B - RF + 1 >= 1 and law(RF, B - RF + 1)(mu) <= DELTA]


def search(mu: float):
    """Smallest budget B and its feasible splits (bandwidth-minimal first)."""
    for B in range(2, 80):
        fs = feasible_splits(mu, B)
        if fs:
            return B, fs
    raise RuntimeError("no feasible budget found")


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


def robust_split(mu: float, splits):
    """The split with the largest mu-shift budget (breaking ties bw-minimal)."""
    return max(splits, key=lambda t: (crossing(law(*t), DELTA, mu), -t[0]))


def slope_lnE(pb, mu: float, h: float = 5e-4) -> float:
    """Numeric d ln E / d mu at mu (E = -ln(1 - P_bad))."""
    e = lambda m: -math.log1p(-pb(m))
    return (math.log(e(mu + h)) - math.log(e(mu - h))) / (2 * h)


def msgs_closed(mu: float, RF: int, s: int) -> float:
    """Honest->honest transmissions per message (fire-once, no resend on the
    arrival link): H*RF pull edges thinned to honest forwarders, plus the
    publisher's s-1 initiation copies to honest targets."""
    H = N - k_of(mu)
    return (H * RF + (s - 1)) * (H - 1) / (N - 1)


def frac_sizing(mu: float):
    """Fractional law crossings (delta split half/half, as in
    full_coverage.md): RF* = ln(2H/d)/ln(1/mu), (s-1)* closes the out-term."""
    H = N - k_of(mu)
    rf = math.log(2 * H / DELTA) / math.log(1 / mu)
    s1 = (math.log(2 * H / DELTA) - rf * (1 - mu)) / math.log(1 / mu)
    return rf, s1


def describe(mu: float, RF: int, s: int, tag: str) -> None:
    p = M3Params(N=N, k=k_of(mu), RF=RF, s=s)
    pb = law(RF, s)
    H = p.H
    mumax = crossing(pb, DELTA, mu)
    msgs = msgs_closed(mu, RF, s)
    print(f"  {tag:>10} ({RF:>2},{s:>2}) B={RF + s - 1:>2} "
          f"E_in {H * p.p_in_isolated():>8.2e} "
          f"E_out {H * p.p_out_isolated():>8.2e} "
          f"P(bad) {pb(mu):>8.2e}  msgs {msgs:>9,.0f} ({msgs / H:>5.2f}/node)"
          f"  links 2B={2 * (RF + s - 1):>2}  mu_max {mumax:.4f} "
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
    ap.add_argument("--tail-check", action="store_true",
                    help="measure the deep-tail factor in the out-dominated "
                         "regime (LONG: 40k graphs at N=4000)")
    ap.add_argument("--trials", type=int, default=40,
                    help="graphs per cost cell (default 40)")
    ap.add_argument("--seed", type=int, default=20260806)
    args = ap.parse_args()

    if args.tail_check:
        rng = random.Random(args.seed)
        p = M3Params(N=4000, k=1200, RF=12, s=5)
        T = 40_000
        pred = p.p_bad()
        print(f"M3 deep-tail factor, out-dominated regime -- N=4000, "
              f"mu=0.3, (12, 5), E_out:E_in = "
              f"{p.p_out_isolated() / p.p_in_isolated():.1f}:1, "
              f"{T} graphs (seed {args.seed})")
        bad = sample_strict_bad(p, T, rng)
        mc = bad / T
        se = math.sqrt(max(mc, 1 / T) * (1 - mc) / T)
        print(f"  pred {pred:.4g}  MC {mc:.4g}  ({bad}/{T})  "
              f"ratio x{mc / pred:.2f}  z={(mc - pred) / se:+.2f}")
        return

    print(f"M3 re-provisioning -- N = {N}, delta = {DELTA:g}, "
          f"grid {GRID}")

    # -- 1. cheapest parameters vs mu_design --------------------------------
    print("\n(1) cheapest parameters per mu_design "
          "(bw = bandwidth-minimal split, rb = robustness-optimal split "
          "at the same budget)")
    designs = []          # (mu, RF, s, tag) for the MC sections
    for mu in GRID:
        B, fs = search(mu)
        bw = fs[0]
        rb = robust_split(mu, fs)
        rf_f, s1_f = frac_sizing(mu)
        print(f"mu_design = {mu:.3f}  (k = {k_of(mu)}, H = {N - k_of(mu)}) "
              f"-- smallest budget B = {B}; feasible splits {fs}; "
              f"fractional RF* = {rf_f:.2f}, (s-1)* = {s1_f:.2f}, "
              f"B* = {rf_f + s1_f:.2f}")
        describe(mu, *bw, "bw-min")
        designs.append((mu, *bw, "bw"))
        if rb != bw:
            describe(mu, *rb, "rb-opt")
            designs.append((mu, *rb, "rb"))

    # -- 2. notches at mu = 0.2 ----------------------------------------------
    mu = MU0
    B, fs = search(mu)
    print(f"\n(2) notches at mu = {MU0} -- base is the bandwidth-minimal "
          f"split of B = {B}")
    describe(mu, *fs[0], "base")
    print(f"  A. same-budget re-splits (B = {B}):")
    for RF, s in fs[1:]:
        describe(mu, RF, s, "re-split")
    print(f"  B. +1 total budget (B = {B + 1}):")
    for RF, s in feasible_splits(mu, B + 1):
        describe(mu, RF, s, "+1-budget")
    notch = [(mu, 13, 7, "notchA"), (mu, 13, 8, "+1B bw+1"), (mu, 14, 7, "+1B rb")]
    designs += notch

    # -- 3. fractional frontier trend vs M4 ----------------------------------
    # Stair-free bandwidth trend from the documented sizing rules:
    # M3: RF* = ln(2H/d)/ln(1/mu),        msgs = H RF* (H-1)/(N-1)
    # M4: RF* = ln(H/d)/(ln(1/mu)+(1-mu)), msgs = 2 H RF* (H-1)/(N-1) - (H-1)
    # (M4 rule per ../../m4/properties/full_coverage.md).
    def frac_msgs(mu_):
        H = N - k_of(mu_)
        m3 = (math.log(2 * H / DELTA) / math.log(1 / mu_)
              ) * H * (H - 1) / (N - 1)
        m4 = 2 * (math.log(H / DELTA) / (math.log(1 / mu_) + 1 - mu_)
                  ) * H * (H - 1) / (N - 1) - (H - 1)
        return m3, m4

    print("\n(3) fractional frontier trend -- M4/M3 bandwidth ratio "
          "(stair-free sizing rules)")
    print(f"  {'mu':>5} {'msgs M3*':>10} {'msgs M4*':>10} {'M4/M3':>6}")
    for mu_ in [0.20, 0.25, 0.30, 0.35, 0.40, 0.45, 0.50, 0.55, 0.60,
                0.65, 0.70]:
        m3, m4 = frac_msgs(mu_)
        print(f"  {mu_:>5.2f} {m3:>10,.0f} {m4:>10,.0f} {m4 / m3:>6.3f}")
    lo, hi = 0.20, 0.89
    for _ in range(60):
        mid = (lo + hi) / 2
        m3, m4 = frac_msgs(mid)
        if m4 >= m3:
            lo = mid
        else:
            hi = mid
    print(f"  parity (M4* = M3*) at mu ~ {lo:.3f}")

    if not (args.mc_costs or args.mc_law):
        return
    rng = random.Random(args.seed)

    # -- 3. cost cross-check (flood simulator, seeded publisher) ------------
    if args.mc_costs:
        T = args.trials
        print(f"\n(4) cost cross-check -- {T} graphs/cell "
              f"(seed {args.seed})")
        print(f"  {'mu':>6} {'(RF,s)':>8} {'msgs MC':>10} {'closed':>10} "
              f"{'diff%':>6} {'hops max':>8} {'hops mean':>9}")
        for mu, RF, s, tag in designs:
            k = k_of(mu)
            H = N - k
            p = M3Params(N=N, k=k, RF=RF, s=s)
            sends = maxd = meand = 0.0
            for _ in range(T):
                g = M3Graph(p, rng)
                adj = g.adjacency()
                seeds = [(0, 0)]
                push = 0
                for t in g.init_targets[0]:
                    push += 1
                    seeds.append((t, 1))
                sd, mx, mn, _r = flood_seeded(adj, seeds, H)
                sends += sd + push
                maxd += mx
                meand += mn
            sends /= T
            closed = msgs_closed(mu, RF, s)
            print(f"  {mu:>6.3f} ({RF:>2},{s:>2}) {sends:>10,.0f} "
                  f"{closed:>10,.0f} {100 * (sends / closed - 1):>+6.2f} "
                  f"{maxd / T:>8.2f} {meand / T:>9.2f}")

    # -- 4. law validation at elevated mu_eff (new designs only) ------------
    if args.mc_law:
        print(f"\n(5) law vs MC at elevated mu_eff -- exact every-publisher "
              f"check (seed {args.seed})")
        print(f"  {'design':>12} {'mu_eff':>7} {'pred':>8} {'MC':>8} "
              f"{'bad/trials':>12} {'z':>6}")
        seen = set()
        for mu, RF, s, tag in designs:
            if (RF, s) in seen or (RF, s) == (12, 8):
                continue          # (12, 8) validated in mu_shift_robustness.md
            seen.add((RF, s))
            pb = law(RF, s)
            for target, T in ((0.10, 400), (0.40, 250)):
                cell = round(crossing(pb, target, mu) * 200) / 200
                pred = pb(cell)
                params = M3Params(N=N, k=k_of(cell), RF=RF, s=s)
                bad = sample_strict_bad(params, T, rng)
                mc = bad / T
                se = math.sqrt(max(mc, 1 / T) * (1 - mc) / T)
                z = (mc - pred) / se
                print(f"  ({RF:>2},{s:>2}) {cell:>7.3f} {pred:>8.4f} "
                      f"{mc:>8.4f} {bad:>6}/{T:<5} {z:>+6.2f}")


if __name__ == "__main__":
    main()
