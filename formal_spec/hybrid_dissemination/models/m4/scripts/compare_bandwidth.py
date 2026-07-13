#!/usr/bin/env python3
"""Bandwidth comparison: transmissions to fully disseminate ONE message,
M3 (directed pull + initiation links) vs M4 (undirected flood), at their
P(bad)=1e-4 operating points for N=20000, mu=0.2.

Rule (both models): a node fires once, on first receipt, sending the message
over every outgoing link EXCEPT the link it arrived on (no resend-back).  We
count honest->honest transmissions (= copies delivered to honest nodes,
including duplicates) -- the bandwidth that does the dissemination work, free
of any assumption about whether adversaries issue requests.

  M3 edges are DIRECTED: forwarder f -> requester j.  f's arrival link is one
  of ITS forwarders, disjoint from its requesters, so resend-back almost never
  fires; every honest->honest pull edge is one transmission:  ~ H*RF*(1-mu).
  M4 edges are UNDIRECTED: a node fires on all incident honest links but the
  arrival one, so each honest edge is used ~twice:  ~ 2*H*RF*(1-mu) - (H-1).

Because M4's links are bidirectional, degree ~ 2*RF -- so the SMALLER RF does
NOT mean less bandwidth.
"""

import os
import random
import sys
from collections import deque

from m4_model import M4Params, M4Graph

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                "..", "..", "m3", "scripts"))
from m3_model import M3Graph  # noqa: E402
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                "..", "..", "m2", "scripts"))
from m2_model import M2Params  # noqa: E402


def flood_sends(adj, seeds):
    """Total sends when each node fires once on first receipt and sends to all
    out-neighbours except the one it received from.  adj[v] = out-neighbours."""
    n = len(adj)
    arrival = [-1] * n
    received = bytearray(n)
    dq = deque()
    for s in seeds:
        if not received[s]:
            received[s] = 1
            arrival[s] = -2          # a seed has no arrival link
            dq.append(s)
    sends = 0
    while dq:
        v = dq.popleft()
        av = arrival[v]
        for w in adj[v]:
            if w == av:
                continue             # don't resend back on the arrival link
            sends += 1
            if not received[w]:
                received[w] = 1
                arrival[w] = v
                dq.append(w)
    return sends


def m4_sends(params, rng):
    g = M4Graph(params, rng)
    return flood_sends(g.adj, [0])            # honest source 0; adj is honest-honest


def m3_sends(params, s, rng):
    g = M3Graph(params, rng)
    adj = g.adjacency()                       # honest forwarder -> honest requester
    seeds = [0]                               # publisher (first honest node)
    for r in rng.sample(range(params.N - 1), s - 1):
        if not g.is_adversarial(r + 1):
            seeds.append(r + 1)
    return flood_sends(adj, seeds)


def main():
    N, mu, trials = 20000, 0.2, 60
    k = int(round(mu * N)); H = N - k
    rng = random.Random(2024)

    print(f"Transmissions per disseminated message  --  N={N}, mu={mu} "
          f"(H={H}), {trials} graphs each")
    print()

    # M3 operating point: RF=11, s=3
    RF3, s = 11, 3
    p3 = M2Params(N=N, k=k, RF=RF3)
    m3 = [m3_sends(p3, s, rng) for _ in range(trials)]
    a3 = sum(m3) / trials
    pred3 = H * RF3 * (1 - mu)

    # M4 operating point: RF=8
    RF4 = 8
    p4 = M4Params(N=N, k=k, RF=RF4)
    m4 = [m4_sends(p4, rng) for _ in range(trials)]
    a4 = sum(m4) / trials
    pred4 = 2 * H * RF4 * (1 - mu) - (H - 1)

    print(f"  {'model':<28} {'total sends':>12} {'per honest node':>16} "
          f"{'(predicted)':>12}")
    print(f"  {'M3  pull RF=11, s=3':<28} {a3:>12,.0f} {a3/H:>16.2f} "
          f"{pred3:>12,.0f}")
    print(f"  {'M4  flood RF=8':<28} {a4:>12,.0f} {a4/H:>16.2f} "
          f"{pred4:>12,.0f}")
    print()
    print(f"  M4 / M3 transmission ratio: {a4/a3:.2f}x")
    print(f"  (M4 uses ~{a4/a3:.0%} of M3's per-message bandwidth despite the "
          f"smaller RF —")
    print(f"   bidirectional degree ~2*RF is what drives the cost, not RF.)")


if __name__ == "__main__":
    main()
