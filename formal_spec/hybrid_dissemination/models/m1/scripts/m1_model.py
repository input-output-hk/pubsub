#!/usr/bin/env python3
"""The M1 dissemination model: pure random push gossip.

M1 (see ../README.md): each node pushes every message to F targets
chosen uniformly without replacement; no ring, no pull, no seeding tier.
Adversaries are silent (they receive but never relay), so a message from an
honest source reaches exactly the honest nodes reachable through honest
push edges.  Reception is governed entirely by a node's IN-edges, which are
other nodes' choices -- the structural weakness the pull models invert.

Node layout: [0, H) regular honest, [H, N) adversarial (silent).  N = H + k,
mu = k/N.  The only per-node parameter is F (push fanout).  Adversaries'
own picks are never sampled -- they create no honest->honest edge.

A sampled graph is GOOD iff every message of every honest publisher reaches
all honest nodes == the honest push digraph is strongly connected.  Two
defect classes dominate badness:

    in-isolated   (cannot receive): no honest node picked it
                                                 ~ e^{-F(1-mu)}, seed-proof
    out-isolated  (muted publisher): all F own picks adversarial   ~ mu^F

The in-term dominates for all mu < 0.47 (ln(1/mu) > 1-mu there).

Provides:
    M1Params    N, k, F (+ derived H, mu) and the closed forms
                p_in_isolated / p_out_isolated / E_defects / p_bad
    M1Graph     ONE sampled push digraph: strong connectivity (`is_bad`),
                honest-only BFS, per-class defect counts
    sample_bad  Monte-Carlo P(bad graph) helper

Run `python3 m1_model.py` for a self-test.  No dependencies beyond stdlib.
"""

from __future__ import annotations

import math
import random
from collections import deque
from dataclasses import dataclass
from typing import List


@dataclass(frozen=True)
class M1Params:
    """M1 parameter set.  N = H + k, H >= 1 honest, single knob F."""

    N: int
    k: int   # adversarial (silent) nodes
    F: int   # push fanout (targets per node per message)

    def __post_init__(self) -> None:
        if self.N < 2:
            raise ValueError("N must be >= 2")
        if not 0 <= self.k < self.N:
            raise ValueError("need 0 <= k < N")
        if not 1 <= self.F <= self.N - 1:
            raise ValueError("F must be in [1, N-1]")

    @property
    def H(self) -> int:
        return self.N - self.k

    @property
    def mu(self) -> float:
        return self.k / self.N

    # -- closed forms --------------------------------------------------------

    def p_in_isolated(self) -> float:
        """Exact P(a fixed honest node has no honest in-edge): none of the
        other H-1 honest nodes includes it among their F uniform picks.

            q_in = (1 - F/(N-1))^(H-1)  ~  e^{-F(1-mu)}

        Such a node is unreachable from EVERY publisher -- seed-proof."""
        return (1 - self.F / (self.N - 1)) ** (self.H - 1)

    def p_out_isolated(self) -> float:
        """Exact P(a fixed honest node has no honest out-edge): all F of its
        own picks are adversarial (hypergeometric) -- a muted publisher.

            q_out = C(k,F)/C(N-1,F)  ~  mu^F"""
        if self.k < self.F:
            return 0.0
        v = 1.0
        for i in range(self.F):
            v *= (self.k - i) / (self.N - 1 - i)
        return v

    def E_defects(self) -> float:
        """Expected number of isolated-vertex defects (in- plus out-)."""
        return self.H * (self.p_in_isolated() + self.p_out_isolated())

    def p_bad(self) -> float:
        """P(honest digraph not strongly connected), isolated-vertex (Poisson)
        estimate: P(bad) ~ 1 - exp(-E_defects)."""
        return 1.0 - math.exp(-self.E_defects())


