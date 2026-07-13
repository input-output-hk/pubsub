#!/usr/bin/env python3
"""M5 coverage study: P(bad graph) vs the two-class isolated-vertex closed
form, and the "which (k_in, k_out)" answer at N=20000, mu=0.2.  Backs
../properties/full_coverage.md.

A sampled graph is bad iff the honest propagation digraph is not strongly
connected (some honest publisher cannot reach some honest node) -- dominated
by in-isolated (no honest in-edge) and out-isolated (no honest out-edge)
honest vertices.  Validation cells are chosen where P(bad) is measurable;
beyond, the validated closed form answers the design question.
"""

import argparse
import math
import random

from m5_model import M5Params, sample_bad


# (N, mu, k_in, k_out, trials) -- P(bad) from ~0.9 down to ~3e-3;
# the (3,6)/(6,3) pair exercises the exact swap symmetry.
PRESET = [
    (4000, 0.2, 4, 4, 500),
    (4000, 0.2, 5, 5, 2000),
    (4000, 0.2, 6, 6, 8000),
    (4000, 0.2, 3, 6, 1500),
    (4000, 0.2, 6, 3, 1500),
    (4000, 0.2, 2, 7, 1000),
    (20000, 0.2, 4, 4, 200),
    (20000, 0.2, 5, 5, 600),
]


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--seed", type=int, default=2024)
    ap.add_argument("--target", type=float, default=1e-4,
                    help="per-epoch P(bad) target for the closed-form answer")
    args = ap.parse_args()
    rng = random.Random(args.seed)

    print("M5 P(bad): closed form vs Monte-Carlo")
    print(f"  {'N':>6} {'mu':>4} {'k_in':>4} {'k_out':>5} {'E_def':>8} "
          f"{'pred':>8} {'MC':>8} {'bad/trials':>12} {'z':>6}")
    for (N, mu, a, b, T) in PRESET:
        k = int(round(mu * N))
        p = M5Params(N=N, k=k, k_in=a, k_out=b)
        bad = sample_bad(p, T, rng)
        mc = bad / T
        pred = p.p_bad()
        se = math.sqrt(max(mc, 1 / T) * (1 - mc) / T)
        z = (mc - pred) / se if se > 0 else float("nan")
        print(f"  {N:>6} {mu:>4} {a:>4} {b:>5} {p.E_defects():>8.4f} "
              f"{pred:>8.4f} {mc:>8.4f} {bad:>6}/{T:<5} {z:>+6.2f}")

    print()
    N, mu = 20000, 0.2
    k = int(round(mu * N))
    print(f"Closed-form answer at N={N}, mu={mu}: symmetric k_in = k_out = K")
    print(f"  {'K':>3} {'E_def':>10} {'P(bad)':>10}")
    for K in range(6, 12):
        p = M5Params(N=N, k=k, k_in=K, k_out=K)
        print(f"  {K:>3} {p.E_defects():>10.3e} {p.p_bad():>10.3e}")

    print()
    print(f"Best split per total budget B = k_in + k_out, and smallest B "
          f"with P(bad) <= {args.target:g}")
    print(f"  {'B':>3} {'best split':>11} {'P(bad)':>10}")
    best_B = None
    for B in range(14, 21):
        pb, a = min(((M5Params(N=N, k=k, k_in=a, k_out=B - a).p_bad(), a)
                     for a in range(1, B)))
        print(f"  {B:>3} {f'({a},{B-a})':>11} {pb:>10.3e}")
        if best_B is None and pb <= args.target:
            best_B = (B, a, B - a)
    print(f"  -> B = {best_B[0]}, (k_in, k_out) = ({best_B[1]}, {best_B[2]}) "
          f"(the balanced split is always optimal)")


if __name__ == "__main__":
    main()
