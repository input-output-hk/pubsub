# ADR 0041: Publisher-pair experiment configuration — model-coherence validation, per-model extraction, and the sends-by-kind split

**Status**: Accepted
**Date**: 2026-08-03
**Feature**: the publisher-pair experiment configuration (the follow-up
declared in 017's out-of-scope list; deliberately on the small-feature path —
no spec dir, this ADR is the decision record)

## Context

The experiment program's model-family stage (E6–E8,
`docs/experiments-program.md` §5) compares the framework's realised behaviour
against the formal M1–M5 models' published coverage laws, cost values, and
Monte-Carlo grids. The node has implemented all five models' mechanics since
015/017: the relay mesh, the publisher link pair for standing initiation
links, and the symmetric handshake with a pick count (exactly M4). What
blocks E6 and E8 is framework-side only, and all three gaps were recorded as
deliberate boundaries to be removed by this feature:

- population construction is relay-only (`publisher: None` — 017 FR-019);
- `forward-to-all` is rejected as a config fan-out (extensionally identical
  to `forward-to-relays` in a relay-only population — the 016 program note);
- the analytics dispatch ships M2 extraction only: publisher links are seed
  edges, never propagation edges (016's `DisseminationModel`, v1).

Two design constraints frame the extension. First, 017 deliberately moved
the sweep config from strategy kind names to **plane coordinates**
(`pick_count`/`bucket_count` axes with boundary values legal), so that
off-model points — mixed pairs, E10's whole (B, K) plane — are expressible
without being "a model"; the publisher extension must not resurrect kind
names. Second, the instrument's credibility rests on the ADR 0036 output
contract and the recorded baseline generations: any row-schema change is a
recorded re-baseline event, so the feature must contain exactly one.

## Decision

**The `model` field stays the analytics dispatch; coordinates stay free;
parse-time validation ties them.** The sweep config's `model` names only the
extraction/seed/goodness rule the realised-graph analytics run — what it
means today. The per-class strategy tables gain a **publisher coordinate
table** mirroring the relay table's fields (`pick_count`, `bucket_count`,
`accept_cap`, `accept_unverified`), building the publisher
`Selection`/`UnifiedAcceptance` pair the node already supports;
`forward-to-all` becomes a legal fan-out kind. Coherence is enforced where
unknown models and zero counts are rejected today — at parse time, before
any run executes:

- `m3` requires publisher links present and `forward-to-relays`;
- `m5` and `m1` require publisher links present and `forward-to-all`;
- `m2` and `m4` require relay-only tables; `m4` additionally requires
  `symmetric`.

A config whose coordinates contradict its model is rejected with the
offending coordinate named. "One config name yields consistent wiring and
measurement" is thereby a checked invariant, not a coupling.

**Per-model extraction and seed rules.** `DisseminationModel` grows the
family; each variant owns its propagation-edge rule and per-publisher seed
set:

| Model | Wiring (honest class) | Propagation edges | Seeds per publisher |
|-------|----------------------|-------------------|---------------------|
| M2 | relay-only, `forward-to-relays` | relay downstream | publisher alone |
| M3 | relay + publisher pair, `forward-to-relays` | relay downstream | publisher + its publisher-link targets |
| M4 | relay-only, symmetric, pick count | relay downstream (bidirectional by construction) | publisher alone |
| M5 | relay + publisher pair, `forward-to-all` | both kinds' downstream | publisher alone |
| M1 | M5 wiring at `k_in` = 0 | = M5 | publisher alone |

M4 shares M2's rules but exists as its own name so configurations are
self-describing and the symmetric requirement is checkable. M1 is M5's
`k_in` = 0 boundary row — a name for the reduction, not separate machinery.

**The M3 parameter mapping.** The model's `s` counts the publisher itself,
so `s` maps to a publisher-seam pick count of **s−1** dialed initiation
links, configured population-wide on the honest class: every node is a
potential publisher, and the drain draws publishers from the honest
population.

**The sends-by-kind split, unconditional.** Every run record carries the
publish drain's send counts split by carrying link kind (relay/publisher),
attributed at emission: a send is relay-attributed iff the sender holds an
`Active` relay downstream link for the recipient — a recipient reachable
over **both** kinds is attributed to the relay mesh, since the deduped
single send would have happened over it regardless — and
publisher-attributed otherwise. A second per-run identity accompanies the
existing one (sends = first receipts + suppressed + sent-to-down): relay +
publisher = total sends, for every model, with degenerate columns constant
at zero rather than absent. The split's reading is model-dependent: under M3 it is
relaying vs seeding own publications; under M5, pull-serving vs
push-forwarding; under M2/M4 the publisher column is identically zero, and
under M1 the relay column is — the M5 grid's boundary reductions become
directly visible in the accounting. This amends ADR 0036's output contract
and is the feature's one row-schema change, hence its one re-baseline of the
recorded generations, with the documented M2 comparison re-validated
(values unchanged) against the new rows.

**Contract-of-record homes.** Merged feature dirs stay frozen; the living
config contract is `configs/experiments/README.md` (schema and validation
rules) and `docs/experiments-program.md` (program semantics and statuses).

## Consequences

- E6 and E8 lose their machinery blocker; with E7 already runnable, every
  model-family fidelity check waits only on program work (pinning cells from
  the formal team's published grids).
- Exactly one new baseline generation covers the row-schema change; the
  `m4-uniform-symmetric` config's switch to declaring `model = "m4"` rides
  the same generation (its manifest changes anyway), so no second
  re-baseline occurs.
- Off-model plane points stay expressible: coherence validation constrains
  only configs that *claim* a model name, never the coordinate space itself.
- The parked node-side direction — a model-level trait owning per-model
  `apply` dispatch arms — stays parked: the fidelity checks need analytics-
  side dispatch only, and the core stays untouched.

## Alternatives rejected

- **The model field as a master switch** (name implies the wiring; tables
  fill in numerics only). Impossible to misconfigure, but it resurrects
  strategy kind names and makes off-model points inexpressible — undoing
  017's deliberate move to coordinates.
- **Emitting the kind split conditionally** (only when publisher links
  exist). Preserves existing artifact bytes but forks the row schema by
  configuration — hostile to the aggregates-as-pure-function-of-rows
  contract and to any cross-model tooling reading the rows.
- **Wiring the publisher pair per designated publisher** rather than
  population-wide. Contradicts the models (every node owns initiation
  links) and would entangle population construction with the drain's
  publisher draw.
- **Retro-editing the merged 016/017 `contracts/sweep-config.md`.** Frozen
  feature records stay frozen; the living homes absorb the schema.
