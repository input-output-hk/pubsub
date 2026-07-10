#!/usr/bin/env python3
"""Latency comparison: hops until full coverage, M3 (directed pull + initiation links)
vs M4 (undirected flood), at their P(bad)=1e-4 operating points for N=20000,
mu=0.2.

Hop distance = number of forwarding rounds from the source to a node (BFS depth
on the honest propagation graph).  We report the FULL-COVERAGE depth (max over
honest nodes = when the last node receives) and the mean (typical node), over
graphs that fully cover.

  M3: publisher at depth 0; its s-1 initiation targets receive at depth 1;
      relaying then follows the directed honest pull edges (forwarder->requester).
  M4: single honest source at depth 0; flooding along undirected honest edges.
"""

import os
import random
import sys
from collections import deque

from m4_model import M4Params, M4Graph

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                "..", "..", "m3", "scripts"))
from m3_model import M3Graph  # noqa: E402
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                "..", "..", "m2", "scripts"))
from m2_model import M2Params  # noqa: E402


def bfs_depths(adj, seeded_depths, H):
    """BFS over adj from pre-seeded (node, depth) pairs; return (max, mean, cov)
    over reached honest nodes [0, H)."""
    depth = [-1] * len(adj)
    dq = deque()
    for v, d in seeded_depths:
        if depth[v] < 0:
            depth[v] = d
            dq.append(v)
    while dq:
        v = dq.popleft()
        dv = depth[v] + 1
        for w in adj[v]:
            if depth[w] < 0:
                depth[w] = dv
                dq.append(w)
    ds = [depth[v] for v in range(H) if depth[v] >= 0]
    return max(ds), sum(ds) / len(ds), len(ds)


def m4_depth(params, rng):
    g = M4Graph(params, rng)
    return bfs_depths(g.adj, [(0, 0)], params.H)


def m3_depth(params, s, rng):
    g = M3Graph(params, rng)
    adj = g.adjacency()
    seeds = [(0, 0)]                             # publisher at depth 0
    for r in rng.sample(range(params.N - 1), s - 1):
        if not g.is_adversarial(r + 1):
            seeds.append((r + 1, 1))             # initiation send arrives at depth 1
    return bfs_depths(adj, seeds, params.H)


def main():
    N, mu, trials = 20000, 0.2, 200
    k = int(round(mu * N)); H = N - k
    rng = random.Random(2024)

    print(f"Hops until full coverage  --  N={N}, mu={mu} (H={H}), "
          f"{trials} graphs each")
    print()

    p3, s = M2Params(N=N, k=k, RF=11), 3
    r3 = [m3_depth(p3, s, rng) for _ in range(trials)]
    good3 = [x for x in r3 if x[2] == H]

    p4 = M4Params(N=N, k=k, RF=8)
    r4 = [m4_depth(p4, rng) for _ in range(trials)]
    good4 = [x for x in r4 if x[2] == H]

    def avg(xs, i):
        return sum(x[i] for x in xs) / len(xs)

    print(f"  {'model':<24} {'full-coverage hops':>19} {'mean-node hops':>15} "
          f"{'branching':>10}")
    print(f"  {'M3  pull RF=11, s=3':<24} {avg(good3, 0):>19.2f} "
          f"{avg(good3, 1):>15.2f} {11*(1-mu):>10.1f}")
    print(f"  {'M4  flood RF=8':<24} {avg(good4, 0):>19.2f} "
          f"{avg(good4, 1):>15.2f} {2*8*(1-mu)-1:>10.1f}")
    print()
    print(f"  (full-coverage hops ~ ln H / ln(branching): "
          f"M3 {__import__('math').log(H)/__import__('math').log(11*(1-mu)):.1f}, "
          f"M4 {__import__('math').log(H)/__import__('math').log(2*8*(1-mu)-1):.1f} + tail)")


if __name__ == "__main__":
    main()
