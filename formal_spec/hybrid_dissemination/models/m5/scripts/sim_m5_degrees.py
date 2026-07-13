#!/usr/bin/env python3
"""M5 node degrees: honest in-/out-degree distributions vs closed forms at
the operating point.  Backs ../properties/node_degrees.md.

in  = own k_in picks (chosen, thinned) + honest out-picks of others hitting
      me (accepted ~ Binomial(H-1, k_out/(N-1)));
out = own k_out picks (chosen, thinned) + honest in-picks of others hitting
      me, i.e. requesters (accepted ~ Binomial(H-1, k_in/(N-1))).
"""
import argparse, random
from m5_model import M5Params, M5Graph


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
    ap.add_argument("--k_in", type=int, default=9)
    ap.add_argument("--k_out", type=int, default=8)
    ap.add_argument("--trials", type=int, default=25)
    ap.add_argument("--seed", type=int, default=2024)
    args = ap.parse_args()
    N, mu, a, b, T = args.N, args.mu, args.k_in, args.k_out, args.trials
    k = int(round(mu * N)); H = N - k
    rng = random.Random(args.seed)

    outs, ins, maxin, maxout = [], [], [], []
    for _ in range(T):
        g = M5Graph(M5Params(N=N, k=k, k_in=a, k_out=b), rng)
        indeg = [0] * H
        for v in range(H):
            outs.append(len(g.adj[v]))
            for w in g.adj[v]:
                indeg[w] += 1
        ins.extend(indeg)
        maxin.append(max(indeg))
        maxout.append(max(len(g.adj[v]) for v in range(H)))
    hon = 1 - k / (N - 1)
    thin_var = (k / (N - 1)) * (1 - k / (N - 1))
    p_in, p_out = a / (N - 1), b / (N - 1)
    mean_out = b * hon + (H - 1) * p_in
    var_out = b * thin_var + (H - 1) * p_in * (1 - p_in)
    mean_in = a * hon + (H - 1) * p_out
    var_in = a * thin_var + (H - 1) * p_out * (1 - p_out)
    print(f"M5 degrees  --  N={N}, mu={mu}, (k_in,k_out)=({a},{b}), "
          f"{T} graphs (chosen: {a} in + {b} out held; honest-effective below)")
    report("in  (chosen + accepted)", ins, mean_in, var_in ** 0.5)
    report("out (chosen + accepted)", outs, mean_out, var_out ** 0.5)
    print(f"  per-graph max in/out degree: mean "
          f"{sum(maxin)/len(maxin):.1f} / {sum(maxout)/len(maxout):.1f}, "
          f"worst {max(maxin)} / {max(maxout)}")
    print(f"  compliant total degree mean = 2*(k_in+k_out) = {2*(a+b)}")


if __name__ == "__main__":
    main()
