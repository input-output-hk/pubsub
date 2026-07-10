#!/usr/bin/env python3
"""M1 node degrees: honest in-/out-degree distributions vs closed forms at
the operating point.  Backs ../properties/node_degrees.md.

out (chosen) = own F picks, thinned to honest counterparts (hypergeometric);
in (accepted) = honest nodes that picked me ~ Binomial(H-1, F/(N-1)).
"""
import argparse, random
from m1_model import M1Params, M1Graph


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
    ap.add_argument("--F", type=int, default=24)
    ap.add_argument("--trials", type=int, default=25)
    ap.add_argument("--seed", type=int, default=2024)
    args = ap.parse_args()
    N, mu, F, T = args.N, args.mu, args.F, args.trials
    k = int(round(mu * N)); H = N - k
    rng = random.Random(args.seed)

    outs, ins, maxin = [], [], []
    for _ in range(T):
        g = M1Graph(M1Params(N=N, k=k, F=F), rng)
        indeg = [0] * H
        for i in range(H):
            outs.append(len(g.adj[i]))
            for j in g.adj[i]:
                indeg[j] += 1
        ins.extend(indeg)
        maxin.append(max(indeg))
    p = F / (N - 1)
    print(f"M1 degrees  --  N={N}, mu={mu}, F={F}, {T} graphs "
          f"(chosen out = {F} held; honest-effective below)")
    report("out (chosen, honest)", outs, F * (1 - k / (N - 1)),
           (F * (k / (N - 1)) * (1 - k / (N - 1))) ** 0.5)
    report("in  (accepted, honest)", ins, (H - 1) * p,
           ((H - 1) * p * (1 - p)) ** 0.5)
    print(f"  per-graph max in-degree: mean "
          f"{sum(maxin)/len(maxin):.1f}, worst {max(maxin)}")
    print(f"  compliant total degree mean = 2F = {2*F}")


if __name__ == "__main__":
    main()