class M1Graph:
    """One sampled push digraph.  Only honest->honest edges are stored
    (adversaries neither relay nor contribute useful in-edges)."""

    def __init__(self, params: M1Params, rng: random.Random) -> None:
        self.params = params
        N, H, F = params.N, params.H, params.F
        adj: List[List[int]] = [[] for _ in range(H)]   # i -> honest targets
        indeg = bytearray(H)                            # has an honest in-edge?
        pool = range(N - 1)
        for i in range(H):                              # honest pushers only
            for r in rng.sample(pool, F):
                j = r if r < i else r + 1               # skip self
                if j < H:                               # dead edge if adversarial
                    adj[i].append(j)
                    indeg[j] = 1
        self.adj = adj
        self._indeg = indeg

    def honest_reached(self, source: int = 0) -> int:
        """# honest nodes reached by a push cascade from `source`."""
        seen = bytearray(self.params.H)
        seen[source] = 1
        dq = deque([source])
        n = 1
        adj = self.adj
        while dq:
            for w in adj[dq.popleft()]:
                if not seen[w]:
                    seen[w] = 1
                    n += 1
                    dq.append(w)
        return n

    def _covers_all(self, adj) -> bool:
        H = self.params.H
        seen = bytearray(H)
        seen[0] = 1
        n = 1
        dq = deque([0])
        while dq:
            for w in adj[dq.popleft()]:
                if not seen[w]:
                    seen[w] = 1
                    n += 1
                    dq.append(w)
        return n == H

    def is_bad(self) -> bool:
        """True iff the honest push digraph is NOT strongly connected (some
        honest publisher cannot reach some honest node): forward and reverse
        BFS from honest node 0 must both cover all honest nodes."""
        if not self._covers_all(self.adj):
            return True
        radj: List[List[int]] = [[] for _ in range(self.params.H)]
        for v, ws in enumerate(self.adj):
            for w in ws:
                radj[w].append(v)
        return not self._covers_all(radj)

    def in_isolated_count(self) -> int:
        """# honest nodes with no honest in-edge (node 0 excluded to keep the
        count comparable with (H-1)*p_in_isolated in tests)."""
        return sum(1 for v in range(1, self.params.H) if not self._indeg[v])

    def out_isolated_count(self) -> int:
        """# honest nodes with no honest out-edge."""
        return sum(1 for v in range(self.params.H) if not self.adj[v])


def sample_bad(params: M1Params, trials: int, rng: random.Random) -> int:
    """# of `trials` sampled graphs that are bad (not strongly connected)."""
    return sum(1 for _ in range(trials) if M1Graph(params, rng).is_bad())


# ---------------------------------------------------------------------------
# Self-test
# ---------------------------------------------------------------------------

def _selftest() -> int:
    failures = 0

    def check(name, ok, detail=""):
        nonlocal failures
        print(f"  [{'ok  ' if ok else 'FAIL'}] {name}"
              + (f"  ({detail})" if detail else ""))
        if not ok:
            failures += 1

    print("m1_model self-test")
    print("=" * 70)
    rng = random.Random(12345)

    # (1) mu=0, F=1: ~H/e in-degree-0 nodes => always bad; large F: good
    print("(1) structural in-degree-0 obstruction")
    bad1 = sample_bad(M1Params(N=2000, k=0, F=1), 100, rng)
    badL = sample_bad(M1Params(N=2000, k=0, F=12), 200, rng)
    check("F=1 always disconnected", bad1 == 100, f"{bad1}/100 bad")
    check("F=12 almost always covers", badL <= 10, f"{badL}/200 bad")

    # (2) closed-form P(bad) vs MC in a measurable regime
    print("(2) closed-form P(bad) vs Monte-Carlo")
    for (N, k, F) in [(2000, 400, 10), (2000, 400, 12), (4000, 800, 12)]:
        p = M1Params(N=N, k=k, F=F)
        T = 3000
        bad = sample_bad(p, T, rng)
        mc = bad / T
        pred = p.p_bad()
        se = math.sqrt(max(mc, 1 / T) * (1 - mc) / T)
        z = (mc - pred) / se if se > 0 else 0.0
        check(f"N={N} k={k} F={F}: pred {pred:.4f} vs MC {mc:.4f} (|z|<=4)",
              abs(z) <= 4, f"z={z:+.2f}")

    # (3) per-class defect counts match closed forms
    print("(3) E[in-/out-isolated] vs closed forms")
    p = M1Params(N=2000, k=400, F=8)
    T = 3000
    tin = sum(M1Graph(p, rng).in_isolated_count() for _ in range(T))
    pred_in = (p.H - 1) * p.p_in_isolated()
    check(f"E_in pred {pred_in:.4f} vs MC {tin / T:.4f}",
          abs(tin / T - pred_in) <= 0.12, f"MC {tin/T:.4f}")
    p = M1Params(N=2000, k=1000, F=3)     # mu=0.5: out-defects measurable
    T = 400
    tout = sum(M1Graph(p, rng).out_isolated_count() for _ in range(T))
    pred_out = p.H * p.p_out_isolated()
    check(f"E_out pred {pred_out:.2f} vs MC {tout / T:.2f} (mu=0.5, F=3)",
          abs(tout / T - pred_out) <= max(1.0, 0.05 * pred_out),
          f"MC {tout/T:.2f}")

    print("=" * 70)
    print("self-test:", "PASS" if failures == 0 else f"{failures} FAILURE(S)")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(_selftest())
