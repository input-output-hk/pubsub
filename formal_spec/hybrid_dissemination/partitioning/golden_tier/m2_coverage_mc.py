#!/usr/bin/env python3
"""
Monte-Carlo multi-hop coverage on the M2 propagation graph (properties #3, #5,
#6 of ../../models/closed_form_analysis.md).

M2 propagation graph (directed):
  Nodes: [0,G) golden seeds, [G, G+H) regular honest, [G+H, N) adversary.
  Edges (message flows along the arrow):
    - golden push:   g -> t   for each of g's F_g random targets;
    - regular pull:  f -> j   iff regular honest j picked f as a forwarder AND
                              f is honest (golden or regular honest).
                              (adversaries are silent: they never forward.)
  Seeds hold the message at round 0: the G golden nodes (or, when G = 0, a single
  regular-honest source, node 0).  A regular honest node is COVERED iff it is
  reachable from the seeds; coverage = (reached regular honest) / H.

Key structural facts this script checks:
  * IN-degree of every regular honest node is exactly RF (it picks its own
    forwarders), so there are NO in-degree-0 nodes -> M2 has none of RandCast's
    isolated-vertex partition mode, at any N (property #5).
  * OUT-degree is Poisson(RF) (how many others picked you), so out-degree-0
    SINKS exist w.p. ~ e^-RF -- invisible to a node's own reception but capping
    propagation.  Hence single-source coverage is a giant-component fraction
    rho(RF) < 1, NOT 1: the M2 graph is NOT strongly connected for fixed RF.
    Full coverage comes from many golden seeds, not from strong connectivity.

Mean-field fixed point for coverage (property #3):
    u = q_push * ( mu + (1-mu) u )^RF,   q_push = (1 - F_g/(N-1))^G,  mu = k/N,
    coverage_fp = 1 - u   (u = P(a regular honest node is not reached)).
  Explicit for RF = 1 (linear) and RF = 2 (quadratic); iterated otherwise.

Experiments:
  (A) single source, no golden: coverage ~ giant-component rho(RF) < 1;
  (B) coverage vs the fixed point under Theta(N) golden seeds (#3);
  (C) structural isolation: 0 in-degree-0 nodes (M2) vs ~e^-F*N (RandCast) (#5);
  (D) delivery-tree depth ~ log N (#6);
  (E) no golden, RF = ceil(ln N): pure pull reaches full coverage on its own
      (out-degree-0 sinks ~ e^-RF*N -> O(1)), vs fixed RF=2 stuck at rho(2);
  (F) coverage design rule: G*F_g ~ N*ln(H*mu^RF/eps_net) gives coverage w.h.p.

Design rule for end-to-end coverage w.h.p. (from experiment F): a node is
permanently uncovered only if all RF forwarders are adversarial and no golden
pushed to it, so E[uncovered] ~ H*e^-lambda*mu^RF (the whole-network eclipse
count).  Requiring that <= eps_net gives  G*F_g >= N*ln(H*mu^RF/eps_net).  If
mu^RF < eps_net/H (RF > ln(H/eps_net)/ln(1/mu)) no golden tier is needed.

No dependencies beyond the standard library.
"""

import argparse
import math
import random
from collections import deque


# ---------------------------------------------------------------------------
# Mean-field fixed point
# ---------------------------------------------------------------------------

def q_push(N, G, Fg):
    return (1 - Fg / (N - 1)) ** G if G > 0 else 1.0


def u_iterate(qp, mu, RF, iters=10000, tol=1e-15):
    """Smallest fixed point of u = qp*(mu+(1-mu)u)^RF in [0,1], from u=0."""
    u = 0.0
    for _ in range(iters):
        nu = qp * (mu + (1 - mu) * u) ** RF
        if abs(nu - u) < tol:
            return nu
        u = nu
    return u


def u_rf1_closed(qp, mu):
    return qp * mu / (1 - qp * (1 - mu))


