# E2/E5 — propagation depth as a distribution

Latency at the five operating points, reported as the full first-receipt depth
distribution rather than as a pair of means. Like the model-family comparisons
it **informs, it does not gate**.

## Why

`comparison.md` reports two latency numbers per design: hops to full coverage
and mean first receipt. Across the family they span 4.8 to 5.9 and 3.6 to 4.3,
which is why the comparison concludes latency does not discriminate. That
conclusion is about the means. Whether it holds for the subscribers who wait
longest is a different question, and the instrument has always recorded the
depth histogram needed to answer it.

No new runs were needed: the pooled histogram was already in the output of the
operating-point cells.

## Provenance

| | |
|---|---|
| Configurations | `comparisons/m{1,3,4,5}-n20k-op.toml`, `m2-operating-point.toml` |
| Runs | 200 per cell (M2: 40) |
| Source | `depth_hist_pooled` in each sweep's `aggregates.json` |

## 1. The distributions

Receipts by wave, pooled over every publication in the cell. Wave 0 is the
publisher's own record.

| Design | 0 | 1 | 2 | 3 | 4 | 5 | 6 | mean | published |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| M3 | 200 | 3,046 | 28,847 | 263,226 | 1,598,654 | 1,300,634 | 5,393 | 4.31 | 4.3 |
| M4 | 200 | 2,548 | 31,206 | 360,103 | 2,102,947 | 702,953 | 43 | 4.09 | 4.1 |
| M5 | 200 | 2,631 | 35,514 | 442,768 | 2,296,463 | 422,423 | 1 | 3.97 | 3.9 |
| M1 | 200 | 3,827 | 72,549 | 1,099,297 | 2,020,780 | 3,347 | — | 3.61 | 3.6 |
| M2 | 40 | 789 | 14,773 | 222,697 | 401,253 | 448 | — | 3.60 | 3.6 |

The means reproduce the published figures exactly.

## 2. The tail does discriminate

The deepest wave carries a tiny share of receipts, but the share differs by
orders of magnitude between designs:

| Design | Deepest wave | Receipts there | Share of all receipts |
|---|---:|---:|---:|
| M3 | 6 | 5,393 | 0.1685 % |
| M4 | 6 | 43 | 0.0013 % |
| M5 | 6 | 1 | 0.0000 % |
| M1 | 5 | 3,347 | 0.1046 % |
| M2 | 5 | 448 | 0.0700 % |

M3 delivers to its slowest subscribers at wave 6 about 125 times as often as
M4 does, and M5 reaches wave 6 once in 3.2 million receipts. Read as means the
field spans 0.7 hops and looks flat; read at the tail it spans two orders of
magnitude.

The absolute numbers keep this in proportion. M3's 5 393 wave-6 receipts over
200 publications is roughly 27 subscribers per publication out of 16 000, so
about 0.17 % of the audience waits one hop longer than under M4. Whether that
matters is a use-case question: for a governance notification it plainly does
not, and for an emergency alert to operators it might.

## 3. What this changes

The conclusion that latency does not separate the designs is correct **at the
mean**, and should be stated that way rather than as a general claim. At the
tail the separation is real but affects a fraction of a percent of subscribers,
and it runs in the same direction as the other axes: M3, cheapest in bandwidth,
is also the slowest to reach its last subscribers.

## 4. Limits

- **Hops, not seconds.** Depth is forwarding steps. Converting needs a per-hop
  latency assumption the framework does not make, and real deployments vary
  by an order of magnitude across regions.
- **One publication per run** at these cells, so the histogram pools across
  runs rather than describing within-run variation.
- **M2's cell is 40 runs**, an eighth of the others, so its tail share is the
  least well resolved.
