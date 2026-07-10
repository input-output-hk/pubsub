#!/usr/bin/env python3
"""The M5 dissemination model: directed k_in/k_out gossip.

M5 (see ../README.md): every node opens k_in INBOUND links (it chooses k_in
forwarders that will relay everything to it) and k_out OUTBOUND links (it
chooses k_out targets it will relay everything to); each pick is a directed
edge.  A node that receives a message relays it once on every outgoing
propagation edge -- its own k_out targets plus the nodes that picked it as
forwarder -- except back on the arrival link.  Adversaries are silent
(receive, never relay), so propagation runs on honest->honest edges only.

Node layout: [0, H) regular honest, [H, N) adversarial.  N = H + k,
mu = k/N.  Knobs: k_in, k_out.  The sampled honest digraph is the classical
k-in/k-out random digraph (Fenner-Frieze) restricted to honest picks.

A sampled graph is GOOD iff every honest node can, as publisher, reach all
honest nodes == the honest propagation digraph is strongly connected.  Two
defect classes dominate badness, each doubly protected (own picks x others'
picks):

    in-isolated   (cannot receive): own k_in picks all adversarial AND no
                  honest node out-picked it     ~ mu^k_in * e^{-k_out(1-mu)}
    out-isolated  (muted publisher): own k_out picks all adversarial AND no
                  honest node in-picked it      ~ mu^k_out * e^{-k_in(1-mu)}

Provides:
    M5Params    N, k, k_in, k_out (+ derived H, mu) and the closed forms
                p_in_isolated / p_out_isolated / E_defects / p_bad
    M5Graph     ONE sampled digraph: propagation adjacency, strong
                connectivity (`is_bad`), per-class defect counts
    sample_bad  Monte-Carlo P(bad graph) helper

Reversing every edge of M5(k_in, k_out) is distributed as M5(k_out, k_in) and
strong connectivity is reversal-invariant, so P_bad is exactly symmetric in
(k_in, k_out).  Run `python3 m5_model.py` for a self-test.  Stdlib only.
"""

from __future__ import annotations

import math
import random
from collections import deque
from dataclasses import dataclass
from typing import List


@dataclass(frozen=True)
class M5Params:
    """M5 parameter set.  N = H + k, H >= 1 honest; knobs k_in, k_out."""

    N: int
    k: int      # adversarial (silent) nodes
    k_in: int   # inbound links each node opens (its chosen forwarders)
    k_out: int  # outbound links each node opens (its chosen targets)

    def __post_init__(self) -> None:
        if self.N < 2:
            raise ValueError("N must be >= 2")
        if not 0 <= self.k < self.N:
            raise ValueError("need 0 <= k < N")
        if not (0 <= self.k_in <= self.N - 1 and 0 <= self.k_out <= self.N - 1):
            raise ValueError("k_in and k_out must be in [0, N-1]")
        if self.k_in + self.k_out < 1:
            raise ValueError("need k_in + k_out >= 1")

    @property
    def H(self) -> int:
        return self.N - self.k

    @property
    def mu(self) -> float:
        return self.k / self.N

    # -- closed forms --------------------------------------------------------

    def _q_own_dead(self, picks: int) -> float:
        """Exact P(all `picks` own picks adversarial) = C(k,picks)/C(N-1,picks)."""
        if self.k < picks:
            return 0.0
        v = 1.0
        for i in range(picks):
            v *= (self.k - i) / (self.N - 1 - i)
        return v

    def _q_no_honest_pick(self, picks: int) -> float:
        """Exact P(none of the other H-1 honest nodes includes a fixed node
        among their `picks` uniform picks) = (1 - picks/(N-1))^(H-1)."""
        return (1 - picks / (self.N - 1)) ** (self.H - 1)

    def p_in_isolated(self) -> float:
        """P(a fixed honest node has no honest in-edge): own k_in picks all
        adversarial AND no honest node out-picked it (independent)."""
        return self._q_own_dead(self.k_in) * self._q_no_honest_pick(self.k_out)

    def p_out_isolated(self) -> float:
        """P(a fixed honest node has no honest out-edge): own k_out picks all
        adversarial AND no honest node in-picked it (independent)."""
        return self._q_own_dead(self.k_out) * self._q_no_honest_pick(self.k_in)

    def E_defects(self) -> float:
        """Expected number of isolated-vertex defects (in- plus out-)."""
        return self.H * (self.p_in_isolated() + self.p_out_isolated())

    def p_bad(self) -> float:
        """P(honest digraph not strongly connected), isolated-vertex (Poisson)
        estimate: P(bad) ~ 1 - exp(-E_defects)."""
        return 1.0 - math.exp(-self.E_defects())


