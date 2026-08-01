# Quickstart: the selection plane and the model recipes (017)

Selection is configured by two per-seam knobs — **the bucket count** (the
hash-gate width; present ⇒ gated, B ≥ 2) and **the pick count** (exactly
min(pick count, gate survivors) seeded uniform picks; absent ⇒ every
survivor; 0 ⇒ none) — plus a per-seam **accept cap** (absolute serving
bound) and the **symmetric** relay switch. There are no strategy kind
names. Registries and shared setup are unchanged from the 015 quickstart.

Common prefix for every recipe below:

```sh
PREFIX="pubsub-node --self-id a \
  --subscription-list subs.toml --topic-registry topics.toml \
  --genesis 42 --selection-seed 7"
```

`--genesis` is the shared public randomness (the gate's context — both
endpoints must agree on it); `--selection-seed` is the per-node sampling
randomness (required whenever any seam has a pick count ≥ 1; see the
privacy note at the end). Same configuration ⇒ same topology.

Constants: `$RF` (relay picks), `$S1` (M3's s − 1 publisher picks),
`$KIN`/`$KOUT` (M5/M1 degrees), `$B_RELAY`/`$B_PUB` (bucket counts),
`$CAP_RELAY`/`$CAP_PUB` (accept caps).

**Choosing values** (guidance, not computed by the program): the balanced
point is B ≈ candidates/K (expected survivors ≈ K); the accept-cap headroom
guidance is ⌈K + c·√K⌉ with c ≈ 3 (e.g. K = 8 → 17). A pinned B larger than
a topic's candidate count can leave zero survivors — and there is no
retry — so small topics are the parameter-setter's responsibility (the old
derived-B connect-to-all floor is gone).

## Family 1 — the formal models (picks only)

```sh
# M1  push-only (= M5 at k_in = 0)
$PREFIX --relay-pick-count 0 --publisher-pick-count $KOUT
# M2  pull baseline (the formal selection family exactly)
$PREFIX --relay-pick-count $RF
# M3  pull + initiation links (the one marked fan-out case)
$PREFIX --relay-pick-count $RF --publisher-pick-count $S1 \
        --fanout-strategy forward-to-relays
# M4  bidirectional — the real M4 (uniform picks + constructed reciprocity)
$PREFIX --relay-pick-count $RF --relay-symmetric
# M5  directed k-in/k-out, both classes carry everything (default fan-out)
$PREFIX --relay-pick-count $KIN --publisher-pick-count $KOUT
```

M4 here carries the label without qualification: one-sided uniform picks
plus the symmetric handshake give minimum degree ≥ `$RF` and mean ≈ 2·`$RF`
— the formal model's defining floor.

## Family 2 — hash-gated versions (add bucket counts; picks stay)

Add `--relay-bucket-count $B_RELAY` and/or
`--publisher-bucket-count $B_PUB` to the family-1 recipes (on their active
seams). Realised out-degree becomes min(pick count, survivors) with
survivors ≈ candidates/B — below the balanced B the pick count binds, above
it the gate does. Acceptors verify the gate automatically wherever a bucket
count is present.

## Family 3 — capped versions (add accept caps to family 1)

Add `--relay-accept-cap $CAP_RELAY` and/or
`--publisher-accept-cap $CAP_PUB`. An over-capacity request is refused with
an explicit rejection (the dialer abandons that edge — no retry). A cap of
0 serves no one.

## Family 4 — gated + capped (families 2 + 3 combined)

The E12 defended configuration: adversarial slot occupancy is bounded
toward ≈ cap/B per victim.

## Cautions

- **Fan-out default is `forward-to-all`.** A node with publisher links and
  no `--fanout-strategy` flag runs M5 semantics (every held message over
  both link classes). M3's exclusivity (publisher links carry only their
  owner's publications) requires the explicit
  `--fanout-strategy forward-to-relays`.
- **Symmetric × accept cap** is expressible but sits on recorded, deferred
  semantics (N-032): the cap's scan counts mirrored own-dials while the
  gate fires only on peer-initiated requests, so realised degree can exceed
  the cap arrival-order-dependently. A symmetric node's healthy degree is
  ≈ 2× the pick count — anchor caps there, not on the directional guidance.
- **Accept-only publisher seam** is `--publisher-pick-count 0` (+ optional
  acceptance knobs). Acceptance-side publisher knobs alone are rejected at
  startup.
- **Trusting acceptors** (gated dialers, non-verifying acceptors — the
  comparison arm): `--relay-accept-unverified` beside
  `--relay-bucket-count`.
- **Sampled picks assume a complete candidate view at dial time.** The pick
  set is a function of the whole view: a node dialing on a partial view
  (staggered boot) draws from the subset it sees — fewer than the pick
  count if the subset is smaller, with no retry — and any re-dial after the
  view grows draws a *different* sample whose union inflates degree
  (add-only; measured: M4 fleet degrees exceed 2× the pick count until an
  epoch rotation). Today's single readiness heartbeat over a fully-synced
  snapshot avoids this by construction; the implementation note recorded
  with this feature (N-038) carries the revisit trigger.

## Verifiability

Every dialed edge under a present bucket count is acceptor-checkable —
gated recipes (families 2 and 4) are the protocol track's verifiable
region, pick count or not (dialing fewer than all valid edges is not a
violation). Recipes without a bucket count use fully private selection —
the formal family, experiments-only on the protocol track. A gate-failing
dial is provable misbehaviour; v1 silently drops it (evidence collection is
the incentive/chain layer's, N-036).

## Experiments: sweeps and the validation procedure

The sweep config speaks the same coordinates (`pick_count`,
`bucket_count`, `accept_cap`, `accept_unverified`, `symmetric` — see
`contracts/sweep-config.md`); `bucket_count = 1` and `pick_count = 0` are
legal there as axis boundary points (E10's ungated cell; E8's k = 0
reductions).

Feature validation (the spec's contract):

1. **Commit A** (refactor neutrality): rebuild and re-run the recorded
   baseline sweeps; `runs.jsonl`/`aggregates.json` must byte-diff identical
   (`notes/experiments-baselines/README.md` procedure; manifests differ in
   tool commit and config text only).
2. **Commit B** (the deliberate derivation change): re-run the
   m2-comparison operating points, confirm statistical agreement with the
   formal values, update `docs/experiments/m2-comparison.md`, record fresh
   baseline generations, and record the first **M4 baseline** from the
   shipped symmetric configuration (~25 s at the operating point,
   `--workers 10`).
3. The determinism battery (value-level, replay-by-seed, worker-count
   invariance) must pass unchanged throughout.

## Seed privacy note

The formal models prescribe private, unpredictable per-node selection
randomness. `--selection-seed` is a low-entropy operator flag — a prototype
stand-in, adequate against oblivious adversaries (every current
experiment). Fleet-shared seed values still yield per-node-independent
draws (the node's identity is mixed into the derivation), but anyone who
knows the seed can recompute the picks. Before uniform selection faces an
adaptive adversary or a real deployment, provisioning must become per-node
secret material (N-037).
