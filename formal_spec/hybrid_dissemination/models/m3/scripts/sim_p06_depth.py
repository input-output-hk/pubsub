#!/usr/bin/env python3
"""Property #6 -- delivery depth (latency in hops) of M3, by simulation.

M3: pull relaying (RF forwarders per node, per epoch) + initiation-link seeding
(publisher pushes to s-1 uniform targets at publication).  Hop accounting:
publisher holds the message at depth 0; initiation targets receive at depth
1; every pull relay hop costs 1.

Per (RF, mu, s) cell, over ignited runs (coverage >= 50%):
    d_med  mean depth of the median covered node   (typical delivery)
    d_99   mean depth by which 99% of covered nodes are reached
    d_max  mean depth of the last covered node     (full-coverage latency)
    ref    branching reference ln(H)/ln(RF*(1-mu)) (typical-distance scale)

No closed form exists (#6 is SIMULATION ONLY); the log reference is a sanity
scale, not a prediction.

Usage: python3 sim_p06_depth.py [--trials T] [--seed S]
"""

import argparse
import math
import random
from collections import deque

import os, sys
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                 "..", "..", "m2", "scripts"))
from m2_model import M2Params
from m3_model import M3Graph

N = 20000


def depth_stats(g: M3Graph, s: int, rng: random.Random):
    """(covered fraction, d_med, d_99, d_max) for one message; push = 1 hop."""
    pub = 0                                 # first regular honest node
    depth = [-1] * g.params.N
    depth[pub] = 0
    dq = deque([pub])
    for r in rng.sample(range(N - 1), s - 1):
        t = r + 1 if r >= pub else r        # skip publisher
        if not g.is_adversarial(t) and depth[t] < 0:
            depth[t] = 1                    # initiation send costs one hop
            dq.append(t)
    adj = g.adjacency()
    # BFS honouring heterogeneous start depths (0 and 1): a deque suffices
    # because push targets are enqueued after the publisher and depths differ
    # by at most one (monotone non-decreasing pop order).
    while dq:
        v = dq.popleft()
        dv = depth[v] + 1
        for w in adj[v]:
            if depth[w] < 0:
                depth[w] = dv
                dq.append(w)
    ds = sorted(depth[j] for j in g.regular_nodes() if depth[j] >= 0)
    if not ds:
        return 0.0, 0, 0, 0
    cov = len(ds) / g.params.H
    return (cov, ds[len(ds) // 2], ds[math.ceil(0.99 * len(ds)) - 1], ds[-1])


def run_cell(RF: int, mu: float, s: int, trials: int,
             rng: random.Random) -> str:
    k = int(round(mu * N))
    params = M2Params(N=N, k=k, RF=RF)
    med = p99 = mx = 0.0
    ignited = 0
    for _ in range(trials):
        g = M3Graph(params, rng)
        cov, d_med, d_99, d_max = depth_stats(g, s, rng)
        if cov >= 0.5:
            ignited += 1
            med += d_med
            p99 += d_99
            mx += d_max
    if ignited == 0:
        return "      dead      "
    return (f" {med / ignited:4.1f} /{p99 / ignited:5.1f} /"
            f"{mx / ignited:5.1f} ")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--trials", type=int, default=60)
    ap.add_argument("--seed", type=int, default=12345)
    args = ap.parse_args()
    rng = random.Random(args.seed)
    T = args.trials

    print(f"M3 delivery depth (hops)  --  N={N}, {T} trials/cell")
    print(f"cell = mean over ignited runs of: median-node / 99%-coverage / "
          f"full-coverage depth")

    print()
    print(f"(A) RF x mu at s = 3")
    mus = (0.0, 0.1, 0.2)
    print(f"{'RF':>4} |" + "".join(f" {'mu='+format(m,'.1f'):>17}" for m in mus)
          + f" | {'ref ln H/ln m':>14}")
    for RF in (5, 8, 11, 16):
        cells = [run_cell(RF, mu, 3, T, rng) for mu in mus]
        refs = "/".join(f"{math.log(N - round(mu*N)) / math.log(RF*(1-mu)):.1f}"
                        for mu in mus)
        print(f"{RF:>4} |" + "".join(f" {c:>17}" for c in cells)
              + f" | {refs:>14}")

    print()
    print(f"(B) seed count at the operating point RF = 11, mu = 0.2")
    print(f"{'s':>4} | {'med / d99 / dmax':>20}")
    for s in (1, 3, 10, 50):
        print(f"{s:>4} | {run_cell(11, 0.2, s, T, rng):>20}")


if __name__ == "__main__":
    main()
