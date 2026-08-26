#!/usr/bin/env python3
"""M4 adaptive eclipse cost: corruptions needed to strand a victim.

Backs ../properties/adaptive_eclipse_cost.md.  See ../../m1/scripts/
sim_m1_eclipse.py for the shared framing (min-cut = degree; threat models
A and B).

M4's links are UNDIRECTED, so deafening and muting are the same attack:
cutting a node's honest links strands it in both directions at once, and
the cost is its honest degree -- NOT twice that.  The degree mixes a chosen
side (own RF picks, hypergeometric) and an accepted side (others' picks,
Poisson), mean 2*RF(1-mu) = 14.4 at the operating point.

Because the two directions are one event rather than two independent draws,
M4's guarantee-breaking cost is its single degree minimum; taking a minimum
over 2H draws here would double-count.

Usage: python3 sim_m4_eclipse.py [--mc] [--trials T] [--seed S]

The closed-form tables print in under a second.  --mc (LONG, minutes at
the default 400 graphs of N = 20000; never run by CI) adds the Monte-Carlo
cross-check columns (MC mean / MC min / observed).  The published tables in
../properties/adaptive_eclipse_cost.md are from --mc --trials 400.
"""

from __future__ import annotations

import argparse
import random
from math import comb

from m4_model import M4Graph, M4Params

JMAX = 70


def chosen_pmf(picks, N, H):
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
    n, p = H - 1, picks / (N - 1)
    out, term = [], (1 - p) ** n
    for j in range(JMAX + 1):
        out.append(term)
        term *= (n - j) / (j + 1) * p / (1 - p)
    return out


def convolve(a, b):
    out = [0.0] * (JMAX + 1)
    for i, ai in enumerate(a):
        if ai:
            for j, bj in enumerate(b):
                if i + j > JMAX:
                    break
                out[i + j] += ai * bj
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
    return mean, sd, emin, p1, p01


def measure(N, H, RF, trials, rng):
    mean_d = 0.0
    mins = []
    for _ in range(trials):
        g = M4Graph(M4Params(N=N, k=N - H, RF=RF), rng)
        deg = [len({w for w in g.adj[v] if w < H}) for v in range(H)]
        mean_d += sum(deg) / H
        mins.append(min(deg))
    return mean_d / trials, mins


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--N", type=int, default=20000)
    ap.add_argument("--mu", type=float, default=0.2)
    ap.add_argument("--RF", type=int, default=9)
    ap.add_argument("--trials", type=int, default=400)
    ap.add_argument("--seed", type=int, default=20260806)
    ap.add_argument("--mc", action="store_true",
                    help="run the Monte-Carlo degree measurement (LONG; "
                         "never run by CI)")
    args = ap.parse_args()

    N, RF, T = args.N, args.RF, args.trials
    k = int(round(args.mu * N))
    H = N - k
    rng = random.Random(args.seed)

    deg = convolve(chosen_pmf(RF, N, H), accepted_pmf(RF, N, H))
    mean, sd, emin, p1, p01 = summarise(deg, H)
    mc = measure(N, H, RF, T, rng) if args.mc else None

    print(f"M4 adaptive eclipse cost -- N={N}, mu={args.mu}, RF={RF}, "
          + (f"{T} graphs" if args.mc else "closed forms (--mc adds MC)"))
    print("  (undirected: deafen and mute are the SAME cut)\n")

    print("THREAT A -- chosen victim (adversary names the target)")
    print(f"  {'direction':<14} {'side':<14} {'mean':>7} {'sd':>6} "
          f"{'p1%':>5} {'p0.1%':>6}"
          + (f" {'MC mean':>9}" if mc else ""))
    print(f"  {'deafen = mute':<14} {'chosen+accept':<14} {mean:>7.2f} "
          f"{sd:>6.2f} {p1:>5d} {p01:>6d}"
          + (f" {mc[0]:>9.2f}" if mc else ""))

    print("\nTHREAT B -- any victim (cheapest break of the delta guarantee)")
    if mc:
        mins = mc[1]
        print(f"  E[min degree] = {emin:.1f}   "
              f"MC min = {sum(mins)/len(mins):.1f}   "
              f"observed {sorted(set(mins))}")
    else:
        print(f"  E[min degree] = {emin:.1f}")
    print(f"  guarantee-breaking cost = {emin:.1f} corruptions "
          f"(adversarial budget is mu*N = {k})")
    print("  NOT min over 2H draws -- one undirected cut kills both directions")

    p = M4Params(N=N, k=k, RF=RF)
    print(f"\nCross-check vs coverage law: P(degree=0)")
    print(f"  m4_model.p_isolated() = {p.p_isolated():.4g}   "
          f"this script = {deg[0]:.4g}   "
          f"{'OK' if abs(p.p_isolated() - deg[0]) <= 1e-12 + 1e-6 * p.p_isolated() else 'MISMATCH'}")


if __name__ == "__main__":
    main()
