#!/usr/bin/env python3
"""The M3 dissemination model: M2's pull graph + standing initiation links.

M3 (see ../README.md) = M2's pull relaying reused verbatim, plus s-1 STANDING
initiation links per node (fixed uniform targets, drawn once per epoch): a
publisher seeds each of its own messages through its initiation links; the
links never relay other nodes' messages.  The *relay graph* is M2's graph, so
this module imports M2Graph from ../../m2/scripts/m2_model.py and extends it:

    M3Params(M2Params) adds s and the strict-criterion closed forms
                       p_in_isolated / p_out_isolated / E_defects / p_bad
    M3Graph(M2Graph)   adds the initiation targets and reachability
                       observables: adjacency, depths (BFS), coverage,
                       delivery_depth, strict_bad (every message of every
                       publisher reaches everyone)
    u_iterate,         the coverage mean-field: u = P(a node is never
    u_rf2_closed       reached), solving u = (mu+(1-mu)u)^RF (smallest root),
                       plus its explicit RF=2 root
    rho_giant          branching-process survival (per-seed ignition)

Run `python3 m3_model.py` for a self-test.  No dependencies beyond stdlib.
"""

from __future__ import annotations

import math
import os
import random
import sys
from collections import deque
from dataclasses import dataclass
from typing import List, Optional, Sequence

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                "..", "..", "m2", "scripts"))
from m2_model import M2Params, M2Graph, mean_var, sample_graphs  # noqa: E402


# ---------------------------------------------------------------------------
# Parameters: M2's pull fanout + the standing initiation-link count
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class M3Params(M2Params):
    """M3 parameter set: RF pull links + s-1 standing initiation links.

    Strict full-coverage criterion (every message of every honest publisher
    reaches all honest nodes) -- two defect classes:

        in-isolated   node: all RF pull picks adversarial     ~ mu^RF
                      (initiation links do NOT help reception: they only
                      carry their owner's own messages)
        out-isolated  publisher: no honest requester AND all s-1 initiation
                      targets adversarial   ~ mu^(s-1) * e^{-RF(1-mu)}
    """

    s: int = 1   # initial holders: publisher + s-1 standing targets

    def __post_init__(self) -> None:
        super().__post_init__()
        if not 1 <= self.s <= self.N:
            raise ValueError("s must be in [1, N]")

    def p_in_isolated(self) -> float:
        """Exact P(a fixed honest node cannot receive): all RF pull picks
        adversarial = C(k,RF)/C(N-1,RF)."""
        return self.q_pull()

    def p_out_isolated(self) -> float:
        """Exact P(a fixed honest publisher is muted): no honest node picked
        it as forwarder AND all s-1 initiation targets are adversarial."""
        q_noreq = (1 - self.RF / (self.N - 1)) ** (self.H - 1)
        return q_noreq * self.q_pull(self.s - 1)

    def E_defects(self) -> float:
        """Expected number of isolated-vertex defects (in- plus out-)."""
        return self.H * (self.p_in_isolated() + self.p_out_isolated())

    def p_bad(self) -> float:
        """P(some publisher's messages cannot cover), isolated-vertex
        (Poisson) estimate: P(bad) ~ 1 - exp(-E_defects)."""
        return 1.0 - math.exp(-self.E_defects())


# ---------------------------------------------------------------------------
# M3 graph: M2's sampled graph + initiation targets + reachability
# ---------------------------------------------------------------------------