def u_rf2_closed(qp, mu):
    a = qp * (1 - mu) ** 2
    b = 2 * qp * mu * (1 - mu) - 1
    c = qp * mu * mu
    if a == 0:
        return -c / b
    disc = b * b - 4 * a * c
    roots = ((-b - math.sqrt(disc)) / (2 * a), (-b + math.sqrt(disc)) / (2 * a))
    return min(r for r in roots if -1e-9 <= r <= 1 + 1e-9)


# ---------------------------------------------------------------------------
# Monte-Carlo simulation of one M2 propagation graph
# ---------------------------------------------------------------------------

def simulate_once(N, G, Fg, H, k, RF, rng):
    """Return (coverage of regular honest, delivery-tree depth to them)."""
    adv_start = G + H
    is_adv = bytearray(N)
    for i in range(adv_start, N):
        is_adv[i] = 1
    adj = [[] for _ in range(N)]
    pool = range(N - 1)

    for g in range(G):                                   # golden push  g -> t
        for r in rng.sample(pool, Fg):
            adj[g].append(r if r < g else r + 1)

    for j in range(G, adv_start):                        # pull  f -> j (f honest)
        for r in rng.sample(pool, RF):
            f = r if r < j else r + 1
            if not is_adv[f]:
                adj[f].append(j)

    depth = [-1] * N
    dq = deque()
    if G > 0:
        for g in range(G):
            depth[g] = 0
            dq.append(g)
    else:
        depth[0] = 0                                     # single regular source
        dq.append(0)

    while dq:                                            # BFS from seeds
        v = dq.popleft()
        dv = depth[v] + 1
        for w in adj[v]:
            if depth[w] < 0:
                depth[w] = dv
                dq.append(w)

    reached = 0
    maxd = 0
    for j in range(G, adv_start):
        if depth[j] >= 0:
            reached += 1
            if depth[j] > maxd:
                maxd = depth[j]
    return reached / H, maxd


def simulate(N, G, Fg, k, RF, trials, rng):
    H = N - G - k
    cov = 0.0
    dep = 0.0
    covs = []
    for _ in range(trials):
        c, d = simulate_once(N, G, Fg, H, k, RF, rng)
        cov += c
        dep += d
        covs.append(c)
    mean = cov / trials
    var = sum((c - mean) ** 2 for c in covs) / (trials - 1) if trials > 1 else 0.0
    return mean, dep / trials, var, H


# ---------------------------------------------------------------------------
# Experiments
# ---------------------------------------------------------------------------

def exp_single_source(trials, rng):
    """Decisive test: G=0, mu=0, single source.  Is coverage 1 (strong
    connectivity) or a giant-component fraction rho(RF) < 1?"""
    print("=" * 78)
    print("(A) Single source, no golden, no adversary: coverage vs N and RF.")
    print("    Branching (Poisson-RF offspring) predicts survival rho(RF):")
    print("    rho = 1 - exp(-RF*rho).  Coverage should approach rho, NOT 1,")
    print("    for fixed RF -> the M2 graph is not strongly connected.")
    print("=" * 78)
    rhos = {}
    for RF in (1, 2, 3, 5):
        r = 0.5
        for _ in range(200):
            r = 1 - math.exp(-RF * r)
        rhos[RF] = r
    print(f"  branching rho(RF): " +
          "  ".join(f"RF={rf}:{rhos[rf]:.4f}" for rf in (1, 2, 3, 5)))
    print()
    print(f"{'N':>7} " + " ".join(f"{'RF='+str(rf):>10}" for rf in (1, 2, 3, 5)))
    for N in (500, 1000, 2000, 4000, 8000):
        row = [f"{N:>7}"]
        for RF in (1, 2, 3, 5):
            mean, _, _, _ = simulate(N, 0, 0, 0, RF, trials, rng)
            row.append(f"{mean:>10.4f}")
        print(" ".join(row))
    print()


