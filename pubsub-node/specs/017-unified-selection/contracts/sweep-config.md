# Contract — sweep-config delta (017)

Delta to the 016 sweep-config contract
(`specs/016-experiments-framework/contracts/sweep-config.md`); everything
not named here is unchanged (population/churn/publishes fields, output
contract, CLI invocation, seed threading).

## Strategy table (per class: `honest_strategies` / `adversarial_strategies`)

Removed fields: `connection` (kind string), `acceptance` (kind string),
`target_degree`, `cap_buffer`.

Added fields (all optional unless noted):

| field | type / domain | absent means | notes |
|---|---|---|---|
| `pick_count` | integer ≥ 0 | dial every gate survivor | 0 is legal: the k_in/k_out = 0 boundary (E8's M5 → M1/M2 reductions as plain values) |
| `bucket_count` | integer ≥ 1 | ungated | **1 is legal here** (the ungated point on a bucket-count axis — E10), unlike the operator CLI which rejects it |
| `accept_cap` | integer ≥ 0 | unbounded acceptance | 0 = serve none (explicit `Rejected`) |
| `accept_unverified` | bool, default `false` | acceptors verify iff `bucket_count` present | the trusting-acceptors comparison arm |
| `symmetric` | bool, default `false` | directional relay links | the symmetric handshake switch — new to the config; required for the M4 baseline (017 FR-027) |

`fanout` is unchanged: `forward-to-relays` \| `silent-relay`
(`forward-to-all` remains rejected — populations are relay-only until the
publisher-pair feature; extensionally identical anyway).

Validation: a table whose `pick_count` ≥ 1 needs no seed field — the
driver's per-participant sampler seeds (master-seed chain, unchanged) feed
the selection instances. `bucket_count = 0` rejected. Unknown fields
rejected (`deny_unknown_fields`, unchanged).

## Axis vocabulary

- `target_degree` axis parameter **renamed `pick_count`** (applies to both
  class tables, as before).
- `bucket_count` **added** as an axis parameter.
- Axis values obey the table domains above (so `bucket_count = 1` and
  `pick_count = 0` are legal axis points).

## Compatibility and validation-contract hooks

- Shipped configs (M2 comparison + smoke) are rewritten to the coordinate
  vocabulary; the manifest embeds the config verbatim, so manifests differ
  across the feature — permitted. At commit A, `runs.jsonl` and
  `aggregates.json` MUST byte-diff identical against the recorded baselines
  (017 FR-026).
- A new shipped configuration expresses the M4-completing recipe
  (`symmetric = true`, `pick_count` set, no `bucket_count`) whose recorded
  baseline is required by 017 FR-027. Its analytics run under the existing
  M2 extraction (research R7 — symmetric populations yield a symmetric
  relay digraph; no new dispatch variant).
- Kind-name spellings (`uniform-sampler`, `connect-to-all`, `hash-gated`,
  `accept-from-all`, `bounded`, `hash-gated-bounded`) are no longer
  accepted anywhere in the config; there is exactly one spelling per plane
  point.
