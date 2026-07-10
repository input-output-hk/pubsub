#!/usr/bin/env python3
"""M2 node degrees: honest in-/out-degree distributions vs closed forms at
the operating point.  Backs ../properties/node_degrees.md.

in (chosen) = own RF forwarder picks, thinned to honest counterparts;
out (accepted) = honest requesters that picked me ~ Binomial(H-1, RF/(N-1)).
"""
import argparse, random
from m2_model import M2Params, M2Graph


def stats(xs):
    n = len(xs)
    m = sum(xs) / n
    v = sum((x - m) ** 2 for x in xs) / (n - 1)
    return m, v ** 0.5


def report(label, per_node, pred_mean, pred_sd):
    m, sd = stats(per_node)
    mx = max(per_node)
    print(f"  {label:<26} mean {m:7.2f} (pred {pred_mean:6.2f})   "
          f"sd {sd:5.2f} (pred {pred_sd:5.2f})   max {mx}")
    return m, mx


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--N", type=int, default=20000)
    ap.add_argument("--mu", type=float, default=0.2)
    ap.add_argument("--RF", type=int, default=24)
    ap.add_argument("--trials", type=int, default=25)
    ap.add_argument("--seed", type=int, default=2024)
    args = ap.parse_args()
    N, mu, RF, T = args.N, args.mu, args.RF, args.trials
    k = int(round(mu * N)); H = N - k
    rng = random.Random(args.seed)

    ins, outs, maxout = [], [], []
    for _ in range(T):
        g = M2Graph(M2Params(N=N, k=k, RF=RF), rng)
        outdeg = [0] * H
        for j in range(H):
            picks = g.picks_of(j)
            ins.append(sum(1 for f in picks if f < H))
            for f in picks:
                if f < H:
                    outdeg[f] += 1
        outs.extend(outdeg)
        maxout.append(max(outdeg))
    p = RF / (N - 1)
    print(f"M2 degrees  --  N={N}, mu={mu}, RF={RF}, {T} graphs "
          f"(chosen in = {RF} held; honest-effective below)")
    report("in  (chosen, honest)", ins, RF * (1 - k / (N - 1)),
           (RF * (k / (N - 1)) * (1 - k / (N - 1))) ** 0.5)
    report("out (accepted, honest)", outs, (H - 1) * p,
           ((H - 1) * p * (1 - p)) ** 0.5)
    print(f"  per-graph max out-degree: mean "
          f"{sum(maxout)/len(maxout):.1f}, worst {max(maxout)}")
    print(f"  compliant total degree mean = 2*RF = {2*RF}")


if __name__ == "__main__":
    main()
