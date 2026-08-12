#!/usr/bin/env python3
"""Cross-model validation suite: simulator-vs-simulator checks.

The per-model self-tests validate each closed-form LAW against its own
simulator.  This suite validates the SIMULATORS against each other and
against independent algorithms, closing the loop a shared bug could hide in:

  (1) Boundary identities [exact]: M5(k_in=RF, k_out=0) IS M2 and
      M5(k_in=0, k_out=F) IS M1 -- the defect closed forms must agree to
      machine precision.
  (2) Distributional equivalence [z-test]: the same boundary pairs, compared
      as independent Monte-Carlo P(bad) estimates from the two samplers.
  (3) M3 strict_bad fast path vs BRUTE FORCE (BFS from every publisher's
      seed set) on small graphs [exact, per graph], including subcritical
      cells that exercise the no-giant-SCC fallback.
  (4) is_bad vs independent algorithms [exact, per graph]: Kosaraju SCC
      count for the directed models (M1, M5), union-find connectivity for
      the undirected M4.
  (5) Flood counters (sends / depths / coverage) vs an independent
      priority-queue reference simulator [exact, per graph], for the cost
      sweeps of M1, M2/M3, M4, M5.
  (6) Re-provisioning searches vs the published operating points [exact]:
      each sweep_m?_reprovision.py parameter search at mu = 0.2 must
      reproduce comparison.md table 1 (F=24, RF=24, (12,8), RF=8, (9,8)).
  (7) min-cut = honest degree [exact, per graph]: the adaptive-eclipse-cost
      property prices an attack at the victim's degree, which is only sound
      if no cheaper cut sits deeper.  Max-flow (Menger) on the weakest node
      in both attack directions, all five models, at reduced fanouts where
      a deeper cut would most plausibly win.

Run `python3 validate.py` (~ a few minutes).  `--tail {m3,m5}` runs a
deep-tail law validation cell instead (tens of minutes; see bottom).
Exit code 0 iff all checks pass.  Stdlib only.
"""

import argparse
import math
import os
import random
import sys
from collections import deque
from heapq import heappush, heappop

_HERE = os.path.dirname(os.path.abspath(__file__))
for _m in ("m1", "m2", "m3", "m4", "m5"):
    sys.path.insert(0, os.path.join(_HERE, _m, "scripts"))

from m1_model import M1Params, M1Graph                    # noqa: E402
from m2_model import M2Params, M2Graph                    # noqa: E402
from m3_model import M3Params, M3Graph                    # noqa: E402
from m4_model import M4Params, M4Graph                    # noqa: E402
from m5_model import M5Params, M5Graph                    # noqa: E402

FAILURES = []


def check(name, ok, detail=""):
    print(f"  [{'ok  ' if ok else 'FAIL'}] {name}"
          + (f"  ({detail})" if detail else ""))
    if not ok:
        FAILURES.append(name)


# ---------------------------------------------------------------------------
# (1) boundary identities of the closed forms
# ---------------------------------------------------------------------------

def check_boundary_identities():
    print("(1) closed-form boundary identities [exact]")
    for (N, k, RF) in [(2000, 400, 12), (20000, 4000, 24), (500, 250, 3)]:
        m5 = M5Params(N=N, k=k, k_in=RF, k_out=0)
        m2 = M2Params(N=N, k=k, RF=RF)
        q_noreq = (1 - RF / (N - 1)) ** (m2.H - 1)
        check(f"M5({RF},0) == M2 in/out terms, N={N}",
              m5.p_in_isolated() == m2.q_pull()
              and m5.p_out_isolated() == q_noreq)
    for (N, k, F) in [(2000, 400, 10), (20000, 4000, 24)]:
        m5 = M5Params(N=N, k=k, k_in=0, k_out=F)
        m1 = M1Params(N=N, k=k, F=F)
        check(f"M5(0,{F}) == M1 in/out terms, N={N}",
              m5.p_in_isolated() == m1.p_in_isolated()
              and m5.p_out_isolated() == m1.p_out_isolated()
              and m5.p_bad() == m1.p_bad())


# ---------------------------------------------------------------------------
# (2) distributional equivalence of the samplers at the boundaries
# ---------------------------------------------------------------------------

def _ztest(bad1, bad2, T):
    p1, p2 = bad1 / T, bad2 / T
    pool = (bad1 + bad2) / (2 * T)
    se = math.sqrt(max(pool * (1 - pool) * 2 / T, 1e-12))
    return (p1 - p2) / se


