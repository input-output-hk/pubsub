#!/usr/bin/env python3
"""M4 full-coverage study (../properties/full_coverage.md).

Measures P(bad graph) = P(honest-induced subgraph disconnected) for the
undirected RF-out model, and compares to the isolated-vertex closed form
  P(bad) ~ 1 - exp(-H * C(k,RF)/C(N-1,RF) * (1 - RF/(N-1))^(H-1)).

Default use case: N = 20000, mu = 0.2 -- sweep RF, validate the formula in the
measurable regime, and report the smallest RF with P(bad) <= 1e-4.

  python3 sim_m4_coverage.py                 # the preset sweep (a few minutes)
  python3 sim_m4_coverage.py --rf 7 --trials 40000 --workers 4   # targeted run
"""

import argparse
import math
import os
import random
from concurrent.futures import ProcessPoolExecutor

from m4_model import M4Params, M4Graph

N_DEFAULT = 20000
MU_DEFAULT = 0.2


def _worker(args) -> int:
    N, k, RF, T, seed = args
    rng = random.Random(seed)
    p = M4Params(N=N, k=k, RF=RF)
    return sum(1 for _ in range(T) if M4Graph(p, rng).is_bad())


def measure(N: int, k: int, RF: int, trials: int, workers: int,
            seed: int) -> int:
    """Total # bad graphs over `trials`, split across `workers` processes."""
    if workers <= 1 or trials < 4 * workers:
        return _worker((N, k, RF, trials, seed))
    per = [trials // workers] * workers
    for i in range(trials - sum(per)):
        per[i] += 1
    jobs = [(N, k, RF, per[i], seed + 1000 * (i + 1)) for i in range(workers)]
    with ProcessPoolExecutor(max_workers=workers) as ex:
        return sum(ex.map(_worker, jobs))


def report_row(N, k, RF, trials, workers, seed):
    p = M4Params(N=N, k=k, RF=RF)
    pred = p.p_bad()
    bad = measure(N, k, RF, trials, workers, seed)
    mc = bad / trials
    se = math.sqrt(max(mc, 1 / trials) * (1 - min(mc, 1 - 1 / trials)) / trials)
    z = (mc - pred) / se if se > 0 else 0.0
    print(f"  {RF:>3} {p.E_isolated():>11.3e} {pred:>11.3e} "
          f"{mc:>11.3e} {bad:>7}/{trials:<8} {z:>+6.2f}")
    return pred, mc


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--N", type=int, default=N_DEFAULT)
    ap.add_argument("--mu", type=float, default=MU_DEFAULT)
    ap.add_argument("--rf", type=int, default=None,
                    help="single RF (targeted run); omit for the preset sweep")
    ap.add_argument("--trials", type=int, default=None)
    ap.add_argument("--workers", type=int, default=os.cpu_count() or 4)
    ap.add_argument("--seed", type=int, default=12345)
    args = ap.parse_args()
    N = args.N
    k = int(round(args.mu * N))
    H = N - k

    print(f"M4 P(bad graph)  --  N={N}, mu={args.mu} (k={k}, H={H}), "
          f"{args.workers} workers")
    print(f"  {'RF':>3} {'E_iso':>11} {'pred P(bad)':>11} {'MC P(bad)':>11} "
          f"{'bad/trials':>16} {'z':>6}")

    if args.rf is not None:
        report_row(N, k, args.rf, args.trials or 40000, args.workers, args.seed)
        return

    # preset sweep: trial budget scaled so each measurable cell sees events
    budget = {3: 2000, 4: 4000, 5: 8000, 6: 30000}
    for RF, T in budget.items():
        report_row(N, k, RF, T, args.workers, args.seed)

    # formula-only rows for the tail (too rare to measure cheaply)
    print("  -- formula only (tail too rare for the preset budget) --")
    for RF in (7, 8, 9):
        p = M4Params(N=N, k=k, RF=RF)
        print(f"  {RF:>3} {p.E_isolated():>11.3e} {p.p_bad():>11.3e} "
              f"{'--':>11} {'--':>16} {'--':>6}")

    # answer: smallest RF with predicted P(bad) <= 1e-4
    target = 1e-4
    rf = next(r for r in range(2, 40)
              if M4Params(N=N, k=k, RF=r).p_bad() <= target)
    print(f"\n  smallest RF with predicted P(bad) <= {target:.0e}:  RF = {rf}  "
          f"(P(bad) = {M4Params(N=N, k=k, RF=rf).p_bad():.2e}; "
          f"RF={rf-1} gives {M4Params(N=N, k=k, RF=rf-1).p_bad():.2e})")


if __name__ == "__main__":
    main()
