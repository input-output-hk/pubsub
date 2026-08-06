#!/usr/bin/env python3
"""M3 adaptive eclipse cost: corruptions needed to strand a victim.

Backs ../properties/adaptive_eclipse_cost.md.  See ../../m1/scripts/
sim_m1_eclipse.py for the shared framing (min-cut = degree; threat models
A and B).

M3 has two link kinds and they carry different traffic, so "deafen" needs a
definition.  The coverage criterion is that every message of every honest
publisher reaches every honest node, and initiation links deliver only their
own owner's publications.  So:

  deafen (coverage) : cut the RF chosen forwarders (mean 9.6).  The victim
                      can no longer receive arbitrary publishers -- coverage
                      is broken -- though it still hears its initiation
                      partners' own messages.
  deafen (silence)  : additionally cut the accepted initiation in-links
                      (mean 5.6), total 15.2, after which the victim hears
                      nothing at all.
  mute              : cut the honest requesters that pull from it (accepted,
                      mean 9.6) AND its own honest initiation targets
                      (chosen, mean 5.6) -- total 15.2.

The reported guarantee-breaking cost uses the coverage reading, since that
is the criterion delta is stated against; the silence reading is printed
alongside because it is the number an operator would recognise as "eclipsed".

Usage: python3 sim_m3_eclipse.py [--trials T] [--seed S]
"""

from __future__ import annotations

import argparse
import random
from math import comb

from m3_model import M3Graph, M3Params

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


def measure(N, H, RF, s, trials, rng):
    acc = {"cover": 0.0, "silence": 0.0, "mute": 0.0}
    mins = {"cover": [], "silence": [], "mute": []}
    for _ in range(trials):
        g = M3Graph(M3Params(N=N, k=N - H, RF=RF, s=s), rng)
        cover = [0] * H
        init_in = [0] * H
        out = [set() for _ in range(H)]
        for j in range(H):
            nb = {f for f in g.picks_of(j) if f < H}
            cover[j] = len(nb)
            for f in nb:
                out[f].add(j)                  # honest requester served
            for t in g.init_targets[j]:        # already honest-filtered
                init_in[t] += 1
                out[j].add(t)                  # own honest initiation target
        silence = [cover[v] + init_in[v] for v in range(H)]
        mute = [len(out[v]) for v in range(H)]
        for key, arr in (("cover", cover), ("silence", silence), ("mute", mute)):
            acc[key] += sum(arr) / H
            mins[key].append(min(arr))
    return {k: (acc[k] / trials, mins[k]) for k in acc}


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--N", type=int, default=20000)
    ap.add_argument("--mu", type=float, default=0.2)
    ap.add_argument("--RF", type=int, default=12)
    ap.add_argument("--s", type=int, default=8)
    ap.add_argument("--trials", type=int, default=400)
    ap.add_argument("--seed", type=int, default=20260806)
    args = ap.parse_args()

    N, RF, s, T = args.N, args.RF, args.s, args.trials
    k = int(round(args.mu * N))
    H = N - k
    rng = random.Random(args.seed)

    cover = chosen_pmf(RF, N, H)                        # chosen forwarders
    init_in = accepted_pmf(s - 1, N, H)                 # accepted initiation
    silence = convolve(cover, init_in)
    mute = convolve(accepted_pmf(RF, N, H), chosen_pmf(s - 1, N, H))

    print(f"M3 adaptive eclipse cost -- N={N}, mu={args.mu}, RF={RF}, s={s}, "
          f"{T} graphs\n")
    mc = measure(N, H, RF, s, T, rng)

    print("THREAT A -- chosen victim (adversary names the target)")
    print(f"  {'attack':<18} {'side':<14} {'mean':>7} {'sd':>6} "
          f"{'p1%':>5} {'p0.1%':>6} {'MC mean':>9}")
    plan = (("deafen (coverage)", cover, "chosen", "cover"),
            ("deafen (silence)", silence, "chosen+accept", "silence"),
            ("mute", mute, "accept+chosen", "mute"))
    for label, pmf, side, key in plan:
        mean, sd, _, p1, p01 = summarise(pmf, H)
        print(f"  {label:<18} {side:<14} {mean:>7.2f} {sd:>6.2f} "
              f"{p1:>5d} {p01:>6d} {mc[key][0]:>9.2f}")

    print("\nTHREAT B -- any victim (cheapest break of the delta guarantee)")
    print(f"  {'attack':<18} {'E[min]':>7} {'MC min':>8}  observed")
    for label, pmf, _side, key in plan:
        _, _, emin, _, _ = summarise(pmf, H)
        print(f"  {label:<18} {emin:>7.1f} "
              f"{sum(mc[key][1])/len(mc[key][1]):>8.1f}  {sorted(set(mc[key][1]))}")
    ca, cb = cdf(cover), cdf(mute)
    joint = sum((1 - ca[j - 1]) ** H * (1 - cb[j - 1]) ** H
                for j in range(1, len(ca)))
    print(f"  guarantee-breaking cost = E[min over all nodes, "
          f"deafen-coverage or mute] = {joint:.1f} corruptions "
          f"(adversarial budget is mu*N = {k})")

    p = M3Params(N=N, k=k, RF=RF, s=s)
    print(f"\nCross-check vs coverage law:")
    for name, got, want in (("P(indeg=0)", cover[0], p.p_in_isolated()),
                            ("P(outdeg=0)", mute[0], p.p_out_isolated())):
        ok = "OK" if abs(got - want) <= 1e-12 + 1e-6 * max(want, 1e-30) else "MISMATCH"
        print(f"  {name}: m3_model = {want:.4g}   this script = {got:.4g}   {ok}")


if __name__ == "__main__":
    main()
