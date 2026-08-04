#!/usr/bin/env python3
"""M4 failure severity: size of the stranded set on bad graphs.

Conditional on a sampled graph being bad (honest-induced subgraph
disconnected), measures d = H - |largest component| and the sizes of the
straggler components (links are bidirectional, so stranded nodes are
symmetric: cut off in both directions).  Sampled at elevated mu where bad
graphs are collectable; at the operating point the defect intensity
E ~ 1e-4 makes simultaneous defects vanishingly rare, so the per-defect
sizes measured here bound the op-point severity.

Usage: python3 sim_m4_severity.py [--seed SEED]
"""

from __future__ import annotations

import argparse
import math
import os
import random
import sys
from collections import Counter, deque

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from m4_model import M4Graph, M4Params  # noqa: E402

N = 20_000
RF = 8
CELLS = [(0.45, 600), (0.50, 300)]   # (mu_eff, trials)


def component_sizes(g):
    """Sizes of the honest subgraph's connected components, descending."""
    H = g.params.H
    adj = g.adj
    seen = bytearray(H)
    sizes = []
    for v in range(H):
        if seen[v]:
            continue
        seen[v] = 1
        n = 1
        dq = deque([v])
        while dq:
            for w in adj[dq.popleft()]:
                if not seen[w]:
                    seen[w] = 1
                    n += 1
                    dq.append(w)
        sizes.append(n)
    sizes.sort(reverse=True)
    return sizes


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--seed", type=int, default=20260721)
    args = ap.parse_args()
    rng = random.Random(args.seed)

    print(f"M4 failure severity -- frozen RF = {RF}, N = {N}")
    for mu, T in CELLS:
        params = M4Params(N=N, k=int(round(mu * N)), RF=RF)
        pred = params.p_bad()
        dist: Counter = Counter()          # d = H - largest component
        frag: Counter = Counter()          # straggler component sizes
        for _ in range(T):
            sizes = component_sizes(M4Graph(params, rng))
            d = params.H - sizes[0]
            if d:
                dist[d] += 1
                for s in sizes[1:]:
                    frag[s] += 1
        bad = sum(dist.values())
        mc = bad / T
        se = math.sqrt(max(mc, 1 / T) * (1 - mc) / T)
        z = (mc - pred) / se
        print(f"  mu_eff={mu:.2f} trials={T} bad={bad} MC={mc:.3f} "
              f"law={pred:.3f} z={z:+.1f}")
        ds = "  ".join(f"{d}x{c}" for d, c in sorted(dist.items()))
        print(f"    d distribution: {ds}   (max {max(dist) if dist else 0})")
        fs = "  ".join(f"size {s}: {c}" for s, c in sorted(frag.items()))
        print(f"    straggler components: {fs}")


if __name__ == "__main__":
    main()
