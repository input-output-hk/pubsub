#!/usr/bin/env python3
"""M4 node degrees: honest degree distribution vs closed form at the
operating point.  Backs ../properties/node_degrees.md.

Undirected: degree = own RF picks (chosen, thinned) + honest nodes that
picked me (accepted ~ Binomial(H-1, RF/(N-1))); every edge is in AND out.
"""
import argparse, random
from m4_model import M4Params, M4Graph


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
    ap.add_argument("--RF", type=int, default=9)
    ap.add_argument("--trials", type=int, default=25)
    ap.add_argument("--seed", type=int, default=2024)
    args = ap.parse_args()
    N, mu, RF, T = args.N, args.mu, args.RF, args.trials
    k = int(round(mu * N)); H = N - k
    rng = random.Random(args.seed)

    degs, maxd = [], []
    for _ in range(T):
        g = M4Graph(M4Params(N=N, k=k, RF=RF), rng)
        d = [len(g.adj[v]) for v in range(H)]
        degs.extend(d)
        maxd.append(max(d))
    p = RF / (N - 1)
    mean = RF * (1 - k / (N - 1)) + (H - 1) * p
    var = (RF * (k / (N - 1)) * (1 - k / (N - 1))
           + (H - 1) * p * (1 - p))
    print(f"M4 degrees  --  N={N}, mu={mu}, RF={RF}, {T} graphs "
          f"(chosen = {RF} held; honest-effective below)")
    report("degree (in = out, honest)", degs, mean, var ** 0.5)
    print(f"  per-graph max degree: mean {sum(maxd)/len(maxd):.1f}, "
          f"worst {max(maxd)}")
    print(f"  compliant total degree mean = 2*RF = {2*RF}")


if __name__ == "__main__":
    main()
