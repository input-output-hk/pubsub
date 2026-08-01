# Contract — node CLI selection surface (017)

The `pubsub-node` binary's configuration contract after this feature.
Operator-facing help text carries none of the FR citations below (they
anchor this artifact only).

## Flags

| flag | type / domain | absent means | present means |
|---|---|---|---|
| `--relay-bucket-count <B>` | integer ≥ 2 (0 and 1 rejected) | relay dialing ungated; relay acceptors do not verify | relay gate at B on dial **and** acceptance (one value, both sides — the agreement condition) |
| `--relay-pick-count <K>` | integer ≥ 0 | dial every gate survivor | draw exactly min(K, survivors) seeded uniform picks; 0 = dial no relay links |
| `--relay-accept-cap <C>` | integer ≥ 0 | unbounded relay acceptance | at most C accepted relay downstreams per topic; over-capacity refused with explicit `Rejected`; 0 = serve none |
| `--relay-symmetric` | bool | directional relay links | symmetric handshake + mirroring (renames `--symmetric-edges`); composes with any knob combination |
| `--relay-accept-unverified` | bool | acceptors verify iff gated | relay acceptors skip predicate verification (trusting acceptors) |
| `--publisher-bucket-count <B>` | as relay | — | publisher-seam mirror (publisher hash domain) |
| `--publisher-pick-count <K>` | as relay | — | publisher-seam mirror; 0 = accept-only publisher seam |
| `--publisher-accept-cap <C>` | as relay | — | publisher-seam mirror |
| `--publisher-accept-unverified` | bool | — | publisher-seam mirror |
| `--selection-seed <u64>` | u64 | (see validation) | sampling seed; expanded at the loader to the 32-byte constructor seed |
| `--fanout-strategy <kind>` | `forward-to-all` \| `forward-to-relays` | **`forward-to-all`** (default flipped by this feature) | `forward-to-relays` = the M3-exclusivity switch |
| `--genesis <u64>` | u64, default 0 | epoch-0 nonce | unchanged |

Deleted (no aliases, pre-release): `--relay-strategy`,
`--relay-acceptance-strategy`, `--relay-degree`, `--publisher-strategy`,
`--publisher-acceptance-strategy`, `--publisher-degree`, `--bucket-count`,
`--cap-buffer`, `--symmetric-edges`.

## Seam activation

- **Relay seam**: always active. No knobs = ungated, uncapped, dial-all
  (the pre-017 default behaviour, preserved). `--relay-pick-count 0` = dial
  none (acceptance still serves — the M1 shape).
- **Publisher seam**: off by construction with no `--publisher-*` flags (no
  dial pass; inbound publisher requests dropped — unchanged seam-off
  semantics). Any publisher knob activates it, **subject to**: activation
  requires at least one dial knob (`--publisher-pick-count`, 0 permitted,
  or `--publisher-bucket-count`) — acceptance-side knobs alone are rejected
  (spec Clarifications 2026-07-31).

## Startup validation matrix (all exit code 2 with an actionable message)

| condition | outcome |
|---|---|
| any bucket count = 0 or 1 | rejected (gating is signalled by the flag; a one-bucket gate is vacuous). Deliberate divergence: the sweep config accepts `bucket_count = 1` as the ungated axis point — see `contracts/sweep-config.md` |
| any seam pick count ≥ 1 and `--selection-seed` absent | rejected (names the missing flag) |
| `--selection-seed` present and no seam has pick count ≥ 1 | rejected as unused |
| publisher seam activated solely by acceptance-side knobs | rejected (names `--publisher-pick-count 0` as the accept-only spelling) |
| `--*-accept-unverified` without that seam's bucket count | rejected as unused (the gate is already vacuous) |

Removed validation: the `--symmetric-edges`-requires-hash-gated rule
(symmetric now composes with every plane point — uniform + symmetric is the
real M4).

## Behavioural notes (operator-visible)

- Accept cap 0 refuses with an explicit `Rejected` (the dialer removes its
  pending entry) — a change from the old disabled-seam silent drop
  (017 FR-013).
- Fan-out default: a node with publisher links and no `--fanout-strategy`
  flag runs M5 semantics (every held message over both link classes), not
  M3 exclusivity — stated in help/quickstart (017 FR-009).
- Parameter guidance documented beside the recipes (not computed by the
  program): balanced-point bucket count B ≈ candidates/K; accept-cap
  headroom ⌈K + c·√K⌉ with c ≈ 3; symmetric nodes anchor caps on ≈ 2K
  (N-032 recorded behaviour).

## Model recipes (canonical set in quickstart.md)

Four families × M1–M5, all knob-only: picks only (formal models); + bucket
counts (hash-gated); + accept caps (capped); both (gated + capped). M1 =
M5 with `--relay-pick-count 0`. M3 alone carries
`--fanout-strategy forward-to-relays`.
