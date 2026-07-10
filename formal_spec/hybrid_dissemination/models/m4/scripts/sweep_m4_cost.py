#!/usr/bin/env python3
"""M4 cost sweep: expected transmissions and hops to full coverage vs RF, at
N=20000, mu=0.2.  Backs ../properties/expected_number_of_messages.md and
../properties/expected_number_of_hops.md.

Rule (flood): a node fires once on first receipt, sending over every incident
honest link except the arrival link.  Transmissions = honest->honest sends
(copies delivered to honest nodes, incl. duplicates).  Hops = BFS depth on the
honest subgraph from a single honest source.
"""

import argparse
import math
import random
from collections import deque

from m4_model import M4Params, M4Graph


def flood(adj, source):
    """One flood from `source` over undirected honest adjacency `adj`.
    Returns (total sends, max depth, mean depth, reached).

    A node fires once and sends to every neighbour except the single node it
    first received from (its BFS parent = the arrival link)."""
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
    ap.add_argument("--trials", type=int, default=60)
    ap.add_argument("--seed", type=int, default=2024)
    args = ap.parse_args()
    N, mu, T = args.N, args.mu, args.trials
    k = int(round(mu * N)); H = N - k
    rng = random.Random(args.seed)

    print(f"M4 cost vs RF  --  N={N}, mu={mu} (H={H}), {T} graphs/cell")
    print(f"  msgs pred = 2*H*RF*(1-mu) - (H-1);  hop branching = 2*RF*(1-mu)-1")
    print(f"  {'RF':>3} {'msgs MC':>10} {'msgs pred':>10} {'/node':>6} | "
          f"{'hops(max)':>9} {'hops(mean)':>10} {'ln H/ln br':>10} {'cov':>6}")
    for RF in (4, 5, 6, 7, 8, 9, 10, 12):
        p = M4Params(N=N, k=k, RF=RF)
        sends = maxd = meand = cov = 0.0
        for _ in range(T):
            g = M4Graph(p, rng)
            s, mx, mn, r = flood(g.adj, 0)
            sends += s; maxd += mx; meand += mn; cov += r / H
        sends /= T; maxd /= T; meand /= T; cov /= T
        pred = 2 * H * RF * (1 - mu) - (H - 1)
        br = 2 * RF * (1 - mu) - 1
        lnbr = math.log(H) / math.log(br) if br > 1 else float("nan")
        print(f"  {RF:>3} {sends:>10,.0f} {pred:>10,.0f} {sends/H:>6.1f} | "
              f"{maxd:>9.2f} {meand:>10.2f} {lnbr:>10.2f} {cov:>6.4f}")


if __name__ == "__main__":
    main()
