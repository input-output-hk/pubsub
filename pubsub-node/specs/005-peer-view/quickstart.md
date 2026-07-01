# Quickstart: exercising the bounded strategies

Illustrative — exact names settle at implementation. Strategies use the current injection shape (and migrate unchanged if the parallel refactor later passes them as `apply` arguments).

## Construct a bounded node

```text
let selection  = SeededBoundedSelection { seed, self_id, upstream_degree };   // dial side
let acceptance = BoundedAcceptance { downstream_degree };                        // inbound side
// strategies injected at node construction (migrate to apply-arguments with the parallel refactor)
```

Omit the bounded strategies (or the seed/degree params at the edge) → today's full-mesh behaviour, unchanged (SC-005).

## Exercise (test shape)

1. Seed candidate membership (fixed before readiness).
2. Drive a node (or small set) to readiness, then drive re-dial by re-invoking `ConnectionSetup` explicitly (decouple flag) — no timers.
3. To exercise rejection: drive a node past its `downstream_degree` with inbound requests; observe the over-capacity `Rejected`. On the dialer, after a `Rejected`, re-invoke `ConnectionSetup` and observe back-fill to the next-ranked candidate.

## Assert via getters/snapshots (never logs)

- **Bound** — `upstream_connections()` ≤ `upstream_degree`/topic; `downstream_connections()` ≤ `downstream_degree`/topic (SC-002).
- **Reproducibility** — same seed + membership → identical upstream set on rebuild (SC-001).
- **Variety** — two seeds → differing selections for candidates > upstream degree (SC-003).
- **Back-fill / under-fill** — a rejected peer is excluded and the next-ranked is dialed on the next `ConnectionSetup`; exhaustion settles at under-fill (FR-014/FR-015).
- **Rejection count** — the rejection getter reflects explicit over-capacity rejections (FR-016/SC-007).
- **Unbiasedness** — over ≥1,000 seeds on a fixed candidate set, per-candidate frequency within tolerance (FR-007/SC-004; research R5).

## Out of scope here

The topology builder, multi-node scale, and delivery-percentile/latency/propagation metrics are the separate experiment-framework feature; golden nodes, edge/golden mode, and adversarial behaviour are later features.
