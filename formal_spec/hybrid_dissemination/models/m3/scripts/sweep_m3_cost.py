#!/usr/bin/env python3
"""M3 cost sweep: expected transmissions and hops to full coverage at the
operating points (RF x s grid), N=20000, mu=0.2.  Backs
../properties/expected_number_of_messages.md (and cross-checks p06's depths).

Rule: the publisher seeds via its s-1 initiation links (those sends
counted when the target is honest, arriving at depth 1), then every node
fires once on first receipt, relaying to its honest requesters over the
directed pull edges (forwarder->requester), skipping a resend back to its
arrival node.  Transmissions = honest->honest sends.
"""

import argparse
import math
import os
import random
import sys
from collections import deque

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__))))
from m3_model import M3Graph  # noqa: E402
from m2_model import M2Params  # noqa: E402


def flood_seeded(adj, seed_depths, H):
    """Cascade from pre-seeded (node, depth) pairs.  Returns
    (relay sends, max depth, mean depth, reached honest)."""
    depth = [-1] * len(adj)
    parent = [-1] * len(adj)
    dq = deque()
    for v, d in seed_depths:
        if depth[v] < 0:
            depth[v] = d
            dq.append(v)
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


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--N", type=int, default=20000)
    ap.add_argument("--mu", type=float, default=0.2)
    ap.add_argument("--s", type=int, default=8)
    ap.add_argument("--trials", type=int, default=40)
    ap.add_argument("--seed", type=int, default=2024)
    args = ap.parse_args()
    N, mu, s, T = args.N, args.mu, args.s, args.trials
    k = int(round(mu * N)); H = N - k
    rng = random.Random(args.seed)

    print(f"M3 cost vs RF  --  N={N}, mu={mu} (H={H}), s={s}, {T} graphs/cell")
    print(f"  msgs pred = H*RF*(1-mu) + (s-1)*(1-mu);  hop branching = RF*(1-mu)")
    print(f"  {'RF':>3} {'msgs MC':>10} {'msgs pred':>10} {'/node':>6} | "
          f"{'hops(max)':>9} {'hops(mean)':>10} {'ln H/ln br':>10} {'cov':>6}")
    for RF in (8, 12, 13, 16):
        p = M2Params(N=N, k=k, RF=RF)
        sends = maxd = meand = cov = 0.0
        for _ in range(T):
            g = M3Graph(p, rng)
            adj = g.adjacency()
            seeds = [(0, 0)]               # publisher (first regular node)
            push_sends = 0
            for r in rng.sample(range(N - 1), s - 1):
                t = r + 1                  # skip the publisher itself (node 0)
                if not g.is_adversarial(t):
                    push_sends += 1        # an initiation copy an honest node got
                    seeds.append((t, 1))
            sd, mx, mn, r_ = flood_seeded(adj, seeds, H)
            sends += sd + push_sends
            maxd += mx; meand += mn; cov += r_ / H
        sends /= T; maxd /= T; meand /= T; cov /= T
        pred = H * RF * (1 - mu) + (s - 1) * (1 - mu)
        br = RF * (1 - mu)
        lnbr = math.log(H) / math.log(br)
        print(f"  {RF:>3} {sends:>10,.0f} {pred:>10,.0f} {sends/H:>6.1f} | "
              f"{maxd:>9.2f} {meand:>10.2f} {lnbr:>10.2f} {cov:>6.4f}")


if __name__ == "__main__":
    main()
