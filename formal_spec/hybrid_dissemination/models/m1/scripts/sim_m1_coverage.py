#!/usr/bin/env python3
"""M1 coverage study: P(bad graph) vs the isolated-vertex closed form,
and the "which F" answer at N=20000, mu=0.2.  Backs
../properties/full_coverage.md.

A sampled graph is bad iff the honest push digraph is not strongly connected
(some honest publisher cannot reach some honest node) -- dominated by
in-isolated nodes (no honest in-edge, seed-proof; ~ H*e^{-F(1-mu)}) plus the
far smaller out-isolated class (muted publishers, ~ H*mu^F).  Validation
cells are chosen where P(bad) is measurable; beyond, the validated closed
form answers the design question.
"""

import argparse
import math
import random

from m1_model import M1Params, sample_bad


# (N, mu, F, trials) -- P(bad) from ~0.66 down to ~9e-3
PRESET = [
    (4000, 0.2, 10, 1000),
    (4000, 0.2, 12, 2000),
    (4000, 0.2, 14, 4000),
    (4000, 0.2, 16, 8000),
    (20000, 0.2, 12, 400),
    (20000, 0.2, 14, 1000),
]


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--seed", type=int, default=2024)
    ap.add_argument("--target", type=float, default=1e-4,
                    help="per-epoch P(bad) target for the closed-form answer")
    args = ap.parse_args()
    rng = random.Random(args.seed)

    print("M1 P(bad): closed form vs Monte-Carlo")
    print(f"  {'N':>6} {'mu':>4} {'F':>3} {'E_def':>8} {'pred':>8} "
          f"{'MC':>8} {'bad/trials':>12} {'z':>6}")
    for (N, mu, F, T) in PRESET:
        k = int(round(mu * N))
        p = M1Params(N=N, k=k, F=F)
        bad = sample_bad(p, T, rng)
        mc = bad / T
        pred = p.p_bad()
        se = math.sqrt(max(mc, 1 / T) * (1 - mc) / T)
        z = (mc - pred) / se if se > 0 else float("nan")
        print(f"  {N:>6} {mu:>4} {F:>3} {p.E_defects():>8.4f} {pred:>8.4f} "
              f"{mc:>8.4f} {bad:>6}/{T:<5} {z:>+6.2f}")

    print()
    N, mu = 20000, 0.2
    k = int(round(mu * N))
    print(f"Closed-form answer at N={N}, mu={mu}: smallest F with "
          f"P(bad) <= {args.target:g}")
    print(f"  {'F':>3} {'E_def':>10} {'P(bad)':>10}")
    best = None
    for F in range(18, 30):
        p = M1Params(N=N, k=k, F=F)
        pb = p.p_bad()
        print(f"  {F:>3} {p.E_defects():>10.3e} {pb:>10.3e}")
        if best is None and pb <= args.target:
            best = F
    print(f"  -> F = {best}")


if __name__ == "__main__":
    main()
