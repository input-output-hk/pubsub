#!/usr/bin/env python3
"""Simulation: probability that a sampled M2 graph GUARANTEES full coverage
(every regular honest node reachable from the seed set) -- the per-epoch
success probability (../properties/full_coverage.md, secondary metric).

Setting: N = 20000; pull relaying with fanout
RF; k = mu*N silent adversaries.  Seeding = initiation links: the publisher (an
honest node) holds the message and pushes it to s-1 targets chosen UNIFORMLY
from the other N-1 nodes -- a target is adversarial with probability ~mu, in
which case that copy is silently wasted.  Swept: RF x s, panels for mu.

Prediction (unified law):

    P(success) ~ (1 - (1-rho_f) * (1-(1-mu)*rho_f)^(s-1)) * exp(-H*u)
                 [ignition succeeds]                       * [floor empty]

    rho_f = 1 - exp(-RF*(1-mu)*rho_f)              per-seed survival
    u     = smallest root of u = (mu+(1-mu)u)^RF   reach floor

  Ignition: the publisher's own spread dies w.p. 1-rho_f; each pushed copy
  is useful iff its target is honest AND its spread survives, failing w.p.
  1-(1-mu)*rho_f.

Usage: python3 sim_p03_full_coverage.py [--trials T] [--seed S]
"""

import argparse
import math
import random

import os, sys
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                 "..", "..", "m2", "scripts"))
from m2_model import M2Params
from m3_model import M3Graph, rho_giant, u_iterate

N = 20000
RFS = (2, 3, 5, 7, 10)
SEEDS = (1, 2, 3, 5, 10)
MUS = (0.0, 0.1)
RFS_BASELINE = (2, 3, 5, 7, 10, 11)
MUS_BASELINE = (0.0, 0.05, 0.1, 0.2)


def predict_success(mu: float, RF: int, s: int) -> float:
    H = N - int(round(mu * N))
    m_branch = RF * (1 - mu)
    rho_f = rho_giant(m_branch) if m_branch > 1 + 1e-9 else 0.0
    u = u_iterate(mu, RF)
    ign_fail = (1 - rho_f) * (1 - (1 - mu) * rho_f) ** (s - 1)
    return (1 - ign_fail) * math.exp(-H * u)


def sample_seed_set(g: M3Graph, s: int, rng: random.Random):
    """Publisher (honest node 0) + s-1 uniform initiation targets; only
    honest targets become holders (adversarial targets waste the copy)."""
    pub = 0                                # first regular honest node
    seeds = [pub]
    for r in rng.sample(range(N - 1), s - 1):
        t = r if r < pub else r + 1
        if not g.is_adversarial(t):
            seeds.append(t)
    return seeds


def table_grid(T: int, rng: random.Random) -> None:
    """RF x s grid, panels per mu (initiation-link seeding)."""
    for mu in MUS:
        k = int(round(mu * N))
        print()
        print(f"  mu = {mu}")
        print(f"  {'RF':>4} |" +
              "".join(f" {'s='+str(s):>13}" for s in SEEDS))
        for RF in RFS:
            params = M2Params(N=N, k=k, RF=RF)
            cells = []
            for s in SEEDS:
                pred = predict_success(mu, RF, s)
                good = 0
                for _ in range(T):
                    g = M3Graph(params, rng)
                    depth = g.depths(seeds=sample_seed_set(g, s, rng))
                    good += all(depth[j] >= 0 for j in g.regular_nodes())
                cells.append((pred, good / T))
            print(f"  {RF:>4} |" +
                  "".join(f" {p:>6.3f}/{m:<6.3f}" for p, m in cells))