class M5Graph:
    """One sampled k_in/k_out digraph.  Only honest nodes' picks are sampled
    (adversary picks are irrelevant -- silent); only honest->honest edges are
    kept in the propagation adjacency."""

    def __init__(self, params: M5Params, rng: random.Random) -> None:
        self.params = params
        N, H = params.N, params.H
        pool = range(N - 1)
        adj: List[List[int]] = [[] for _ in range(H)]   # honest->honest edges
        has_in = bytearray(H)
        has_out = bytearray(H)
        for j in range(H):
            for r in rng.sample(pool, params.k_in):     # forwarder f -> j
                f = r if r < j else r + 1
                if f < H:
                    adj[f].append(j)
                    has_in[j] = 1
                    has_out[f] = 1
            for r in rng.sample(pool, params.k_out):    # j -> target t
                t = r if r < j else r + 1
                if t < H:
                    adj[j].append(t)
                    has_out[j] = 1
                    has_in[t] = 1
        self.adj = adj
        self._has_in = has_in
        self._has_out = has_out

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
        """True iff the honest digraph is NOT strongly connected (some honest
        publisher cannot reach some honest node): forward and reverse BFS
        from honest node 0 must both cover all honest nodes."""
        if not self._covers_all(self.adj):
            return True
        radj: List[List[int]] = [[] for _ in range(self.params.H)]
        for v, ws in enumerate(self.adj):
            for w in ws:
                radj[w].append(v)
        return not self._covers_all(radj)

    def in_isolated_count(self) -> int:
        """# honest nodes with no honest in-edge."""
        return sum(1 for v in range(self.params.H) if not self._has_in[v])

    def out_isolated_count(self) -> int:
        """# honest nodes with no honest out-edge."""
        return sum(1 for v in range(self.params.H) if not self._has_out[v])


def sample_bad(params: M5Params, trials: int, rng: random.Random) -> int:
    """# of `trials` sampled graphs that are bad (not strongly connected)."""
    return sum(1 for _ in range(trials) if M5Graph(params, rng).is_bad())


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

    print("m5_model self-test")
    print("=" * 70)
    rng = random.Random(12345)

    # (1) mu=0: k_in=k_out=1 is mostly not strongly connected; 2/2 is (F-F)
    print("(1) no adversaries: 1-in/1-out bad, 2-in/2-out good (Fenner-Frieze)")
    bad1 = sample_bad(M5Params(N=2000, k=0, k_in=1, k_out=1), 100, rng)
    bad2 = sample_bad(M5Params(N=2000, k=0, k_in=2, k_out=2), 100, rng)
    check("1-in/1-out fails at a non-vanishing rate", bad1 >= 5,
          f"{bad1}/100 bad")
    check("2-in/2-out almost always SC", bad2 <= 10, f"{bad2}/100 bad")

    # (2) closed-form P(bad) vs MC in a measurable regime
    print("(2) closed-form P(bad) vs Monte-Carlo")
    for (N, k, a, b, T) in [(2000, 400, 3, 3, 1000), (2000, 400, 4, 4, 3000),
                            (2000, 400, 2, 6, 1500), (2000, 400, 6, 2, 1500)]:
        p = M5Params(N=N, k=k, k_in=a, k_out=b)
        bad = sample_bad(p, T, rng)
        mc = bad / T
        pred = p.p_bad()
        se = math.sqrt(max(mc, 1 / T) * (1 - mc) / T)
        z = (mc - pred) / se if se > 0 else 0.0
        check(f"N={N} k={k} ({a},{b}): pred {pred:.4f} vs MC {mc:.4f} (|z|<=4)",
              abs(z) <= 4, f"z={z:+.2f}")

    # (3) per-class defect counts match E_in, E_out (exact means)
    print("(3) E[in-/out-isolated] vs closed forms")
    p = M5Params(N=2000, k=400, k_in=3, k_out=4)
    T = 2000
    tin = tout = 0
    for _ in range(T):
        g = M5Graph(p, rng)
        tin += g.in_isolated_count()
        tout += g.out_isolated_count()
    e_in, e_out = p.H * p.p_in_isolated(), p.H * p.p_out_isolated()
    check(f"E_in pred {e_in:.4f} vs MC {tin / T:.4f}",
          abs(tin / T - e_in) <= max(0.1, 0.15 * e_in), f"MC {tin/T:.4f}")
    check(f"E_out pred {e_out:.4f} vs MC {tout / T:.4f}",
          abs(tout / T - e_out) <= max(0.1, 0.15 * e_out), f"MC {tout/T:.4f}")

    # (4) exact (k_in,k_out) <-> (k_out,k_in) symmetry of the closed form
    print("(4) closed-form symmetry under (k_in,k_out) swap")
    ok = all(math.isclose(M5Params(N=2000, k=400, k_in=a, k_out=b).p_bad(),
                          M5Params(N=2000, k=400, k_in=b, k_out=a).p_bad(),
                          rel_tol=1e-12)
             for a, b in ((2, 6), (3, 5), (1, 9)))
    check("p_bad(a,b) == p_bad(b,a)", ok)

    print("=" * 70)
    print("self-test:", "PASS" if failures == 0 else f"{failures} FAILURE(S)")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(_selftest())