class M3Graph(M2Graph):
    """M2's sampled pull graph, extended with standing initiation targets and
    the multi-hop propagation observables M3's coverage analysis needs.

    Accepts M3Params (samples s-1 initiation targets per honest node) or a
    plain M2Params (no initiation links; ad-hoc seeds may still be passed to
    depths())."""

    def __init__(self, params, rng: random.Random, **kwargs) -> None:
        super().__init__(params, rng, **kwargs)
        self._adj: Optional[List[List[int]]] = None
        self._depth: Optional[List[int]] = None
        # standing initiation targets (honest ones only -- adversarial
        # targets are dead links), s-1 per honest node
        s = getattr(params, "s", 1)
        H, N = params.H, params.N
        self.init_targets: List[List[int]] = [[] for _ in range(H)]
        if s > 1:
            pool = range(N - 1)
            for j in range(H):
                ts = []
                for r in rng.sample(pool, s - 1):
                    t = r if r < j else r + 1
                    if t < H:
                        ts.append(t)
                self.init_targets[j] = ts

    def adjacency(self) -> List[List[int]]:
        """Directed propagation edges: honest forwarder f -> requester j."""
        if self._adj is None:
            adj: List[List[int]] = [[] for _ in range(self.params.N)]
            for j in self.regular_nodes():
                for f in self.picks_of(j):
                    if f < self.adv_start:          # honest forwarder only
                        adj[f].append(j)
            self._adj = adj
        return self._adj

    def seeds(self) -> List[int]:
        """Default seed: the first regular honest node (single-source mode)."""
        return [0]

    def depths(self, seeds: Optional[Sequence[int]] = None) -> List[int]:
        """BFS depth from the seed set; -1 = unreached.  Default seeds cached."""
        default = seeds is None
        if default and self._depth is not None:
            return self._depth
        adj = self.adjacency()
        depth = [-1] * self.params.N
        dq: deque = deque()
        for s in (self.seeds() if default else seeds):
            depth[s] = 0
            dq.append(s)
        while dq:
            v = dq.popleft()
            dv = depth[v] + 1
            for w in adj[v]:
                if depth[w] < 0:
                    depth[w] = dv
                    dq.append(w)
        if default:
            self._depth = depth
        return depth

    def coverage(self, seeds: Optional[Sequence[int]] = None) -> float:
        """Fraction of regular honest nodes reachable from the seeds."""
        depth = self.depths(seeds)
        return sum(1 for j in self.regular_nodes()
                   if depth[j] >= 0) / self.params.H

    def delivery_depth(self, seeds: Optional[Sequence[int]] = None) -> int:
        """Max BFS depth over reached regular honest nodes (tree depth)."""
        depth = self.depths(seeds)
        return max((depth[j] for j in self.regular_nodes() if depth[j] >= 0),
                   default=0)

    def regular_sink_count(self) -> int:
        """Regular honest nodes with out-degree 0 in the propagation graph
        (Poisson(RF) out-degree => ~ e^-RF * H sinks)."""
        adj = self.adjacency()
        return sum(1 for j in self.regular_nodes() if not adj[j])

    # -- strict full-coverage criterion ----------------------------------------

    def _bfs_set(self, adj, sources) -> bytearray:
        seen = bytearray(self.params.H)
        dq = deque()
        for v in sources:
            if not seen[v]:
                seen[v] = 1
                dq.append(v)
        while dq:
            for w in adj[dq.popleft()]:
                if not seen[w]:
                    seen[w] = 1
                    dq.append(w)
        return seen

    def strict_bad(self) -> bool:
        """True iff some honest publisher's messages cannot reach every honest
        node.  A message from p spreads from {p} + p's honest initiation
        targets over the pull relay edges only (initiation links never relay).

        Exact algorithm: locate a member of the giant SCC of the relay graph;
        RG = its forward set, AG = its reverse set.  Every publisher whose
        seed set touches AG reaches (at least) RG, so only two defect surfaces
        remain, both tiny: (a) honest nodes outside RG -- for each, every
        publisher's seed set must intersect its ancestor set; (b) publishers
        whose seed set misses AG -- checked by explicit BFS."""
        H = self.params.H
        adj = self.adjacency()
        radj: List[List[int]] = [[] for _ in range(H)]
        for v in range(H):
            for w in adj[v]:
                radj[w].append(v)

        # giant-SCC member (a few candidates, then exact brute force)
        core = None
        for v in range(min(5, H)):
            fwd = self._bfs_set(adj, [v])
            rev = self._bfs_set(radj, [v])
            if sum(a & b for a, b in zip(fwd, rev)) >= H // 2:
                core, RG, AG = v, fwd, rev
                break
        if core is None:                       # no giant SCC found: brute force
            for p in range(H):
                if sum(self._bfs_set(adj, [p] + self.init_targets[p])) < H:
                    return True
            return False

        # (a) nodes the core cannot reach: every publisher must have a seed
        #     among their ancestors
        for j in range(H):
            if not RG[j]:
                anc = self._bfs_set(radj, [j])
                for p in range(H):
                    if not anc[p] and not any(anc[t] for t in self.init_targets[p]):
                        return True

        # (b) publishers whose seed set cannot reach the core: explicit BFS
        for p in range(H):
            if not AG[p] and not any(AG[t] for t in self.init_targets[p]):
                if sum(self._bfs_set(adj, [p] + self.init_targets[p])) < H:
                    return True
        return False


def sample_strict_bad(params: M3Params, trials: int, rng: random.Random) -> int:
    """# of `trials` sampled graphs that are bad under the strict criterion."""
    return sum(1 for _ in range(trials) if M3Graph(params, rng).strict_bad())


# ---------------------------------------------------------------------------
# Coverage mean-field machinery
# ---------------------------------------------------------------------------

def u_iterate(mu: float, RF: int, iters: int = 100000,
              tol: float = 1e-15) -> float:
    """Smallest fixed point of u = (mu+(1-mu)*u)^RF in [0,1], from u=0.

    u = P(a fixed regular honest node is never reached), given ignition."""
    u = 0.0
    for _ in range(iters):
        nu = (mu + (1 - mu) * u) ** RF
        if abs(nu - u) < tol:
            return nu
        u = nu
    return u


def u_rf2_closed(mu: float) -> float:
    """Explicit RF=2 root of the coverage fixed point: (mu/(1-mu))^2 for
    mu < 1/2, else 1."""
    if mu >= 0.5:
        return 1.0
    return (mu / (1 - mu)) ** 2