def table_baseline(T: int, rng: random.Random) -> None:
    """Publisher-only seeding (s=1) for a FIXED publisher.  RF x mu.
    Prediction reduces to rho_f * exp(-H*u)."""
    print()
    print(f"  fixed publisher, s = 1 (no initiation links): RF x mu")
    print(f"  {'RF':>4} |" +
          "".join(f" {'mu='+format(mu, '.2f'):>13}" for mu in MUS_BASELINE))
    for RF in RFS_BASELINE:
        cells = []
        for mu in MUS_BASELINE:
            k = int(round(mu * N))
            params = M2Params(N=N, k=k, RF=RF)
            pred = predict_success(mu, RF, 1)
            good = 0
            for _ in range(T):
                g = M3Graph(params, rng)
                depth = g.depths(seeds=sample_seed_set(g, 1, rng))
                good += all(depth[j] >= 0 for j in g.regular_nodes())
            cells.append((pred, good / T))
        print(f"  {RF:>4} |" +
              "".join(f" {p:>6.3f}/{m:<6.3f}" for p, m in cells))


RFS_M2 = (8, 10, 12, 14, 16, 18)


def predict_good_m2(mu: float, RF: int) -> float:
    """M2 (RF links only, ANY publisher): graph is good iff every
    regular honest node can, as publisher, reach all regular honest nodes.
    Defects: out (spread dies; sink-dominated) ~ H*(1-rho_f); in (eclipse
    floor) ~ H*u.  P(good) ~ exp(-H*[(1-rho_f)+u])."""
    H = N - int(round(mu * N))
    m_branch = RF * (1 - mu)
    rho_f = rho_giant(m_branch) if m_branch > 1 + 1e-9 else 0.0
    u = u_iterate(mu, RF)
    return math.exp(-H * ((1 - rho_f) + u))


def m2_graph_good(g: M3Graph) -> bool:
    """Good iff forward AND backward BFS from an honest node cover all
    regular honest nodes (adversaries never relay; adj already honest-only)."""
    from collections import deque
    adj = g.adjacency()
    radj = [[] for _ in range(g.params.N)]
    for v, ws in enumerate(adj):
        for w in ws:
            radj[w].append(v)
    start = 0                                # first regular honest node
    for edges in (adj, radj):
        seen = bytearray(g.params.N)
        seen[start] = 1
        dq = deque([start])
        while dq:
            v = dq.popleft()
            for w in edges[v]:
                if not seen[w]:
                    seen[w] = 1
                    dq.append(w)
        if not all(seen[j] for j in g.regular_nodes()):
            return False
    return True


def table_m2(T: int, rng: random.Random) -> None:
    """M2 (no seeding mechanism): good-graph probability, RF x mu."""
    print()
    print(f"  M2 (RF links only, any publisher): RF x mu")
    print(f"  {'RF':>4} |" +
          "".join(f" {'mu='+format(mu, '.2f'):>13}" for mu in MUS_BASELINE))
    for RF in RFS_M2:
        cells = []
        for mu in MUS_BASELINE:
            k = int(round(mu * N))
            params = M2Params(N=N, k=k, RF=RF)
            pred = predict_good_m2(mu, RF)
            good = sum(m2_graph_good(M3Graph(params, rng)) for _ in range(T))
            cells.append((pred, good / T))
        print(f"  {RF:>4} |" +
              "".join(f" {p:>6.3f}/{m:<6.3f}" for p, m in cells))


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--trials", type=int, default=200)
    ap.add_argument("--seed", type=int, default=12345)
    ap.add_argument("--table", choices=("all", "grid", "baseline", "m2"),
                    default="all", help="which table(s) to produce")
    args = ap.parse_args()
    rng = random.Random(args.seed)
    T = args.trials

    print(f"P(sampled graph guarantees full coverage)  --  N={N}, "
          f"{T} trials/cell, predicted/MC")
    if args.table in ("all", "grid"):
        table_grid(T, rng)
    if args.table in ("all", "baseline"):
        table_baseline(T, rng)
    if args.table in ("all", "m2"):
        table_m2(T, rng)


if __name__ == "__main__":
    main()
