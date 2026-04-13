#!/usr/bin/env python3
"""
Monte Carlo estimation of RandCast partition probability.

RandCast: N nodes, each with RF directed outgoing r-links (no self-loops,
sampled without replacement from the other N-1 nodes). Source = node 0.

P(partition) = P(some node unreachable from source via directed paths).

No adversary — this measures the natural connectivity fragility of the pure
random topology, for comparison with RingCast under adversarial attack
(see adversarial_partition_report.md).

Usage:
    # Print default table (N = 6..200, RF = 1..3)
    python randcast_partition_mc.py

    # Single configuration
    python randcast_partition_mc.py --N 50 --RF 2 --trials 500000

    # Reproducible run
    python randcast_partition_mc.py --seed 42
"""

import argparse
import math
import random
from collections import deque

# Threshold formula: P(partition) ≈ 1 − exp(−exp(−c))  where c = RF − ln(N).
# Derivation: at RF = ln(N) + c, expected number of in-degree-0 nodes → exp(−c),
# and their count is asymptotically Poisson, so P(none) → exp(−exp(−c)).
THRESHOLD_FORMULA = lambda c: 1.0 - math.exp(-math.exp(-c))


# ---------------------------------------------------------------------------
# Core primitives
# ---------------------------------------------------------------------------

def sample_graph(N, RF):
    """
    Sample one random directed graph: each node i picks RF targets uniformly
    at random without replacement from {0..N-1} \\ {i}.
    Returns adjacency list (list of lists).
    """
    adj = []
    for i in range(N):
        pool = list(range(i)) + list(range(i + 1, N))
        adj.append(random.sample(pool, RF))
    return adj


def is_partitioned(adj, N):
    """
    Return True if some node is unreachable from node 0 in directed graph adj.
    """
    reached = bytearray(N)
    reached[0] = 1
    count = 1
    queue = deque([0])
    while queue:
        node = queue.popleft()
        for t in adj[node]:
            if not reached[t]:
                reached[t] = 1
                count += 1
                queue.append(t)
    return count < N


def estimate(N, RF, trials):
    """
    Estimate P(partition) for RandCast(N, RF) using Monte Carlo.
    Returns (probability, half-width of 95% CI).
    """
    hits = sum(is_partitioned(sample_graph(N, RF), N) for _ in range(trials))
    p = hits / trials
    ci = 1.96 * math.sqrt(p * (1.0 - p) / trials) if 0 < p < 1 else 0.0
    return p, ci


# ---------------------------------------------------------------------------
# Output helpers
# ---------------------------------------------------------------------------

def fmt(p, ci):
    return f"{p:.4f} ± {ci:.4f}"


def print_threshold_table(ns, trials):
    """
    For each N, run at RF = ceil(ln(N)) — the connectivity threshold —
    and compare the simulated P(partition) with the asymptotic formula
    P(partition) ≈ 1 − exp(−exp(−c)) where c = RF − ln(N).
    """
    print(f"{'N':>6}  {'RF':>4}  {'ln(N)':>7}  {'c':>7}  "
          f"{'P(partition)':>18}  {'formula':>10}")
    print("-" * 66)
    for N in ns:
        t = 100_000 if N <= 200 else 20_000
        t = min(t, trials)
        RF  = math.ceil(math.log(N))
        lnN = math.log(N)
        c   = RF - lnN
        pred = THRESHOLD_FORMULA(c)
        p, ci = estimate(N, RF, t)
        print(f"{N:>6}  {RF:>4}  {lnN:>7.3f}  {c:>7.3f}  "
              f"{fmt(p, ci):>18}  {pred:>10.4f}", flush=True)
    print()
    print("formula: P(partition) ≈ 1 − exp(−exp(−c)),  c = RF − ln(N)")


def print_table(ns, rfs, trials):
    col = 20
    header = f"{'N':>6}" + "".join(f"{'RF=' + str(rf):>{col}}" for rf in rfs)
    print(header)
    print("-" * len(header))
    for N in ns:
        row = f"{N:>6}"
        for RF in rfs:
            p, ci = estimate(N, RF, trials)
            row += f"{fmt(p, ci):>{col}}"
        print(row, flush=True)

    print()
    # Reference: PRISM exact values at N=6
    print("PRISM exact (N=6):  RF=1: 0.9616   RF=2: 0.3736")
    # Reference: RingCast adversarial floor (k=2, large N)
    print("RingCast adv. floor (k=2, N→∞):  "
          + "   ".join(f"RF={rf}: {math.exp(-rf):.4f}" for rf in rfs))


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(
        description="Monte Carlo estimation of RandCast partition probability"
    )
    parser.add_argument("--N",      type=int, default=None,
                        help="Single N (default: run comparison table)")
    parser.add_argument("--RF",     type=int, default=None,
                        help="Single RF (default: run comparison table)")
    parser.add_argument("--trials", type=int, default=100_000,
                        help="Monte Carlo trials per configuration (default: 100000)")
    parser.add_argument("--seed",      type=int, default=None,
                        help="Random seed for reproducibility")
    parser.add_argument("--threshold", action="store_true",
                        help="Run threshold experiment: RF=ceil(ln(N)) for N=6..1000")
    args = parser.parse_args()

    if args.seed is not None:
        random.seed(args.seed)

    print(f"RandCast partition probability — Monte Carlo")
    print(f"Trials per configuration: {args.trials:,}")
    print()

    if args.N is not None and args.RF is not None:
        p, ci = estimate(args.N, args.RF, args.trials)
        print(f"N={args.N}, RF={args.RF}: P(partition) = {fmt(p, ci)} (95% CI)")
    elif args.threshold:
        ns = [6, 10, 20, 50, 100, 200, 500, 1000]
        print_threshold_table(ns, args.trials)
    else:
        ns  = [6, 10, 20, 50, 100, 200]
        rfs = [1, 2, 3]
        print_table(ns, rfs, args.trials)


if __name__ == "__main__":
    main()
