#!/usr/bin/env python3
"""M5 adaptive eclipse cost: corruptions needed to strand a victim.

Backs ../properties/adaptive_eclipse_cost.md.  See ../../m1/scripts/
sim_m1_eclipse.py for the shared framing (min-cut = degree; threat models
A and B).

M5 is the only model that mixes chosen and accepted on BOTH sides: a node's
in-degree is its own k_in picks (chosen, hypergeometric) plus others' k_out
picks landing on it (accepted, Poisson), and symmetrically for out.  That
makes the two directions nearly balanced -- and it means neither side gets
the concentration benefit that M2's all-chosen in-side enjoys.

Usage: python3 sim_m5_eclipse.py [--mc] [--trials T] [--seed S]

The closed-form tables print in under a second.  --mc (LONG, minutes at
the default 400 graphs of N = 20000; never run by CI) adds the Monte-Carlo
cross-check columns (MC mean / MC min / observed).  The published tables in
../properties/adaptive_eclipse_cost.md are from --mc --trials 400.
"""

from __future__ import annotations

import argparse
import random
from math import comb

from m5_model import M5Graph, M5Params

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


def measure(N, H, k_in, k_out, trials, rng):
    mean_in = mean_out = 0.0
    min_in, min_out = [], []
    for _ in range(trials):
        g = M5Graph(M5Params(N=N, k=N - H, k_in=k_in, k_out=k_out), rng)
        ins = [set() for _ in range(H)]
        outdeg = []
        for v in range(H):
            nb = {w for w in g.adj[v] if w < H}
            outdeg.append(len(nb))
            for w in nb:
                ins[w].add(v)
        indeg = [len(sv) for sv in ins]
        mean_in += sum(indeg) / H
        mean_out += sum(outdeg) / H
        min_in.append(min(indeg))
        min_out.append(min(outdeg))
    return (mean_in / trials, min_in), (mean_out / trials, min_out)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--N", type=int, default=20000)
    ap.add_argument("--mu", type=float, default=0.2)
    ap.add_argument("--k_in", type=int, default=9)
    ap.add_argument("--k_out", type=int, default=8)
    ap.add_argument("--trials", type=int, default=400)
    ap.add_argument("--seed", type=int, default=20260806)
    ap.add_argument("--mc", action="store_true",
                    help="run the Monte-Carlo degree measurement (LONG; "
                         "never run by CI)")
    args = ap.parse_args()

    N, ki, ko, T = args.N, args.k_in, args.k_out, args.trials
    k = int(round(args.mu * N))
    H = N - k
    rng = random.Random(args.seed)

    deafen = convolve(chosen_pmf(ki, N, H), accepted_pmf(ko, N, H))
    mute = convolve(chosen_pmf(ko, N, H), accepted_pmf(ki, N, H))

    print(f"M5 adaptive eclipse cost -- N={N}, mu={args.mu}, "
          f"(k_in,k_out)=({ki},{ko}), "
          + (f"{T} graphs\n" if args.mc else "closed forms (--mc adds MC)\n"))
    mc = measure(N, H, ki, ko, T, rng) if args.mc else None

    print("THREAT A -- chosen victim (adversary names the target)")
    print(f"  {'direction':<10} {'side':<14} {'mean':>7} {'sd':>6} "
          f"{'p1%':>5} {'p0.1%':>6}"
          + (f" {'MC mean':>9}" if mc else ""))
    rows = (("deafen", deafen), ("mute", mute))
    for i, (label, pmf) in enumerate(rows):
        mean, sd, _, p1, p01 = summarise(pmf, H)
        print(f"  {label:<10} {'chosen+accept':<14} {mean:>7.2f} {sd:>6.2f} "
              f"{p1:>5d} {p01:>6d}"
              + (f" {mc[i][0]:>9.2f}" if mc else ""))

    print("\nTHREAT B -- any victim (cheapest break of the delta guarantee)")
    print(f"  {'direction':<10} {'E[min]':>7}"
          + (f" {'MC min':>8}  observed" if mc else ""))
    for i, (label, pmf) in enumerate(rows):
        _, _, emin, _, _ = summarise(pmf, H)
        tail = ""
        if mc:
            obs = mc[i][1]
            tail = f" {sum(obs)/len(obs):>8.1f}  {sorted(set(obs))}"
        print(f"  {label:<10} {emin:>7.1f}" + tail)
    ca, cb = cdf(deafen), cdf(mute)
    joint = sum((1 - ca[j - 1]) ** H * (1 - cb[j - 1]) ** H
                for j in range(1, len(ca)))
    print(f"  guarantee-breaking cost = E[min over all nodes, both "
          f"directions] = {joint:.1f} corruptions "
          f"(adversarial budget is mu*N = {k})")
    print("  (below either marginal: M5's two directions are close enough "
          "that\n   the cheapest node overall beats the cheapest in either "
          "direction alone)")

    p = M5Params(N=N, k=k, k_in=ki, k_out=ko)
    print(f"\nCross-check vs coverage law:")
    for name, got, want in (("P(indeg=0)", deafen[0], p.p_in_isolated()),
                            ("P(outdeg=0)", mute[0], p.p_out_isolated())):
        ok = "OK" if abs(got - want) <= 1e-12 + 1e-6 * max(want, 1e-30) else "MISMATCH"
        print(f"  {name}: m5_model = {want:.4g}   this script = {got:.4g}   {ok}")


if __name__ == "__main__":
    main()