def exp_fixed_point(trials, rng):
    """Validate the coverage fixed point across mu, with golden seeds."""
    print("=" * 78)
    print("(B) Coverage vs mean-field fixed point (N=4000, G=40, F_g=200).")
    print("    coverage_fp = 1 - u,  u = q_push*(mu+(1-mu)u)^RF.")
    print("=" * 78)
    N, G, Fg = 4000, 40, 200
    qp = q_push(N, G, Fg)
    print(f"  q_push = {qp:.4f}  (lambda_push = {G*Fg/N:.2f})")
    for RF in (1, 2):
        print(f"  RF = {RF}:")
        print(f"    {'mu':>6} {'cov (MC)':>10} {'cov (fp)':>10} "
              f"{'cov (closed)':>13} {'diff':>9}")
        for mu in (0.0, 0.2, 0.4, 0.6, 0.8):
            k = int(round(mu * N))
            mean, _, _, _ = simulate(N, G, Fg, k, RF, trials, rng)
            u_it = u_iterate(qp, mu, RF)
            u_cl = u_rf1_closed(qp, mu) if RF == 1 else u_rf2_closed(qp, mu)
            print(f"    {mu:>6.2f} {mean:>10.4f} {1-u_it:>10.4f} "
                  f"{1-u_cl:>13.4f} {mean-(1-u_it):>+9.4f}")
    print()


def simulate_randcast_once(N, F, rng):
    """RandCast push: every node pushes to F random targets; source = node 0.
    Coverage = fraction of all nodes reachable from the source."""
    adj = [[] for _ in range(N)]
    pool = range(N - 1)
    for i in range(N):
        for r in rng.sample(pool, F):
            adj[i].append(r if r < i else r + 1)
    seen = bytearray(N)
    seen[0] = 1
    dq = deque([0])
    while dq:
        v = dq.popleft()
        for w in adj[v]:
            if not seen[w]:
                seen[w] = 1
                dq.append(w)
    return sum(seen) / N


def count_isolated(N, G, Fg, k, RF, rng, model):
    """# nodes with honest in-degree 0 (structurally unreachable from any seed).
    model='m2': regular honest nodes; 'randcast': all nodes."""
    H = N - G - k
    adv_start = G + H
    is_adv = bytearray(N)
    for i in range(adv_start, N):
        is_adv[i] = 1
    pool = range(N - 1)
    indeg = [0] * N
    if model == "m2":
        for g in range(G):
            for r in rng.sample(pool, Fg):
                indeg[r if r < g else r + 1] += 1        # golden push in-edge
        for j in range(G, adv_start):
            for r in rng.sample(pool, RF):
                f = r if r < j else r + 1
                if not is_adv[f]:
                    indeg[j] += 1                        # honest pull forwarder
        return sum(1 for j in range(G, adv_start) if indeg[j] == 0)
    for i in range(N):                                   # randcast push
        for r in rng.sample(pool, RF):
            indeg[r if r < i else r + 1] += 1
    return sum(1 for j in range(N) if indeg[j] == 0)


