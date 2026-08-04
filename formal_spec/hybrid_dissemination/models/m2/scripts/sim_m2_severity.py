#!/usr/bin/env python3
"""M2 failure severity: size and type of the stranded set on bad graphs.

Conditional on a sampled graph being bad (honest pull digraph not strongly
connected), measures d = H - |giant SCC| and classifies the stranded
nodes: deaf (not reachable from the giant: eclipsed), mute (cannot reach
the giant: no requester path), cut (neither).  Sampled at elevated mu
where bad graphs are collectable; at the operating point the defect
intensity E ~ 1e-4 makes simultaneous defects vanishingly rare, so the
per-defect sizes measured here bound the op-point severity.

Usage: python3 sim_m2_severity.py [--seed SEED]
"""

from __future__ import annotations

import argparse
import math
import os
import random
import sys
from collections import Counter, deque

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _HERE)
sys.path.insert(0, os.path.join(_HERE, "..", "..", "m3", "scripts"))

from m2_model import M2Params            # noqa: E402
from m3_model import M3Graph, rho_giant, u_iterate  # noqa: E402

N = 20_000
RF = 24
CELLS = [(0.55, 700), (0.60, 350)]   # (mu_eff, trials)


def p_bad(mu: float) -> float:
    H = N - int(round(mu * N))
    return 1 - math.exp(-H * ((1 - rho_giant(RF * (1 - mu)))
                              + u_iterate(mu, RF)))


def bfs_set(adj, starts, H):
    seen = bytearray(H)
    dq = deque()
    for s in starts:
        if not seen[s]:
            seen[s] = 1
            dq.append(s)
    while dq:
        for w in adj[dq.popleft()]:
            if not seen[w]:
                seen[w] = 1
                dq.append(w)
    return seen


def severity(g, H):
    """(d, deaf, mute, cut) via giant-SCC classification (d = 0: good)."""
    adj = g.adjacency()                  # forwarder -> requester, honest only
    radj = [[] for _ in range(H)]
    for v in range(H):
        for w in adj[v]:
            radj[w].append(v)
    for v in range(min(8, H)):
        fwd = bfs_set(adj, [v], H)
        rev = bfs_set(radj, [v], H)
        if sum(a & b for a, b in zip(fwd, rev)) >= H // 2:
            break
    else:
        return None                              # no giant SCC (never seen)
    deaf = mute = cut = 0
    for w in range(H):
        r, a = fwd[w], rev[w]
        if r and a:
            continue
        if a:
            deaf += 1
        elif r:
            mute += 1
        else:
            cut += 1
    return (deaf + mute + cut, deaf, mute, cut)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--seed", type=int, default=20260721)
    args = ap.parse_args()
    rng = random.Random(args.seed)

    print(f"M2 failure severity -- frozen RF = {RF}, N = {N}")
    for mu, T in CELLS:
        k = int(round(mu * N))
        params = M2Params(N=N, k=k, RF=RF)
        pred = p_bad(mu)
        dist: Counter = Counter()
        deaf = mute = cut = 0
        for _ in range(T):
            s = severity(M3Graph(params, rng), N - k)
            if s is None:
                print("  WARNING: no giant SCC found")
                continue
            d, de, mu_, cu = s
            if d:
                dist[d] += 1
                deaf += de
                mute += mu_
                cut += cu
        bad = sum(dist.values())
        mc = bad / T
        se = math.sqrt(max(mc, 1 / T) * (1 - mc) / T)
        z = (mc - pred) / se
        print(f"  mu_eff={mu:.2f} trials={T} bad={bad} MC={mc:.3f} "
              f"law={pred:.3f} z={z:+.1f}")
        ds = "  ".join(f"{d}x{c}" for d, c in sorted(dist.items()))
        print(f"    d distribution: {ds}   (max {max(dist) if dist else 0})")
        print(f"    stranded-node classes: deaf {deaf}, mute {mute}, "
              f"cut {cut}")


if __name__ == "__main__":
    main()