def check_distributional(rng, T=800):
    print("(2) sampler equivalence at the boundaries [|z| <= 4]")
    # M5(RF,0) vs M2 (strong connectivity via M3Graph.strict_bad at s=1)
    N, k, RF = 2000, 400, 12
    bad5 = sum(M5Graph(M5Params(N=N, k=k, k_in=RF, k_out=0), rng).is_bad()
               for _ in range(T))
    bad2 = sum(M3Graph(M2Params(N=N, k=k, RF=RF), rng).strict_bad()
               for _ in range(T))
    z = _ztest(bad5, bad2, T)
    check(f"M5({RF},0) vs M2: {bad5}/{T} vs {bad2}/{T}", abs(z) <= 4,
          f"z={z:+.2f}")
    # M5(0,F) vs M1
    N, k, F = 2000, 400, 10
    bad5 = sum(M5Graph(M5Params(N=N, k=k, k_in=0, k_out=F), rng).is_bad()
               for _ in range(T))
    bad1 = sum(M1Graph(M1Params(N=N, k=k, F=F), rng).is_bad()
               for _ in range(T))
    z = _ztest(bad5, bad1, T)
    check(f"M5(0,{F}) vs M1: {bad5}/{T} vs {bad1}/{T}", abs(z) <= 4,
          f"z={z:+.2f}")


# ---------------------------------------------------------------------------
# (3) M3 strict_bad fast path vs brute force
# ---------------------------------------------------------------------------

def _brute_strict_bad(g):
    """Reference: explicit BFS from every publisher's seed set."""
    H = g.params.H
    adj = g.adjacency()
    for p in range(H):
        seen = bytearray(H)
        dq = deque()
        for v in [p] + g.init_targets[p]:
            if not seen[v]:
                seen[v] = 1
                dq.append(v)
        n = sum(seen)
        while dq:
            for w in adj[dq.popleft()]:
                if not seen[w]:
                    seen[w] = 1
                    n += 1
                    dq.append(w)
        if n < H:
            return True
    return False


def check_strict_bad_brute(rng, graphs=120):
    print("(3) M3 strict_bad fast path vs brute force [exact per graph]")
    for (RF, s) in [(4, 3), (2, 2), (6, 1), (3, 4)]:
        p = M3Params(N=400, k=80, RF=RF, s=s)
        mism = bads = 0
        for _ in range(graphs):
            g = M3Graph(p, rng)
            fast, brute = g.strict_bad(), _brute_strict_bad(g)
            bads += brute
            mism += (fast != brute)
        check(f"RF={RF} s={s}: {graphs} graphs ({bads} bad)", mism == 0,
              f"{mism} mismatches")


# ---------------------------------------------------------------------------
# (4) is_bad vs independent algorithms
# ---------------------------------------------------------------------------

def _kosaraju_sc(adj, n):
    """True iff the digraph on [0,n) is strongly connected (Kosaraju)."""
    order = []
    seen = bytearray(n)
    for s in range(n):
        if seen[s]:
            continue
        stack = [(s, iter(adj[s]))]
        seen[s] = 1
        while stack:
            v, it = stack[-1]
            adv = False
            for w in it:
                if not seen[w]:
                    seen[w] = 1
                    stack.append((w, iter(adj[w])))
                    adv = True
                    break
            if not adv:
                order.append(v)
                stack.pop()
    radj = [[] for _ in range(n)]
    for v in range(n):
        for w in adj[v]:
            radj[w].append(v)
    seen = bytearray(n)
    comps = 0
    for s in reversed(order):
        if seen[s]:
            continue
        comps += 1
        if comps > 1:
            return False
        seen[s] = 1
        dq = deque([s])
        while dq:
            for w in radj[dq.popleft()]:
                if not seen[w]:
                    seen[w] = 1
                    dq.append(w)
    return comps == 1


def _unionfind_connected(adj, n):
    parent = list(range(n))

    def find(x):
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    for v in range(n):
        for w in adj[v]:
            rv, rw = find(v), find(w)
            if rv != rw:
                parent[rv] = rw
    root = find(0)
    return all(find(v) == root for v in range(n))