def exp_coverage_vs_N(trials, rng):
    """Structural isolation and coverage vs N.  The 'ln N threshold' is about
    in-degree-0 nodes: RandCast push has ~ e^-F * N of them (count GROWS with
    N -> needs F ~ ln N); M2 pull has ZERO (deterministic in-degree RF).  M2
    coverage under Theta(N) golden seeds is N-invariant (no collapse)."""
    print("=" * 78)
    print("(C) Structural isolation (in-degree-0 count, mu=0) and M2 coverage")
    print("    vs N.  M2 pull: 0 isolated at any N.  RandCast push (F=2): "
          "~e^-2*N.")
    print("=" * 78)
    G, Fg_lam, RF = 20, 0.5, 2
    print(f"{'N':>7} {'M2 iso':>7} {'RandCast iso':>13} {'e^-2*N':>9} "
          f"| {'M2 cov(MC)':>11} {'cov(fp)':>9} {'depth':>6}  (mu=0.1)")
    for N in (500, 1000, 2000, 4000, 8000, 16000):
        Fg = max(1, round(Fg_lam * N / G))
        t = trials if N <= 4000 else max(30, trials // 3)
        # structural isolation at mu = 0
        m2_iso = sum(count_isolated(N, G, Fg, 0, RF, rng, "m2")
                     for _ in range(t)) / t
        rc_iso = sum(count_isolated(N, 0, 0, 0, 2, rng, "randcast")
                     for _ in range(t)) / t
        # coverage with adversary (mu = 0.1) and Theta(N) golden seeds
        k = int(round(0.1 * N))
        cov, dep, _, _ = simulate(N, G, Fg, k, RF, t, rng)
        cov_fp = 1 - u_iterate(q_push(N, G, Fg), 0.1, RF)
        print(f"{N:>7} {m2_iso:>7.1f} {rc_iso:>13.1f} {math.exp(-2)*N:>9.1f} "
              f"| {cov:>11.4f} {cov_fp:>9.4f} {dep:>6.2f}")
    print()


def exp_depth(trials, rng):
    """Delivery-tree depth (#6).  Depth grows ~ log N in both cases; Theta(N)
    golden seeding shifts the tree shallower by a constant (more roots), it
    does not make depth N-independent."""
    print("=" * 78)
    print("(D) Delivery depth vs N (~ log N).  Golden seeding (G=20, F_g=N/40)")
    print("    gives a shallower tree than a single source, same log growth.")
    print("=" * 78)
    print(f"{'N':>7} {'log_3 N':>8} {'depth (1 src, RF=3)':>20} "
          f"{'depth (golden seeds)':>21}")
    for N in (500, 1000, 2000, 4000, 8000):
        t = trials if N <= 4000 else max(30, trials // 3)
        _, dep_src, _, _ = simulate(N, 0, 0, 0, 3, t, rng)
        Fg = max(1, round(0.5 * N / 20))
        _, dep_gold, _, _ = simulate(N, 20, Fg, int(0.1 * N), 2, t, rng)
        print(f"{N:>7} {math.log(N)/math.log(3):>8.2f} {dep_src:>20.2f} "
              f"{dep_gold:>21.2f}")
    print()


def simulate_no_golden_once(N, k, RF, rng):
    """Pure pull, no golden, single source = node 0.  Layout: [0,H) regular
    honest (0 is the source), [H,N) adversary.  Returns
    (coverage of honest, depth, # honest out-degree-0 sinks)."""
    H = N - k
    is_adv = bytearray(N)
    for i in range(H, N):
        is_adv[i] = 1
    adj = [[] for _ in range(N)]
    pool = range(N - 1)
    for j in range(H):                                   # pull f -> j (f honest)
        for r in rng.sample(pool, RF):
            f = r if r < j else r + 1
            if not is_adv[f]:
                adj[f].append(j)
    depth = [-1] * N
    depth[0] = 0
    dq = deque([0])
    while dq:
        v = dq.popleft()
        dv = depth[v] + 1
        for w in adj[v]:
            if depth[w] < 0:
                depth[w] = dv
                dq.append(w)
    reached = maxd = 0
    for j in range(H):
        if depth[j] >= 0:
            reached += 1
            if depth[j] > maxd:
                maxd = depth[j]
    sinks = sum(1 for v in range(H) if not adj[v])       # honest, forward to none
    return reached / H, maxd, sinks


def exp_no_golden(trials, rng):
    """Pure pull (no golden), single source, RF = ceil(ln N).  In-degree is
    always RF (no in-degree-0), so the only obstacle to full coverage is
    Poisson(RF) OUT-degree-0 sinks (~ e^-RF * N).  At RF ~ ln N that is O(1),
    so coverage -> 1 (near strong connectivity) -- unlike fixed RF=2, stuck at
    the giant-component fraction rho(2) ~ 0.80."""
    print("=" * 78)
    print("(E) No golden, single source, RF = ceil(ln N) -- can pure pull stand")
    print("    alone?  Contrast with fixed RF=2 (stuck at giant component).")
    print("=" * 78)
    print(f"{'N':>7} {'RF':>4} {'cov mu=0':>9} {'cov mu=0.1':>11} "
          f"{'sinks':>7} {'e^-RF*N':>8} {'depth':>6} | {'cov RF=2':>9}")
    for N in (500, 1000, 2000, 4000, 8000, 16000):
        RF = math.ceil(math.log(N))
        t = trials if N <= 4000 else max(30, trials // 3)
        c0 = d0 = s0 = 0.0
        for _ in range(t):
            c, d, s = simulate_no_golden_once(N, 0, RF, rng)
            c0 += c; d0 += d; s0 += s
        k = int(round(0.1 * N))
        c1 = sum(simulate_no_golden_once(N, k, RF, rng)[0] for _ in range(t)) / t
        c2 = sum(simulate_no_golden_once(N, 0, 2, rng)[0] for _ in range(t)) / t
        print(f"{N:>7} {RF:>4} {c0/t:>9.4f} {c1:>11.4f} {s0/t:>7.2f} "
              f"{math.exp(-RF)*N:>8.3f} {d0/t:>6.2f} | {c2:>9.4f}")
    print()


def exp_coverage_design(trials, rng):
    """Design rule for end-to-end coverage w.h.p.  A node with all RF forwarders
    adversarial is reachable only by golden push, so E[uncovered] ~ whole-network
    eclipse count H*e^-lambda*mu^RF.  Setting that to eps_net and solving:
        G*F_g >= N * ln( H*mu^RF / eps_net ).
    For each (mu, RF, G) we pick F_g from the rule (eps_net=1) and check the
    simulated uncovered count lands near 1."""
    print("=" * 78)
    print("(F) Coverage design rule: F_g = ceil( (N/G)*ln(H*mu^RF) ), target")
    print("    E[uncovered] ~ 1.  Verifies G*F_g ~ N*ln(H*mu^RF) gives full")
    print("    coverage w.h.p.  (N=8000, eps_net=1)")
    print("=" * 78)
    N = 8000
    print(f"{'mu':>5} {'RF':>3} {'G':>5} {'lambda*':>8} {'F_g':>6} "
          f"{'miss (MC)':>10} {'target':>7}")
    for mu, RF in ((0.1, 2), (0.2, 2), (0.1, 3), (0.15, 3)):
        k = int(round(mu * N))
        H = N - k
        lam = math.log(H * mu ** RF)                     # eps_net = 1
        if lam <= 0:
            print(f"{mu:>5} {RF:>3}   ln(H*mu^RF) = {lam:.2f} <= 0  ->  "
                  f"no golden needed (eclipse floor already < 1 node)")
            continue
        for G in (20, 100):
            Fg = math.ceil(lam * N / G)
            H2 = N - G - k
            cov, _, _, _ = simulate(N, G, Fg, k, RF, trials, rng)
            print(f"{mu:>5} {RF:>3} {G:>5} {lam:>8.2f} {Fg:>6} "
                  f"{(1 - cov) * H2:>10.1f} {'~1':>7}")
    print()


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[1])
    parser.add_argument("--trials", type=int, default=200)
    parser.add_argument("--seed", type=int, default=12345)
    args = parser.parse_args()
    rng = random.Random(args.seed)

    exp_single_source(args.trials, rng)
    exp_fixed_point(args.trials, rng)
    exp_coverage_vs_N(args.trials, rng)
    exp_depth(args.trials, rng)
    exp_no_golden(args.trials, rng)
    exp_coverage_design(args.trials, rng)


if __name__ == "__main__":
    main()
