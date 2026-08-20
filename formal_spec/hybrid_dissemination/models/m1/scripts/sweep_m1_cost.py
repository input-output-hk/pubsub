#!/usr/bin/env python3
"""M1 cost sweep: expected transmissions and hops to full coverage vs F,
at N=20000, mu=0.2.  Backs ../properties/expected_number_of_messages.md and
../properties/expected_number_of_hops.md.

Rule (push cascade): a node fires once on first receipt, pushing to its F
targets (skipping a resend back to its arrival node, which almost never
coincides).  Transmissions = honest->honest sends (copies delivered to honest
nodes, incl. duplicates).  Hops = BFS depth from a single honest source.
"""

import argparse
import math
import random
from collections import deque

from m1_model import M1Params, M1Graph


def flood(adj, source):
    """One cascade from `source` over directed honest adjacency `adj`.
    Returns (total sends, max depth, mean depth, reached)."""
    n = len(adj)
    depth = [-1] * n
    parent = [-1] * n
    depth[source] = 0
    dq = deque([source])
    order = []
    while dq:
        v = dq.popleft()
        order.append(v)
        dv1 = depth[v] + 1
        for w in adj[v]:
            if depth[w] < 0:
                depth[w] = dv1
                parent[w] = v
                dq.append(w)
    sends = 0
    for v in order:
        av = parent[v]
        for w in adj[v]:
            if w != av:                    # don't resend back on the arrival link
                sends += 1
    reached = [depth[v] for v in order]
    return sends, max(reached), sum(reached) / len(reached), len(reached)


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--N", type=int, default=20000)
    ap.add_argument("--mu", type=float, default=0.2)
    ap.add_argument("--trials", type=int, default=40)
    ap.add_argument("--seed", type=int, default=2024)
    args = ap.parse_args()
    N, mu, T = args.N, args.mu, args.trials
    k = int(round(mu * N)); H = N - k
    rng = random.Random(args.seed)

    print(f"M1 cost vs F  --  N={N}, mu={mu} (H={H}), {T} graphs/cell")
    print(f"  msgs pred = H*F*(1-mu);  hop branching = F*(1-mu)")
    print(f"  {'F':>3} {'msgs MC':>10} {'msgs pred':>10} {'/node':>6} | "
          f"{'hops(max)':>9} {'hops(mean)':>10} {'ln H/ln br':>10} {'cov':>6}")
    for F in (12, 16, 20, 24, 25, 28):
        p = M1Params(N=N, k=k, F=F)
        sends = maxd = meand = cov = 0.0
        for _ in range(T):
            g = M1Graph(p, rng)
            s, mx, mn, r = flood(g.adj, 0)
            sends += s; maxd += mx; meand += mn; cov += r / H
        sends /= T; maxd /= T; meand /= T; cov /= T
        pred = H * F * (1 - mu)
        br = F * (1 - mu)
        lnbr = math.log(H) / math.log(br) if br > 1 else float("nan")
        print(f"  {F:>3} {sends:>10,.0f} {pred:>10,.0f} {sends/H:>6.1f} | "
              f"{maxd:>9.2f} {meand:>10.2f} {lnbr:>10.2f} {cov:>6.4f}")


if __name__ == "__main__":
    main()