def rho_giant(mean_offspring: float) -> float:
    """Survival probability of a Poisson(mean_offspring) branching process:
    the largest root of rho = 1 - exp(-mean_offspring * rho).  0 at/below
    criticality (mean_offspring <= 1)."""
    if mean_offspring <= 1 + 1e-12:
        return 0.0
    r = 0.5
    for _ in range(500):
        r = 1 - math.exp(-mean_offspring * r)
    return r


def _sample_m3(params: M2Params, trials: int, rng: random.Random):
    """Yield `trials` independent M3Graph samples (M2 sampler + reachability)."""
    for _ in range(trials):
        yield M3Graph(params, rng)


# ---------------------------------------------------------------------------
# Self-test (M3: reachability sampler vs coverage mean-field / branching)
# ---------------------------------------------------------------------------

def _selftest() -> int:
    failures = 0

    def check(name, ok, detail=""):
        nonlocal failures
        print(f"  [{'ok  ' if ok else 'FAIL'}] {name}"
              + (f"  ({detail})" if detail else ""))
        if not ok:
            failures += 1

    print("m3_model self-test")
    print("=" * 70)
    rng = random.Random(12345)

    # (1) explicit RF=2 fixed-point root == iteration; u(0) = 0
    print("(1) fixed-point roots vs iteration")
    mus = (0.0, 0.1, 0.2, 0.3, 0.45)
    check("RF=2 closed == iterate",
          all(math.isclose(u_rf2_closed(mu), u_iterate(mu, 2),
                           rel_tol=1e-9, abs_tol=1e-12) for mu in mus))
    check("u(mu=0) == 0",
          all(u_iterate(0.0, RF) == 0.0 for RF in (2, 3, 5)))

    # (2) single-source coverage ~ branching survival rho(RF)
    print("(2) single-source coverage vs rho(RF)")
    for RF in (2, 3):
        rho = rho_giant(RF)
        p = M2Params(N=2000, k=0, RF=RF)
        cov = [g.coverage() for g in _sample_m3(p, 100, rng)]
        m, _ = mean_var(cov)
        check(f"coverage ~ rho({RF}) = {rho:.4f} (+/- 0.03)",
              abs(m - rho) <= 0.03, f"MC {m:.4f}")

    # (3) seeded unreached count ~ H*u (eclipse-floor fixed point)
    print("(3) seeded unreached count vs H*u")
    N, RF, mu, s = 4000, 5, 0.2, 5
    p = M2Params(N=N, k=int(mu * N), RF=RF)
    seeds = list(range(s))
    unc = []
    for g in _sample_m3(p, 100, rng):
        depth = g.depths(seeds)
        unc.append(sum(1 for j in g.regular_nodes() if depth[j] < 0))
    m, v = mean_var(unc)
    pred = p.H * u_iterate(mu, RF)
    se = math.sqrt(v / len(unc))
    check(f"E[unreached] ~ H*u = {pred:.2f} (|z| <= 4)",
          abs(m - pred) <= 4 * max(se, 0.1), f"MC {m:.2f}, se {se:.2f}")

    # (4) strict criterion: closed-form P(bad) vs MC in a measurable regime
    print("(4) strict-criterion P(bad) vs closed form")
    for (N, k, RF, s, T) in [(2000, 400, 6, 4, 1200), (2000, 400, 5, 5, 800)]:
        p = M3Params(N=N, k=k, RF=RF, s=s)
        bad = sample_strict_bad(p, T, rng)
        mc = bad / T
        pred = p.p_bad()
        se = math.sqrt(max(mc, 1 / T) * (1 - mc) / T)
        z = (mc - pred) / se if se > 0 else 0.0
        check(f"N={N} k={k} RF={RF} s={s}: pred {pred:.4f} vs MC {mc:.4f} "
              f"(|z|<=4)", abs(z) <= 4, f"z={z:+.2f}")

    # (5) strict criterion at s=1 == strong connectivity of the pull graph
    print("(5) s=1 reduces to pull strong connectivity")
    p1 = M3Params(N=2000, k=400, RF=10, s=1)
    p2 = M2Params(N=2000, k=400, RF=10)
    ok = True
    for _ in range(60):
        seed = rng.randrange(1 << 30)
        g1 = M3Graph(p1, random.Random(seed))
        g2 = M3Graph(p2, random.Random(seed))
        adj = g2.adjacency()
        radj = [[] for _ in range(p2.H)]
        for v in range(p2.H):
            for w in adj[v]:
                radj[w].append(v)
        sc = (sum(g2._bfs_set(adj, [0])) == p2.H
              and sum(g2._bfs_set(radj, [0])) == p2.H)
        if g1.strict_bad() == sc:              # bad <=> NOT strongly connected
            ok = False
            break
    check("strict_bad(s=1) == not-strongly-connected", ok)

    print("=" * 70)
    print("self-test:", "PASS" if failures == 0 else f"{failures} FAILURE(S)")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(_selftest())
