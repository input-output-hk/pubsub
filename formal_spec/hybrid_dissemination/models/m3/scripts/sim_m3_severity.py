#!/usr/bin/env python3
"""M3 failure severity: size and type of the stranded set on bad graphs.

Conditional on a sampled graph being bad under the strict every-publisher
criterion, counts the actually failing nodes: deaf (some publisher's
message cannot reach them, even via initiation seeding) and mute
(publishers whose messages cannot cover all honest nodes).  Also reports
the initiation-rescue effect: how many pull-mute candidates (publishers
with no requester path to the giant SCC) the standing initiation links
save.  Sampled at elevated mu where bad graphs are collectable; at the
operating point the defect intensity E ~ 1e-4 makes simultaneous defects
vanishingly rare, so the per-defect sizes measured here bound the
op-point severity.

Usage: python3 sim_m3_severity.py [--seed SEED]
"""

from __future__ import annotations

import argparse
import math
import os
import random
import sys
from collections import Counter, deque

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from m3_model import M3Graph, M3Params  # noqa: E402

N = 20_000
RF = 12
S = 8
CELLS = [(0.40, 650), (0.45, 280)]   # (mu_eff, trials)


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
    """(deaf, mute, mute_candidates) under the strict criterion.

    deaf: honest nodes outside the giant's forward set that at least one
    publisher's seed set cannot reach (mirrors strict_bad surface (a));
    mute: publishers whose seed set misses the giant's reverse set and
    whose own spread does not cover H (surface (b)); mute_candidates:
    publishers with no requester path to the giant -- mute under pure
    pull, before initiation seeding is credited."""
    adj = g.adjacency()
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

    deaf = 0
    for j in range(H):
        if fwd[j]:
            continue
        anc = bfs_set(radj, [j], H)
        for p in range(H):
            if not anc[p] and not any(anc[t] for t in g.init_targets[p]):
                deaf += 1
                break

    mute = mute_cand = 0
    for p in range(H):
        if rev[p]:
            continue
        mute_cand += 1
        if any(rev[t] for t in g.init_targets[p]):
            continue                             # rescued by initiation
        if sum(bfs_set(adj, [p] + g.init_targets[p], H)) < H:
            mute += 1
    return (deaf, mute, mute_cand)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--seed", type=int, default=20260721)
    args = ap.parse_args()
    rng = random.Random(args.seed)

    print(f"M3 failure severity -- frozen (RF, s) = ({RF}, {S}), N = {N}")
    for mu, T in CELLS:
        params = M3Params(N=N, k=int(round(mu * N)), RF=RF, s=S)
        pred = params.p_bad()
        H = params.H
        dist: Counter = Counter()
        deaf_tot = mute_tot = cand_tot = 0
        for _ in range(T):
            s = severity(M3Graph(params, rng), H)
            if s is None:
                print("  WARNING: no giant SCC found")
                continue
            deaf, mute, cand = s
            cand_tot += cand
            d = deaf + mute
            if d:
                dist[d] += 1
                deaf_tot += deaf
                mute_tot += mute
        bad = sum(dist.values())
        mc = bad / T
        se = math.sqrt(max(mc, 1 / T) * (1 - mc) / T)
        z = (mc - pred) / se
        print(f"  mu_eff={mu:.2f} trials={T} bad={bad} MC={mc:.3f} "
              f"law={pred:.3f} z={z:+.1f}")
        ds = "  ".join(f"{d}x{c}" for d, c in sorted(dist.items()))
        print(f"    d distribution: {ds}   (max {max(dist) if dist else 0})")
        print(f"    failing nodes: deaf {deaf_tot}, mute {mute_tot}; "
              f"pull-mute candidates {cand_tot} "
              f"(initiation rescued {cand_tot - mute_tot})")


if __name__ == "__main__":
    main()
