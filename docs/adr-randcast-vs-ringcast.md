# ADR: RandCast vs RingCast

**Status:** Proposed

**Context:** The dissemination layer can use either random-only links
(RandCast) or a Harary graph with random links (RingCast). This is the
key design choice that determines protocol complexity.

---

## Comparison

| | RandCast (random links only) | RingCast (Harary + random links) |
|---|---|---|
| **Delivery guarantee** | Probabilistic (high but not 100%) | Deterministic under t-1 crash failures |
| **Join/leave complexity** | Trivial — get random peers, done | Convergence period to find ring position |
| **Positional Sybil risk** | N/A — no ring to game | Requires unpredictable position assignment |
| **New joiner experience** | Immediately fully integrated | Degraded until ring converges |
| **Implementation complexity** | Low | Medium-high (ring maintenance, convergence) |
| **BFT story** | Simpler — fewer attack surfaces | Stronger guarantee but more gaps to close |
| **When to choose** | SPO-scale, low churn, notification use case | High-failure environments, strict delivery SLAs |

---

## Open Questions

- Is probabilistic delivery sufficient for the base use case (SPO
  notifications), or do we need deterministic guarantees?
- Does the threat model (Epic #5) justify the added complexity of ring
  maintenance and convergence?
- Can RandCast delivery properties be formally verified to a
  satisfactory bound?

---

## Decision

Pending — to be resolved by Epic #4 (Formal Dissemination Analysis) and
Epic #5 (Threat Model & Security Analysis).
