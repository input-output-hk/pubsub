#!/usr/bin/env python3
"""The M4 dissemination model: undirected (bidirectional) RF-out gossip.

M4 (see ../README.md): every node picks RF peers uniformly; each pick is a
BIDIRECTIONAL link.  A message floods along all incident links except the one
it arrived on.  Adversaries are silent (they receive but never relay), so a
message from a source reaches exactly the honest nodes in the source's
connected component of the honest-induced subgraph.

Node layout: [0, H) regular honest, [H, N) adversarial (silent).  N = H + k,
mu = k/N.  The only per-node parameter is RF.

Provides:
    M4Params    N, k, RF  (+ derived H, mu) and the closed forms
                p_isolated / E_isolated / p_bad
    M4Graph     ONE sampled undirected graph: honest-only BFS coverage,
                connectivity (`is_bad`), isolated-honest count
    sample_bad  Monte-Carlo P(bad graph) helper

`is_bad()` == "the honest-induced subgraph is disconnected" == "some honest
node fails to receive a message flooded from an honest source" (undirected, so
independent of which honest source).  Run `python3 m4_model.py` for a self-test.
"""

from __future__ import annotations

import math
import random
from collections import deque
from dataclasses import dataclass
from typing import List


@dataclass(frozen=True)
class M4Params:
    """M4 parameter set.  N = H + k, H >= 1 honest, single knob RF."""

    N: int
    k: int   # adversarial (silent) nodes
    RF: int  # peers each node picks (bidirectional)

    def __post_init__(self) -> None:
        if self.N < 2:
            raise ValueError("N must be >= 2")
        if not 0 <= self.k < self.N:
            raise ValueError("need 0 <= k < N")
        if not 1 <= self.RF <= self.N - 1:
            raise ValueError("RF must be in [1, N-1]")

    @property
    def H(self) -> int:
        return self.N - self.k

    @property
    def mu(self) -> float:
        return self.k / self.N

    # -- closed forms --------------------------------------------------------

    def p_isolated(self) -> float:
        """Exact P(a fixed honest node is isolated in the honest subgraph):
        all RF of its own picks are adversarial AND no honest node picks it.

            p_iso = C(k,RF)/C(N-1,RF) * (1 - RF/(N-1))^(H-1)

        The two factors are independent (a node's own picks vs. who picks it)."""
        N, k, RF, H = self.N, self.k, self.RF, self.H
        # own picks all adversarial (hypergeometric)
        q_out = 1.0
        if k < RF:
            q_out = 0.0
        else:
            for i in range(RF):
                q_out *= (k - i) / (N - 1 - i)
        # no honest node (of the other H-1) picks this node
        q_in = (1 - RF / (N - 1)) ** (H - 1)
        return q_out * q_in

    def E_isolated(self) -> float:
        """Expected number of isolated honest nodes."""
        return self.H * self.p_isolated()

    def p_bad(self) -> float:
        """P(honest subgraph disconnected), isolated-vertex (Poisson) estimate.

        Near/above the connectivity threshold, disconnection is dominated by
        isolated honest vertices, so P(bad) ~ 1 - exp(-E_isolated)."""
        return 1.0 - math.exp(-self.E_isolated())


class M4Graph:
    """One sampled undirected RF-out graph.  Only honest nodes' adjacency is
    stored (adversary lists are never traversed -- silent)."""

    def __init__(self, params: M4Params, rng: random.Random) -> None:
        self.params = params
        N, H, RF = params.N, params.H, params.RF
        adj: List[List[int]] = [[] for _ in range(H)]   # honest-honest edges only
        pool = range(N - 1)
        for i in range(N):
            for r in rng.sample(pool, RF):
                j = r if r < i else r + 1                # skip self
                if i < H and j < H:                      # dead edge if either adv
                    adj[i].append(j)
                    adj[j].append(i)
        self.adj = adj

    def honest_reached(self, source: int = 0) -> int:
        """# honest nodes reached by a flood from `source` (honest-only BFS)."""
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

    def is_bad(self) -> bool:
        """True iff some honest node is unreachable (honest subgraph split)."""
        return self.honest_reached(0) < self.params.H

    def isolated_honest_count(self) -> int:
        """# honest nodes with no honest neighbour (degree 0 in honest graph)."""
        return sum(1 for v in range(self.params.H) if not self.adj[v])


def sample_bad(params: M4Params, trials: int, rng: random.Random) -> int:
    """# of `trials` sampled graphs that are bad (honest subgraph split)."""
    return sum(1 for _ in range(trials) if M4Graph(params, rng).is_bad())


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

    print("m4_model self-test")
    print("=" * 70)
    rng = random.Random(12345)

    # (1) mu=0: RF=1 disconnects (functional graph), RF>=2 connected w.h.p.
    print("(1) no adversaries: RF=1 bad, RF=2 good")
    bad1 = sample_bad(M4Params(N=2000, k=0, RF=1), 200, rng)
    bad2 = sample_bad(M4Params(N=2000, k=0, RF=2), 200, rng)
    check("RF=1 almost always disconnected", bad1 >= 180, f"{bad1}/200 bad")
    check("RF=2 almost always connected", bad2 <= 10, f"{bad2}/200 bad")

    # (2) closed form vs MC in a measurable regime (P(bad) ~ 0.1-0.7)
    print("(2) closed-form P(bad) vs Monte-Carlo")
    for (N, k, RF) in [(2000, 400, 3), (2000, 400, 4), (4000, 800, 4)]:
        p = M4Params(N=N, k=k, RF=RF)
        T = 3000
        bad = sample_bad(p, T, rng)
        mc = bad / T
        pred = p.p_bad()
        se = math.sqrt(max(mc, 1 / T) * (1 - mc) / T)
        z = (mc - pred) / se if se > 0 else 0.0
        check(f"N={N} k={k} RF={RF}: pred {pred:.4f} vs MC {mc:.4f} (|z|<=4)",
              abs(z) <= 4, f"z={z:+.2f}")

    # (3) isolated-count mean matches E_isolated
    print("(3) E[isolated honest] vs closed form")
    p = M4Params(N=2000, k=400, RF=4)
    T = 3000
    tot = sum(M4Graph(p, rng).isolated_honest_count() for _ in range(T))
    check(f"E_iso pred {p.E_isolated():.4f} vs MC {tot / T:.4f}",
          abs(tot / T - p.E_isolated()) <= 0.05, f"MC {tot/T:.4f}")

    print("=" * 70)
    print("self-test:", "PASS" if failures == 0 else f"{failures} FAILURE(S)")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(_selftest())
