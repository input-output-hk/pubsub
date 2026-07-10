#!/usr/bin/env python3
"""M5 cost sweep: expected transmissions and hops to full coverage vs
(k_in, k_out), at N=20000, mu=0.2.  Backs
../properties/expected_number_of_messages.md and
../properties/expected_number_of_hops.md.

Rule: a node fires once on first receipt, relaying on every outgoing
propagation edge (its own k_out targets + the nodes that in-picked it),
skipping a resend back to its arrival node (which almost never coincides).
Transmissions = honest->honest sends (copies delivered to honest nodes, incl.
duplicates).  Hops = BFS depth from a single honest publisher.
"""

import argparse
import math
import random
from collections import deque

from m5_model import M5Params, M5Graph


def flood(adj, source):
    """One cascade from `source` over the directed honest adjacency.
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

    print(f"M5 cost vs (k_in, k_out)  --  N={N}, mu={mu} (H={H}), "
          f"{T} graphs/cell")
    print(f"  msgs pred = H*(k_in+k_out)*(1-mu);  "
          f"hop branching = (k_in+k_out)*(1-mu)")
    print(f"  {'k_in':>4} {'k_out':>5} {'msgs MC':>10} {'msgs pred':>10} "
          f"{'/node':>6} | {'hops(max)':>9} {'hops(mean)':>10} "
          f"{'ln H/ln br':>10} {'cov':>6}")
    for (a, b) in ((6, 6), (8, 8), (9, 8), (9, 9), (10, 10), (12, 12)):
        p = M5Params(N=N, k=k, k_in=a, k_out=b)
        sends = maxd = meand = cov = 0.0
        for _ in range(T):
            g = M5Graph(p, rng)
            s, mx, mn, r = flood(g.adj, 0)
            sends += s; maxd += mx; meand += mn; cov += r / H
        sends /= T; maxd /= T; meand /= T; cov /= T
        pred = H * (a + b) * (1 - mu)
        br = (a + b) * (1 - mu)
        lnbr = math.log(H) / math.log(br) if br > 1 else float("nan")
        print(f"  {a:>4} {b:>5} {sends:>10,.0f} {pred:>10,.0f} "
              f"{sends/H:>6.1f} | {maxd:>9.2f} {meand:>10.2f} "
              f"{lnbr:>10.2f} {cov:>6.4f}")


if __name__ == "__main__":
    main()