def check_isbad_independent(rng, graphs=100):
    print("(4) is_bad vs independent algorithms [exact per graph]")
    p1 = M1Params(N=1000, k=200, F=9)
    mism = bads = 0
    for _ in range(graphs):
        g = M1Graph(p1, rng)
        bad, ref = g.is_bad(), not _kosaraju_sc(g.adj, p1.H)
        bads += ref
        mism += (bad != ref)
    check(f"M1 vs Kosaraju: {graphs} graphs ({bads} bad)", mism == 0,
          f"{mism} mismatches")

    p5 = M5Params(N=1000, k=200, k_in=4, k_out=4)
    mism = bads = 0
    for _ in range(graphs):
        g = M5Graph(p5, rng)
        bad, ref = g.is_bad(), not _kosaraju_sc(g.adj, p5.H)
        bads += ref
        mism += (bad != ref)
    check(f"M5 vs Kosaraju: {graphs} graphs ({bads} bad)", mism == 0,
          f"{mism} mismatches")

    p4 = M4Params(N=1000, k=200, RF=3)
    mism = bads = 0
    for _ in range(graphs):
        g = M4Graph(p4, rng)
        bad, ref = g.is_bad(), not _unionfind_connected(g.adj, p4.H)
        bads += ref
        mism += (bad != ref)
    check(f"M4 vs union-find: {graphs} graphs ({bads} bad)", mism == 0,
          f"{mism} mismatches")


# ---------------------------------------------------------------------------
# (5) flood counters vs an independent reference simulator
# ---------------------------------------------------------------------------

def _reference_disseminate(adj, seed_depths):
    """Priority-queue (Dijkstra-style) dissemination: nodes fire in
    nondecreasing first-receipt depth, ties broken by arrival order (the
    "arrival link" is the FIRST arrival, as in the models); a firing node
    sends to every out-neighbour except its arrival parent.  Returns
    (sends, max depth, mean depth, reached)."""
    n = len(adj)
    depth = [-1] * n
    parent = [-1] * n
    heap = []
    tick = 0
    for v, d in seed_depths:
        if depth[v] < 0:
            depth[v] = d
            heappush(heap, (d, tick, v))
            tick += 1
    sends = 0
    fired = bytearray(n)
    reached = []
    while heap:
        d, _, v = heappop(heap)
        if fired[v]:
            continue
        fired[v] = 1
        reached.append(d)
        for w in adj[v]:
            if w == parent[v]:
                continue
            sends += 1
            if depth[w] < 0:
                depth[w] = d + 1
                parent[w] = v
                heappush(heap, (d + 1, tick, w))
                tick += 1
    return sends, max(reached), sum(reached) / len(reached), len(reached)


def _flood_equal(a, r):
    """Compare (sends, max depth, mean depth, reached): integers exactly,
    the mean up to float summation order."""
    return (a[0] == r[0] and a[1] == r[1] and a[3] == r[3]
            and math.isclose(a[2], r[2], rel_tol=1e-12))


def check_flood_reference(rng, graphs=25):
    print("(5) flood counters vs reference simulator [exact per graph]")
    import sweep_m1_cost, sweep_m2_cost, sweep_m4_cost, sweep_m5_cost
    import sweep_m3_cost

    p = M1Params(N=1500, k=300, F=12)
    mism = sum(not _flood_equal(sweep_m1_cost.flood(g.adj, 0),
                                _reference_disseminate(g.adj, [(0, 0)]))
               for g in (M1Graph(p, rng) for _ in range(graphs)))
    check(f"M1 sweep flood ({graphs} graphs)", mism == 0, f"{mism} mismatches")

    p = M2Params(N=1500, k=300, RF=14)
    mism = 0
    for _ in range(graphs):
        g = M3Graph(p, rng)
        adj = g.adjacency()
        a = sweep_m2_cost.flood(adj, 0, p.H)
        r = _reference_disseminate(adj, [(0, 0)])
        mism += not _flood_equal(a, r)
    check(f"M2 sweep flood ({graphs} graphs)", mism == 0, f"{mism} mismatches")

    pp = M3Params(N=1500, k=300, RF=12, s=5)
    mism = 0
    for _ in range(graphs):
        g = M3Graph(pp, rng)
        adj = g.adjacency()
        seeds = [(0, 0)] + [(t, 1) for t in g.init_targets[0]]
        a = sweep_m3_cost.flood_seeded(adj, seeds, pp.H)
        r = _reference_disseminate(adj, seeds)
        mism += not _flood_equal(a, r)
    check(f"M3 seeded flood ({graphs} graphs)", mism == 0, f"{mism} mismatches")

    p = M4Params(N=1500, k=300, RF=6)
    mism = sum(not _flood_equal(sweep_m4_cost.flood(g.adj, 0),
                                _reference_disseminate(g.adj, [(0, 0)]))
               for g in (M4Graph(p, rng) for _ in range(graphs)))
    check(f"M4 sweep flood ({graphs} graphs)", mism == 0, f"{mism} mismatches")

    p = M5Params(N=1500, k=300, k_in=6, k_out=6)
    mism = sum(not _flood_equal(sweep_m5_cost.flood(g.adj, 0),
                                _reference_disseminate(g.adj, [(0, 0)]))
               for g in (M5Graph(p, rng) for _ in range(graphs)))
    check(f"M5 sweep flood ({graphs} graphs)", mism == 0, f"{mism} mismatches")


