#!/usr/bin/env python3
"""M1 adaptive eclipse cost: corruptions needed to strand a victim.

Backs ../properties/adaptive_eclipse_cost.md.

The cost of stranding a victim is its honest degree in the attacked
direction -- min-cut = degree whp, since at a branching factor of 19.2 the
depth->=2 shell is an order of magnitude larger than the depth-1 shell, so
Menger's disjoint-path count saturates at the degree.

  deafen v : cut v's honest IN-edges  -> v misses some publisher
  mute   v : cut v's honest OUT-edges -> v's own messages reach nobody

Two threat models:

  A  chosen victim -- the adversary names v and pays v's own draw.  Report
     mean/sd and the lower tail: epoch rotation re-draws the degree, so a
     standing target's exposure is the tail, not the mean.
  B  any victim -- the adversary only needs to break the delta guarantee, so
     it takes the cheapest node in either direction: min over H nodes.

In M1 the in-side is ACCEPTED (others' F picks, Poisson lower tail) and the
out-side is CHOSEN (own F picks, hypergeometric, concentrated), so M1 is
cheap to deafen and expensive to mute.

Consistency: the j=0 cell of each law is exactly m1_model's p_in_isolated /
p_out_isolated, i.e. the coverage law's defect term.  This script reads the
same distributions at j >= 1.

Usage: python3 sim_m1_eclipse.py [--trials T] [--seed S]
"""

from __future__ import annotations

import argparse
import random
from math import comb

from m1_model import M1Graph, M1Params

JMAX = 70


def chosen_pmf(picks, N, H):
    """Own picks landing on honest nodes -- hypergeometric."""
    tot, good = N - 1, H - 1
    bad = tot - good
    out = []
    for j in range(JMAX + 1):
        if j > picks or j > good or picks - j > bad:
            out.append(0.0)
        else:
            out.append(comb(good, j) * comb(bad, picks - j) / comb(tot, picks))
    return out


def accepted_pmf(picks, N, H):
    """Others' picks landing on this node -- Binomial(H-1, picks/(N-1))."""
    n, p = H - 1, picks / (N - 1)
    out, term = [], (1 - p) ** n
    for j in range(JMAX + 1):
        out.append(term)
        term *= (n - j) / (j + 1) * p / (1 - p)
    return out


def cdf(pmf):
    c, run = [], 0.0
    for p in pmf:
        run += p
        c.append(min(run, 1.0))
    return c


def summarise(pmf, H):
    c = cdf(pmf)
    mean = sum(j * p for j, p in enumerate(pmf))
    sd = sum((j - mean) ** 2 * p for j, p in enumerate(pmf)) ** 0.5
    emin = sum((1 - c[j - 1]) ** H for j in range(1, len(c)))
    p1 = next(j for j in range(len(c)) if c[j] >= 0.01)
    p01 = next(j for j in range(len(c)) if c[j] >= 0.001)
    return mean, sd, emin, p1, p01, c


def measure(N, H, F, trials, rng):
    """MC: per-graph mean and minimum honest in-/out-degree."""
    mean_in = mean_out = 0.0
    min_in, min_out = [], []
    for _ in range(trials):
        g = M1Graph(M1Params(N=N, k=N - H, F=F), rng)
        indeg = [0] * H
        outdeg = []
        for v in range(H):
            nb = {w for w in g.adj[v] if w < H}
            outdeg.append(len(nb))
            for w in nb:
                indeg[w] += 1
        mean_in += sum(indeg) / H
        mean_out += sum(outdeg) / H
        min_in.append(min(indeg))
        min_out.append(min(outdeg))
    return (mean_in / trials, min_in), (mean_out / trials, min_out)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--N", type=int, default=20000)
    ap.add_argument("--mu", type=float, default=0.2)
    ap.add_argument("--F", type=int, default=24)
    ap.add_argument("--trials", type=int, default=15)
    ap.add_argument("--seed", type=int, default=20260806)
    args = ap.parse_args()

    N, F, T = args.N, args.F, args.trials
    k = int(round(args.mu * N))
    H = N - k
    rng = random.Random(args.seed)

    deafen = accepted_pmf(F, N, H)      # in-edges: others picked me
    mute = chosen_pmf(F, N, H)          # out-edges: my own picks, honest part

    print(f"M1 adaptive eclipse cost -- N={N}, mu={args.mu}, F={F}, "
          f"{T} graphs\n")
    (mi, mins_in), (mo, mins_out) = measure(N, H, F, T, rng)

    print("THREAT A -- chosen victim (adversary names the target)")
    print(f"  {'direction':<10} {'side':<9} {'mean':>7} {'sd':>6} "
          f"{'p1%':>5} {'p0.1%':>6} {'MC mean':>9}")
    for label, pmf, side, mc in (("deafen", deafen, "accepted", mi),
                                 ("mute", mute, "chosen", mo)):
        mean, sd, _, p1, p01, _ = summarise(pmf, H)
        print(f"  {label:<10} {side:<9} {mean:>7.2f} {sd:>6.2f} "
              f"{p1:>5d} {p01:>6d} {mc:>9.2f}")

    print("\nTHREAT B -- any victim (cheapest break of the delta guarantee)")
    print(f"  {'direction':<10} {'E[min]':>7} {'MC min':>8}  observed")
    emins = []
    for label, pmf, obs in (("deafen", deafen, mins_in),
                            ("mute", mute, mins_out)):
        _, _, emin, _, _, _ = summarise(pmf, H)
        emins.append(emin)
        print(f"  {label:<10} {emin:>7.1f} {sum(obs)/len(obs):>8.1f}  "
              f"{sorted(set(obs))}")
    ca, cb = cdf(deafen), cdf(mute)
    joint = sum((1 - ca[j - 1]) ** H * (1 - cb[j - 1]) ** H
                for j in range(1, len(ca)))
    print(f"  guarantee-breaking cost = E[min over all nodes, both "
          f"directions] = {joint:.1f} corruptions "
          f"(adversarial budget is mu*N = {k})")

    # consistency with the published coverage law
    p = M1Params(N=N, k=k, F=F)
    law = H * (p.p_in_isolated() + p.p_out_isolated())
    mine = H * (deafen[0] + mute[0])
    print(f"\nCross-check vs coverage law: H*(P(indeg=0)+P(outdeg=0))")
    print(f"  m1_model.E_defects() = {law:.4g}   this script = {mine:.4g}   "
          f"{'OK' if abs(law - mine) <= 1e-9 + 1e-6 * law else 'MISMATCH'}")


if __name__ == "__main__":
    main()
