#!/usr/bin/env python3
"""M2 adaptive eclipse cost: corruptions needed to strand a victim.

Backs ../properties/adaptive_eclipse_cost.md.  See sim_m1_eclipse.py for the
shared framing (min-cut = degree; threat models A and B).

M2 is M1's exact mirror.  Its in-side is CHOSEN (own RF pull picks,
hypergeometric, concentrated) and its out-side is ACCEPTED (the requesters
that happen to pull from it, Poisson lower tail).  So M2 is expensive to
deafen and cheap to mute -- and since coverage fails either way, its
guarantee-breaking cost is set by the muted-publisher side, matching the
fact that M2's published P(bad) is carried entirely by that term.

Usage: python3 sim_m2_eclipse.py [--trials T] [--seed S]
"""

from __future__ import annotations

import argparse
import random
from math import comb

from m2_model import M2Graph, M2Params

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
    mean_in = mean_out = 0.0
    min_in, min_out = [], []
    for _ in range(trials):
        g = M2Graph(M2Params(N=N, k=N - H, RF=RF), rng)
        indeg = [0] * H
        outdeg = [0] * H
        for j in range(H):
            nb = {f for f in g.picks_of(j) if f < H}
            indeg[j] = len(nb)
            for f in nb:
                outdeg[f] += 1
        mean_in += sum(indeg) / H
        mean_out += sum(outdeg) / H
        min_in.append(min(indeg))
        min_out.append(min(outdeg))
    return (mean_in / trials, min_in), (mean_out / trials, min_out)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--N", type=int, default=20000)
    ap.add_argument("--mu", type=float, default=0.2)
    ap.add_argument("--RF", type=int, default=24)
    ap.add_argument("--trials", type=int, default=400)
    ap.add_argument("--seed", type=int, default=20260806)
    args = ap.parse_args()

    N, RF, T = args.N, args.RF, args.trials
    k = int(round(args.mu * N))
    H = N - k
    rng = random.Random(args.seed)

    deafen = chosen_pmf(RF, N, H)       # in-edges: my own forwarder picks
    mute = accepted_pmf(RF, N, H)       # out-edges: requesters that pull me

    print(f"M2 adaptive eclipse cost -- N={N}, mu={args.mu}, RF={RF}, "
          f"{T} graphs\n")
    (mi, mins_in), (mo, mins_out) = measure(N, H, RF, T, rng)

    print("THREAT A -- chosen victim (adversary names the target)")
    print(f"  {'direction':<10} {'side':<9} {'mean':>7} {'sd':>6} "
          f"{'p1%':>5} {'p0.1%':>6} {'MC mean':>9}")
    for label, pmf, side, mc in (("deafen", deafen, "chosen", mi),
                                 ("mute", mute, "accepted", mo)):
        mean, sd, _, p1, p01 = summarise(pmf, H)
        print(f"  {label:<10} {side:<9} {mean:>7.2f} {sd:>6.2f} "
              f"{p1:>5d} {p01:>6d} {mc:>9.2f}")

    print("\nTHREAT B -- any victim (cheapest break of the delta guarantee)")
    print(f"  {'direction':<10} {'E[min]':>7} {'MC min':>8}  observed")
    for label, pmf, obs in (("deafen", deafen, mins_in),
                            ("mute", mute, mins_out)):
        _, _, emin, _, _ = summarise(pmf, H)
        print(f"  {label:<10} {emin:>7.1f} {sum(obs)/len(obs):>8.1f}  "
              f"{sorted(set(obs))}")
    ca, cb = cdf(deafen), cdf(mute)
    joint = sum((1 - ca[j - 1]) ** H * (1 - cb[j - 1]) ** H
                for j in range(1, len(ca)))
    print(f"  guarantee-breaking cost = E[min over all nodes, both "
          f"directions] = {joint:.1f} corruptions "
          f"(adversarial budget is mu*N = {k})")

    p = M2Params(N=N, k=k, RF=RF)
    print(f"\nCross-check vs coverage law:")
    print(f"  P(indeg=0): m2_model.p_eclipse() = {p.p_eclipse():.4g}   "
          f"this script = {deafen[0]:.4g}   "
          f"{'OK' if abs(p.p_eclipse() - deafen[0]) <= 1e-12 + 1e-6 * p.p_eclipse() else 'MISMATCH'}")
    print(f"  P(outdeg=0) = e^-RF(1-mu) = {mute[0]:.4g}  "
          f"(the muted-publisher term that carries M2's P(bad))")


if __name__ == "__main__":
    main()