# ---------------------------------------------------------------------------
# (6) re-provisioning searches vs the published mu = 0.2 operating points
# ---------------------------------------------------------------------------

def check_reprovision_anchor():
    print("(6) re-provisioning searches vs published mu = 0.2 points [exact]")
    import sweep_m1_reprovision as r1
    import sweep_m2_reprovision as r2
    import sweep_m3_reprovision as r3
    import sweep_m4_reprovision as r4
    import sweep_m5_reprovision as r5
    check("M1 search(0.2) == F = 24", r1.search(0.2) == 24,
          f"got {r1.search(0.2)}")
    check("M2 search(0.2) == RF = 24", r2.search(0.2) == 24,
          f"got {r2.search(0.2)}")
    B3, fs3 = r3.search(0.2)
    check("M3 search(0.2) == budget 19, bw-minimal (12, 8)",
          B3 == 19 and fs3[0] == (12, 8), f"got B={B3}, {fs3[:1]}")
    check("M4 search(0.2) == RF = 8", r4.search(0.2) == 8,
          f"got {r4.search(0.2)}")
    B5, fs5 = r5.search(0.2)
    check("M5 search(0.2) == budget 17, balanced (9, 8)",
          B5 == 17 and fs5[0] == (9, 8), f"got B={B5}, {fs5[:1]}")


# ---------------------------------------------------------------------------
# deep-tail law validation (long runs, invoked explicitly)
# ---------------------------------------------------------------------------

def tail_m3(rng, trials=30000):
    """M3 law at E ~ 5e-3: N=4000, mu=0.2, (RF=9, s=5)."""
    p = M3Params(N=4000, k=800, RF=9, s=5)
    bad = sum(M3Graph(p, rng).strict_bad() for _ in range(trials))
    _report_tail("M3 (RF=9, s=5)", p.p_bad(), bad, trials)


def tail_m5(rng, trials=50000):
    """M5 law at E ~ 1e-3: N=4000, mu=0.2, (k_in=6, k_out=7)."""
    p = M5Params(N=4000, k=800, k_in=6, k_out=7)
    bad = sum(M5Graph(p, rng).is_bad() for _ in range(trials))
    _report_tail("M5 (6,7)", p.p_bad(), bad, trials)


def _report_tail(label, pred, bad, trials):
    mc = bad / trials
    se = math.sqrt(max(mc, 1 / trials) * (1 - mc) / trials)
    z = (mc - pred) / se if se > 0 else float("nan")
    print(f"{label}: pred {pred:.3e}  MC {mc:.3e}  ({bad}/{trials})  "
          f"z={z:+.2f}  {'ok' if abs(z) <= 4 else 'FAIL'}")


# ---------------------------------------------------------------------------
# (6) min-cut = degree, by max-flow (Menger), in both attack directions
# ---------------------------------------------------------------------------

_INF = 10 ** 9


def _max_disjoint_paths(out_adj, H, target, cap):
    """Max vertex-disjoint paths from every other honest node into `target`.

    Node-splitting (v_in = 2v, v_out = 2v+1) with unit capacity between the
    halves, so a path routed through v spends the single corruption it takes
    to remove v.  By Menger the value equals the minimum vertex cut
    separating `target` from the rest of the honest graph.
    """
    from collections import defaultdict
    S, T = 2 * H, 2 * target
    g = defaultdict(lambda: defaultdict(int))
    for v in range(H):
        if v != target:
            g[2 * v][2 * v + 1] += 1
            g[S][2 * v] += 1
    g[2 * target][2 * target + 1] += _INF
    for v in range(H):
        for w in out_adj[v]:
            if w < H:
                g[2 * v + 1][2 * w] += _INF
    flow = 0
    while flow <= cap:
        parent, dq, found = {S: None}, deque([S]), False
        while dq and not found:
            u = dq.popleft()
            for v, c in list(g[u].items()):
                if c > 0 and v not in parent:
                    parent[v] = u
                    if v == T:
                        found = True
                        break
                    dq.append(v)
        if not found:
            break
        v = T
        while parent[v] is not None:
            u = parent[v]
            g[u][v] -= 1
            g[v][u] += 1
            v = u
        flow += 1
    return flow


def _rev(out_adj, H):
    r = [[] for _ in range(H)]
    for v in range(H):
        for w in out_adj[v]:
            if w < H:
                r[w].append(v)
    return r


