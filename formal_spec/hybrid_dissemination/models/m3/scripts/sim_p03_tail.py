#!/usr/bin/env python3
"""Deep-tail validation of the recommended M3 operating point
(../properties/full_coverage.md, secondary metric): N=20000, mu=0.2, RF=11, s=3.

The per-graph failure probability is predicted at ~3.3e-4, decomposed as

  floor     P(some node's RF picks all adversarial) = 1-(1-q_pull)^H   [EXACT:
            pull picks are independent across nodes; q_pull hypergeometric]
  ignition  (1-rho_f) * (1-(1-mu)*rho_f)^(s-1)                  [branching]
  other     amplification / partial-spread corrections           [~1e-6]

Coarse MC (400 trials) cannot resolve this; here we run tens of thousands of
full-BFS graphs in parallel and classify every failure by cause:
  - floor event:    unreached node(s) whose picks are all adversarial
  - ignition event: essentially the whole network unreached (message never
                    escaped the seeds)
  - other:          unreached nodes with at least one honest pick

Usage: python3 sim_p03_tail.py [--trials 50000] [--seed S] [--workers W]
"""

import argparse
import math
import os
import random
from concurrent.futures import ProcessPoolExecutor

import os, sys
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                 "..", "..", "m2", "scripts"))
from m2_model import M2Params
from m3_model import M3Graph, rho_giant

N, MU, RF, S = 20000, 0.2, 11, 3
K = int(MU * N)
PARAMS = M2Params(N=N, k=K, RF=RF)


def run_chunk(args):
    """(seed, trials) -> (trials, [(n_unreached, n_floor) per bad graph])."""
    seed, trials = args
    rng = random.Random(seed)
    events = []
    for _ in range(trials):
        g = M3Graph(PARAMS, rng)
        seeds = [0]                                  # publisher (honest)
        for r in rng.sample(range(N - 1), S - 1):    # initiation targets
            t = r + 1                                # shift skips publisher 0
            if not g.is_adversarial(t):
                seeds.append(t)
        depth = g.depths(seeds=seeds)
        unreached = [j for j in g.regular_nodes() if depth[j] < 0]
        if unreached:
            n_floor = sum(1 for j in unreached if g.pull_failed(j))
            events.append((len(unreached), n_floor))
    return trials, events


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--trials", type=int, default=50000)
    ap.add_argument("--seed", type=int, default=12345)
    ap.add_argument("--workers", type=int, default=max(1, os.cpu_count() - 2))
    args = ap.parse_args()

    # predictions
    q_pull = PARAMS.q_pull()
    p_floor = 1 - (1 - q_pull) ** PARAMS.H          # exact
    rho_f = rho_giant(RF * (1 - MU))
    p_ign = (1 - rho_f) * (1 - (1 - MU) * rho_f) ** (S - 1)
    print(f"Operating point: N={N} mu={MU} RF={RF} s={S}  "
          f"({args.trials} graphs, {args.workers} workers)")
    print(f"  predicted: floor {p_floor:.3e} (exact)  +  ignition "
          f"{p_ign:.3e} (branching)  =  {p_floor + p_ign:.3e}")

    chunks = 4 * args.workers
    per = args.trials // chunks
    jobs = [(args.seed + i, per) for i in range(chunks)]
    jobs[-1] = (jobs[-1][0], per + args.trials - per * chunks)

    total = 0
    events = []
    with ProcessPoolExecutor(max_workers=args.workers) as ex:
        for t, ev in ex.map(run_chunk, jobs):
            total += t
            events.extend(ev)

    bad = len(events)
    p_mc = bad / total
    se = math.sqrt(max(bad, 1)) / total              # Poisson SE
    ign_events = [e for e in events if e[0] > PARAMS.H // 2]
    floor_events = [e for e in events if e[0] <= PARAMS.H // 2
                    and e[1] == e[0]]
    other_events = [e for e in events if e[0] <= PARAMS.H // 2
                    and e[1] < e[0]]
    print(f"  measured:  {bad} bad graphs / {total}  ->  P(bad) = "
          f"{p_mc:.3e}  (+/- {se:.1e} 1-sigma)")
    print(f"  breakdown: floor {len(floor_events)} "
          f"(sizes {sorted(e[0] for e in floor_events)}), "
          f"ignition {len(ign_events)}, other {len(other_events)}"
          + (f" (sizes {sorted(e[0] for e in other_events)})"
             if other_events else ""))
    z = (p_mc - (p_floor + p_ign)) / se
    print(f"  z vs prediction: {z:+.2f}")


if __name__ == "__main__":
    main()
