# E-degree — standing links per node, measured

The state axis of the [model comparison](../../../formal_spec/hybrid_dissemination/models/comparison.md),
measured by the experiments framework for the first time. Like the model-family
comparisons it **informs, it does not gate**; the statistical conventions are
[`m2-comparison.md`](m2-comparison.md) §4.

## Why this was missing

The framework's degree histograms are taken over the extracted **propagation
digraph** — the graph dissemination actually flows along. That graph omits, by
construction, any link that carries no traffic: under M3 the initiation links
are seed edges and never relay, so fourteen of that model's thirty-eight held
links never entered a measured distribution. Under M1 the relay mesh is empty
and only publisher links propagate.

That left the comparison lopsided. Bandwidth was replicated between the two
instruments to a few hundredths of a percent, while the axis on which M4 beats
M3 — connections held — rested on the formal side's twenty-five graphs alone.
Since the design choice turns on state as much as on traffic, the weaker axis
was the deciding one.

## What is counted

Per up-honest node, the distinct **(peer, link kind)** pairs it holds an
established link with, in either direction, regardless of the counterparty's
class — an adversary still occupies a connection slot. A symmetric relay link
registers on both the upstream and downstream side for the same peer and kind
and is counted once.

This is the chooser-plus-acceptor total: under protocol-compliant opening it
should come to twice the nominal budget, which is what the published table's
"compliant total" column states.

## Provenance

| | |
|---|---|
| Tool commit | the standing-degree change on `experiments/churn-sweep` |
| Configurations | the five shipped operating points — `comparisons/m{1,3,4,5}-n20k-op.toml` and `m2-operating-point.toml` |
| Runs | 200 per cell (M2: 40, the shipped config's count) |
| Timings | ~6 min total |
| Reference | `models/comparison.md` §3 |

## 1. Means — against the published table

| Design | Parameters | Runs | Measured mean | Published | Propagation degree (mean in) |
|---|---|---:|---:|---:|---:|
| M3 | RF=12, s=8 | 200 | **37.99** | 38 | 9.60 |
| M4 | RF=8 | 200 | **16.00** | 16 | 12.80 |
| M5 | (9, 8) | 200 | **33.99** | 34 | 13.60 |
| M1 | F=24 | 200 | **47.97** | 48 | 19.20 |
| M2 | RF=24 | 40 | **47.97** | 48 | 19.20 |

Every mean lands on the published value. The last column is the propagation
degree the framework already reported, and the gap between the two columns is
the point: for M3 it is 38 against 9.6, because relaying and seeding are
separate link kinds and only one of them carries dissemination.

## 2. Maxima — a number the published table does not carry

Connection slots are provisioned for the worst-affected node, not the average
one, so the maximum matters as much as the mean.

| Design | Mean held | Max held | Chosen (deterministic) | Implied max accepted | Published max accepted |
|---|---:|---:|---:|---:|---:|
| M3 | 37.99 | **64** | 19 | 45 | ~36 accepted |
| M4 | 16.00 | **36** | 8 | 28 | 29 |
| M5 | 33.99 | **58** | 17 | 41 | 33 |
| M1 | 47.97 | **75** | 24 | 51 | 41 |
| M2 | 47.97 | **75** | 24 | 51 | 41 |

The published column is not the same quantity: `comparison.md` §3 records "a
balls-in-bins tail over the **accepted** side (measured, 25 graphs)", so it
excludes the node's own deterministic picks. The implied-accepted column above
subtracts them for a like-for-like read.

On that basis ours run higher for four of the five, which is expected — an
extreme-value statistic grows with sample size, and these are 200 graphs
against 25. **M4 is the exception**: ours implies 28 accepted against a
published 29, where more graphs should have produced at least as large a
maximum. The discrepancy is small and may be a definitional difference in the
symmetric case, where a link's "accepted side" is ambiguous because one accept
establishes both directions. It is recorded here rather than resolved.

## 3. What this changes

**The state axis is now measured on both instruments**, at 200 graphs rather
than 25, and the means agree exactly. The comparison's weakest axis is no
longer its deciding one.

**The worst case separates the designs more sharply than the mean suggests.**
M3's busiest node holds 64 connections against M4's 36 — and against a mean of
38 and 16. A deployment sizing connection limits reads the second column, not
the first.

**M1 and M2 are worse than their means imply too**, at 75 held connections on
the busiest node.

## 4. Limits

- **One network size and one adversarial fraction.** All cells are N = 20 000,
  μ = 0.2. The maximum is a balls-in-bins tail and grows with population;
  these figures do not transfer to a larger deployment unchanged.
- **The maximum is sample-size dependent** by construction. Ours is over 200
  graphs (M2: 40); a longer run would find a larger one. It is a measured lower
  bound on the worst case, not a bound on it.
- **Down nodes still hold their links.** Under churn a node's slots are
  occupied whether or not the counterparty is up, and this measurement is taken
  post-churn on up-honest nodes only.