def _in_out_degrees(out_adj, H):
    outd = [len({w for w in out_adj[v] if w < H}) for v in range(H)]
    ind = [0] * H
    for v in range(H):
        for w in {w for w in out_adj[v] if w < H}:
            ind[w] += 1
    return ind, outd


def check_mincut_equals_degree(rng, N=2000, graphs=3):
    """The adaptive-eclipse-cost property prices an attack at the victim's
    honest degree.  That is only sound if no cheaper cut sits deeper in the
    graph -- a depth->=2 shell could in principle be smaller.  Verify by
    max-flow on the WEAKEST node in each direction (the one an adaptive
    adversary picks), for every model.

    KNOWN LIMITATION (PR #140 review): with every honest node a source, the
    victim's neighbours are sources themselves, so the flow equals the
    degree on any graph and a cheaper deeper cut cannot surface.  Until the
    sources are restricted to distant publishers this pins the max-flow
    implementation against the degree count; the property's min-cut = degree
    claim rests on the shell argument in the property docs.

    Run at reduced fanouts: low degrees are where a deeper cut would most
    plausibly win, so this stresses the claim harder than the operating point
    does.  The fanouts are chosen so the weakest node still has degree >= 1 --
    a degree-0 node has min-cut 0 trivially and would make the check pass
    vacuously, so `_weakest` skips them and the assertion below fails loudly
    if a graph offers nothing but isolated nodes.
    """
    print("\n(7) min-cut = honest degree, by max-flow (both directions)")
    k = int(0.2 * N)
    H = N - k

    def m1(r):
        g = M1Graph(M1Params(N=N, k=k, F=15), r)
        return g.adj, g.adj

    def m2(r):
        g = M2Graph(M2Params(N=N, k=k, RF=15), r)
        adj = [[] for _ in range(H)]
        for j in range(H):
            for f in g.picks_of(j):
                if f < H:
                    adj[f].append(j)
        return adj, adj

    def m3(r):
        g = M3Graph(M3Params(N=N, k=k, RF=10, s=4), r)
        relay = [list(x) for x in g.adjacency()[:H]]
        mute = [list(x) for x in relay]
        for j in range(H):
            mute[j].extend(g.init_targets[j])   # extra seeds, never relay
        return relay, mute

    def m4(r):
        g = M4Graph(M4Params(N=N, k=k, RF=8), r)
        return g.adj, g.adj

    def m5(r):
        g = M5Graph(M5Params(N=N, k=k, k_in=8, k_out=8), r)
        return g.adj, g.adj

    def _weakest(deg):
        """Lowest-degree node with degree >= 1 (degree 0 tests nothing)."""
        cands = [v for v in range(H) if deg[v] >= 1]
        return min(cands, key=lambda x: deg[x]) if cands else None

    for label, build in (("M1", m1), ("M2", m2), ("M3", m3),
                         ("M4", m4), ("M5", m5)):
        ok, detail = True, []
        for _ in range(graphs):
            in_adj, out_adj = build(rng)
            ind, _ = _in_out_degrees(in_adj, H)
            v = _weakest(ind)
            if v is None:
                ok = False
                detail.append("deafen VACUOUS")
            else:
                cut = _max_disjoint_paths(in_adj, H, v, ind[v])
                ok &= cut == ind[v]
                detail.append(f"deafen {ind[v]}={cut}")

            _, outd = _in_out_degrees(out_adj, H)
            u = _weakest(outd)
            if u is None:
                ok = False
                detail.append("mute VACUOUS")
            else:
                cut2 = _max_disjoint_paths(_rev(out_adj, H), H, u, outd[u])
                ok &= cut2 == outd[u]
                detail.append(f"mute {outd[u]}={cut2}")
        check(f"{label}: weakest-node min-cut equals its honest degree",
              ok, ", ".join(detail))


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--seed", type=int, default=20260709)
    ap.add_argument("--tail", choices=("m3", "m5"),
                    help="run a deep-tail law validation cell instead")
    args = ap.parse_args()
    rng = random.Random(args.seed)

    if args.tail == "m3":
        return tail_m3(rng) or 0
    if args.tail == "m5":
        return tail_m5(rng) or 0

    print("cross-model validation suite")
    print("=" * 70)
    check_boundary_identities()
    check_distributional(rng)
    check_strict_bad_brute(rng)
    check_isbad_independent(rng)
    check_flood_reference(rng)
    check_reprovision_anchor()
    check_mincut_equals_degree(rng)
    print("=" * 70)
    if FAILURES:
        print(f"RESULT: {len(FAILURES)} FAILURE(S): " + "; ".join(FAILURES))
        return 1
    print("RESULT: PASS -- samplers, checkers and counters mutually consistent")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
