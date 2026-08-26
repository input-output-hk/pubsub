#!/usr/bin/env python3
"""M3 node degrees: honest in-/out-degree distributions vs closed forms at
the operating point.  Backs ../properties/node_degrees.md.

in  = own RF forwarder picks (chosen, thinned) + initiation links of others
      targeting me (accepted ~ Binomial(H-1, (s-1)/(N-1)));
out = honest requesters (accepted ~ Binomial(H-1, RF/(N-1))) + own s-1
      initiation links (chosen, thinned).
"""
import argparse, random
from m3_model import M3Params, M3Graph


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
    ap.add_argument("--RF", type=int, default=13)
    ap.add_argument("--s", type=int, default=7)
    ap.add_argument("--trials", type=int, default=25)
    ap.add_argument("--seed", type=int, default=2024)
    args = ap.parse_args()
    N, mu, RF, s, T = args.N, args.mu, args.RF, args.s, args.trials
    k = int(round(mu * N)); H = N - k
    rng = random.Random(args.seed)

    pull_in, req_out, init_out, init_in, maxtot = [], [], [], [], []
    for _ in range(T):
        g = M3Graph(M3Params(N=N, k=k, RF=RF, s=s), rng)
        reqdeg = [0] * H
        initdeg = [0] * H
        for j in range(H):
            pull_in.append(sum(1 for f in g.picks_of(j) if f < H))
            for f in g.picks_of(j):
                if f < H:
                    reqdeg[f] += 1
            init_out.append(len(g.init_targets[j]))
            for t in g.init_targets[j]:
                initdeg[t] += 1
        req_out.extend(reqdeg)
        init_in.extend(initdeg)
        maxtot.append(max(reqdeg[v] + initdeg[v] for v in range(H)))
    pr, pi = RF / (N - 1), (s - 1) / (N - 1)
    print(f"M3 degrees  --  N={N}, mu={mu}, RF={RF}, s={s}, {T} graphs "
          f"(chosen: {RF} in + {s-1} out held; honest-effective below)")
    report("in: forwarders (chosen)", pull_in, RF * (1 - k / (N - 1)),
           (RF * (k / (N - 1)) * (1 - k / (N - 1))) ** 0.5)
    report("in: initiation (accepted)", init_in, (H - 1) * pi,
           ((H - 1) * pi * (1 - pi)) ** 0.5)
    report("out: requesters (accepted)", req_out, (H - 1) * pr,
           ((H - 1) * pr * (1 - pr)) ** 0.5)
    report("out: initiation (chosen)", init_out, (s - 1) * (1 - k / (N - 1)),
           ((s - 1) * (k / (N - 1)) * (1 - k / (N - 1))) ** 0.5)
    print(f"  per-graph max accepted (requesters+initiation): mean "
          f"{sum(maxtot)/len(maxtot):.1f}, worst {max(maxtot)}")
    print(f"  compliant total degree mean = 2*(RF+s-1) = {2*(RF+s-1)}")


if __name__ == "__main__":
    main()
