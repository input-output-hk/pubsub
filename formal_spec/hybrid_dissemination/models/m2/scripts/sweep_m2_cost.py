#!/usr/bin/env python3
"""M2 cost sweep: expected transmissions and hops to full coverage vs
RF, at N=20000, mu=0.2 -- pure pull, no initiation links (the publisher's own
serving set is the only injection path).  Backs
../properties/expected_number_of_messages.md and
../properties/expected_number_of_hops.md.

Rule: a node fires once on first receipt, relaying to its honest requesters
(directed pull edges forwarder->requester), skipping a resend back to its
arrival node (which almost never coincides).  Transmissions = honest->honest
sends.  Hops = BFS depth from the publisher.

`--coverage` additionally validates the M2 good-graph law
P_good ~ exp(-H*[(1-rho_f)+u]) (strong connectivity of the honest pull
digraph -- see ../properties/full_coverage.md) in a measurable small-N
regime.
"""

import argparse
import math
import os
import random
import sys
from collections import deque

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                "..", "..", "m3", "scripts"))
from m3_model import M3Graph, rho_giant, u_iterate  # noqa: E402
from m2_model import M2Params  # noqa: E402


def flood(adj, source, H):
    """One relay cascade from `source` over directed pull adjacency.
    Returns (total sends, max depth, mean depth, reached honest)."""
    depth = [-1] * len(adj)
    parent = [-1] * len(adj)
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
    ds = [depth[v] for v in range(H) if depth[v] >= 0]
    return sends, max(ds), sum(ds) / len(ds), len(ds)


def strongly_connected(adj, H):
    """True iff the honest pull digraph is strongly connected (== every honest
    publisher reaches every honest node): forward and reverse BFS from node 0
    both cover all H honest nodes."""
    radj = [[] for _ in range(len(adj))]
    for v in range(H):
        for w in adj[v]:
            radj[w].append(v)
    for a in (adj, radj):
        seen = bytearray(len(adj))
        seen[0] = 1
        n = 1
        dq = deque([0])
        while dq:
            for w in a[dq.popleft()]:
                if not seen[w]:
                    seen[w] = 1
                    n += 1
                    dq.append(w)
        if n < H:
            return False
    return True


def coverage_ladder(seed):
    """Validate P_bad = 1 - exp(-H*[(1-rho_f)+u]) where it is measurable."""
    rng = random.Random(seed)
    print("M2 P(bad graph) -- any-publisher (strong connectivity) law")
    print(f"  {'N':>6} {'mu':>4} {'RF':>3} {'pred':>8} {'MC':>8} "
          f"{'bad/trials':>12} {'z':>6}")
    for (N, mu, RF, T) in [(4000, 0.2, 12, 1000), (4000, 0.2, 14, 2000),
                           (4000, 0.2, 16, 8000)]:
        k = int(round(mu * N)); H = N - k
        p = M2Params(N=N, k=k, RF=RF)
        bad = 0
        for _ in range(T):
            g = M3Graph(p, rng)
            if not strongly_connected(g.adjacency(), H):
                bad += 1
        mc = bad / T
        rho = rho_giant(RF * (1 - mu))
        u = u_iterate(mu, RF)
        pred = 1 - math.exp(-H * ((1 - rho) + u))
        se = math.sqrt(max(mc, 1 / T) * (1 - mc) / T)
        z = (mc - pred) / se if se > 0 else float("nan")
        print(f"  {N:>6} {mu:>4} {RF:>3} {pred:>8.4f} {mc:>8.4f} "
              f"{bad:>6}/{T:<5} {z:>+6.2f}")


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--N", type=int, default=20000)
    ap.add_argument("--mu", type=float, default=0.2)
    ap.add_argument("--trials", type=int, default=40)
    ap.add_argument("--seed", type=int, default=2024)
    ap.add_argument("--coverage", action="store_true",
                    help="also run the small-N good-graph law validation")
    args = ap.parse_args()
    N, mu, T = args.N, args.mu, args.trials
    k = int(round(mu * N)); H = N - k
    rng = random.Random(args.seed)

    print(f"M2 cost vs RF  --  N={N}, mu={mu} (H={H}), {T} graphs/cell, "
          f"publisher-only injection")
    print(f"  msgs pred = H*RF*(1-mu);  hop branching = RF*(1-mu)")
    print(f"  {'RF':>3} {'msgs MC':>10} {'msgs pred':>10} {'/node':>6} | "
          f"{'hops(max)':>9} {'hops(mean)':>10} {'ln H/ln br':>10} {'cov':>6}")
    for RF in (16, 20, 24, 25):
        p = M2Params(N=N, k=k, RF=RF)
        sends = maxd = meand = cov = 0.0
        for _ in range(T):
            g = M3Graph(p, rng)
            s, mx, mn, r = flood(g.adjacency(), 0, H)
            sends += s; maxd += mx; meand += mn; cov += r / H
        sends /= T; maxd /= T; meand /= T; cov /= T
        pred = H * RF * (1 - mu)
        br = RF * (1 - mu)
        lnbr = math.log(H) / math.log(br)
        print(f"  {RF:>3} {sends:>10,.0f} {pred:>10,.0f} {sends/H:>6.1f} | "
              f"{maxd:>9.2f} {meand:>10.2f} {lnbr:>10.2f} {cov:>6.4f}")

    if args.coverage:
        print()
        coverage_ladder(args.seed + 1)


if __name__ == "__main__":
    main()
