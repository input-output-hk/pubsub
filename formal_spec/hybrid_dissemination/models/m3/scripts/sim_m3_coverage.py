#!/usr/bin/env python3
"""M3 coverage study under the strict criterion: P(bad graph) vs the
two-class isolated-vertex closed form, and the "which (RF, s)" answer at
N=20000, mu=0.2.  Backs ../properties/full_coverage.md.

A sampled graph is bad iff some honest publisher's messages cannot reach
every honest node (message = publisher + its s-1 standing initiation targets,
spreading over the pull relay edges).  Dominated by in-isolated nodes
(all RF pull picks dead, ~ mu^RF -- initiation links cannot help reception)
and out-isolated publishers (no honest requester AND all s-1 initiation
targets dead, ~ mu^(s-1)*e^{-RF(1-mu)}).
"""

import argparse
import math
import random

from m3_model import M3Params, sample_strict_bad


# (N, mu, RF, s, trials) -- P(bad) from ~0.67 down to ~9e-3
PRESET = [
    (4000, 0.2, 6, 4, 600),
    (4000, 0.2, 8, 3, 1000),
    (4000, 0.2, 8, 5, 4000),
    (4000, 0.2, 10, 4, 8000),
    (20000, 0.2, 8, 3, 300),
    (20000, 0.2, 10, 4, 800),
]


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--seed", type=int, default=2024)
    ap.add_argument("--target", type=float, default=1e-4,
                    help="per-epoch P(bad) target for the closed-form answer")
    args = ap.parse_args()
    rng = random.Random(args.seed)

    print("M3 P(bad), strict criterion: closed form vs Monte-Carlo")
    print(f"  {'N':>6} {'mu':>4} {'RF':>3} {'s':>3} {'E_def':>8} {'pred':>8} "
          f"{'MC':>8} {'bad/trials':>12} {'z':>6}")
    for (N, mu, RF, s, T) in PRESET:
        k = int(round(mu * N))
        p = M3Params(N=N, k=k, RF=RF, s=s)
        bad = sample_strict_bad(p, T, rng)
        mc = bad / T
        pred = p.p_bad()
        se = math.sqrt(max(mc, 1 / T) * (1 - mc) / T)
        z = (mc - pred) / se if se > 0 else float("nan")
        print(f"  {N:>6} {mu:>4} {RF:>3} {s:>3} {p.E_defects():>8.4f} "
              f"{pred:>8.4f} {mc:>8.4f} {bad:>6}/{T:<5} {z:>+6.2f}")

    print()
    N, mu = 20000, 0.2
    k = int(round(mu * N))
    print(f"Closed-form answer at N={N}, mu={mu}: best (RF, s-1) per total "
          f"budget B = RF + (s-1)")
    print(f"  {'B':>3} {'best (RF,s)':>12} {'P(bad)':>10}")
    best_B = None
    for B in range(16, 22):
        pb, RF = min(((M3Params(N=N, k=k, RF=RF, s=B - RF + 1).p_bad(), RF)
                      for RF in range(1, B)))
        print(f"  {B:>3} {f'({RF},{B-RF+1})':>12} {pb:>10.3e}")
        if best_B is None and pb <= args.target:
            best_B = B
    print(f"  -> smallest budget B = {best_B}")

    print()
    print(f"Pareto points at B = {best_B} (bandwidth follows RF only):")
    for RF in range(11, 15):
        s = best_B - RF + 1
        if s < 1:
            continue
        p = M3Params(N=N, k=k, RF=RF, s=s)
        flag = "  <= target" if p.p_bad() <= args.target else ""
        print(f"  (RF={RF}, s={s}): P(bad) = {p.p_bad():.3e}, "
              f"relay copies/node = {RF * (1 - mu):.1f}{flag}")


if __name__ == "__main__":
    main()
