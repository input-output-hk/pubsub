# Data model — 017-unified-selection

Entities, field domains, and derivations. `NodeState`, the wire vocabulary,
the handlers, and the receive path are **unchanged** by this feature; every
entity here lives in the strategy/configuration layer.

## The selection plane point

A per-seam coordinate pair. Domains differ by surface — the core type means
the same thing everywhere; each edge enforces its consumer's intent:

| knob | core (constructors) | node CLI | sweep config |
|---|---|---|---|
| bucket count | `Option<usize>`, ≥ 1 (0 rejected; absent ≡ 1) | absent or ≥ 2 (1 rejected) | absent or ≥ 1 (1 = ungated axis point) |
| pick count | `Option<usize>`, any value (0 = dial none) | same | same (0 = k_in/k_out boundary axis point) |
| accept cap | `Option<usize>`, any value (0 = serve none) | same | same |

Legacy strategies as points (documentation vocabulary only — no code
carries these names after this feature):

| point | bucket count | pick count |
|---|---|---|
| connect-to-all | absent | absent |
| uniform sampler (formal family) | absent | K |
| hash-gated | B ≥ 2 | absent |
| gated + capped | B ≥ 2 | K |

## `Selection` (dial seam; implements `ConnectionStrategy`)

```
Selection {
    self_id:      PeerId,          // requester side of the edge predicate; mixed into the draw preimage (commit B)
    kind:         LinkKind,        // Relay | Publisher — selects the hash domain
    symmetric:    bool,            // relay instances only: unordered-pair predicate + symmetric vocabulary
    bucket_count: Option<usize>,   // gate width; None ≡ 1 (everyone survives)
    pick_count:   Option<usize>,   // None = all survivors; Some(k) = exactly min(k, survivors)
    seed:         [u8; 32],        // sampling seed; read only when pick_count ≥ 1
}
```

**`expected_links(&NodeView)` per subscribed topic**:

1. *Gate*: survivors = `candidates_for(topic)` (sorted, self-excluded)
   filtered by the seam's predicate at `bucket_count` — `is_valid_edge` /
   `is_valid_edge_publisher` / `is_valid_edge_sym` — skipped entirely when
   `bucket_count` is absent (≡ B = 1, the predicate's short-circuit).
2. *Pick*: `pick_count` absent → all survivors; `Some(k)` →
   `sample(ChaCha20Rng::from_seed(topic_seed), survivors.len(),
   min(k, survivors.len()))` mapped through the ordered survivor list;
   `Some(0)` → empty.

The result is a pure function of (fields, view) — order-independent in the
candidate set, stable across `Heartbeat` re-dials within an epoch,
re-drawn on `Epoch` (commit B, via the nonce in the preimage).

**`topic_seed` derivation**:

- *Commit A (byte-identity pin)*: `SHA-256("experiments/uniform-sampler/v1"
  ‖ seed ‖ topic-bytes)` — concatenated, no length prefixes, no nonce, no
  self-id; value-identical to the deleted `UniformSampler`.
- *Commit B (final)*: `SHA-256( lp(domain) ‖ lp(seed) ‖ lp(self-id key
  bytes) ‖ nonce_le8 ‖ lp(topic-bytes) )` with `lp` = `push_len_prefixed`
  and the domain selected per seam by the instance's `LinkKind` — mirroring
  the edge predicate's per-seam domains:
  `pubsub/uniform-selection/relay/v1` /
  `pubsub/uniform-selection/publisher/v1`. Properties carried by each
  component: per-seam domain → the relay and publisher instances of one
  node draw independently; self-id → fleet-shared-seed independence;
  nonce → epoch re-randomisation + heartbeat stability; length prefixes →
  no concatenation collisions across distinct tuples.

## `UnifiedAcceptance` (acceptance seam; implements `ConnectionAcceptanceStrategy`)

```
UnifiedAcceptance {
    self_id:    PeerId,          // candidate side of the verified edge
    kind:       LinkKind,        // which link class it admits and counts (disjoint capacities)
    symmetric:  bool,            // verify with the unordered-pair predicate
    gate:       Option<usize>,   // bucket count to verify; None = no verification
    accept_cap: Option<usize>,   // None = unbounded; Some(0) = serve none
}
```

**`admit(emitter, topic, &NodeView)` decision order** (all shared helpers
reused verbatim):

1. `admit_prelude(kind, …)` — membership check (`RejectMembership`,
   silent), idempotent already-held re-Accept, one borrow-only scan
   returning the accepted-on-topic count.
2. *Gate* (`gate = Some(B)`): predicate fails → `RejectIllegitimate`
   (silent drop; the documented future evidence-collection point).
3. *Cap* (`accept_cap = Some(c)`): count ≥ c → `RejectOverCapacity`
   (explicit `Rejected`; dialer cleans up its pending entry). `c = 0`
   refuses every new link this way — the deliberate behavioural change from
   the deleted `AcceptNone`'s silent drop.
4. Otherwise `Accept`.

The `Admission` enum, reply semantics, and severance are unchanged.
"Verification follows the seam's bucket count with an explicit opt-out" is
resolved at construction: the edge passes the seam's bucket count as `gate`,
or `None` when `--*-accept-unverified` is set.

## Construction parameters (ADR 0028 reshaped — phase 1 dissolves)

```
SelectionParams  { self_id, kind, symmetric, bucket_count, pick_count, seed }
AcceptanceParams { self_id, kind, symmetric, bucket_count, accept_cap }
   // bucket_count here is the post-opt-out gate value (None when unverified)
```

`NodeStrategies` keeps its five fields (relay pair, optional publisher
pair, `symmetric_edges`); construction becomes one fallible call taking the
relay param pair plus `Option<(SelectionParams, AcceptanceParams)>` for the
publisher seam — one `StrategyConfigError` map site at the edge (absorbs
§1.2 item 6). `NodeStrategies::relay_only` is unchanged. Core-domain
validation: bucket count 0 rejected (`InvalidParameter`); everything else
total. `require_target_degree` / `validate_bucket_count` are deleted with
their consumers.

## Node CLI surface (the contract detail is `contracts/node-cli.md`)

Flags added: `--relay-bucket-count`, `--relay-pick-count`,
`--relay-accept-cap`, `--relay-symmetric`, `--relay-accept-unverified`,
`--publisher-bucket-count`, `--publisher-pick-count`,
`--publisher-accept-cap`, `--publisher-accept-unverified`,
`--selection-seed <u64>`. Flags deleted: `--relay-strategy`,
`--relay-acceptance-strategy`, `--relay-degree`, `--publisher-strategy`,
`--publisher-acceptance-strategy`, `--publisher-degree`, `--bucket-count`,
`--cap-buffer`, `--symmetric-edges`. `--fanout-strategy` default flips to
`forward-to-all`. Seed expansion at the loader:
`SHA-256(lp("pubsub/selection-seed/v1") ‖ seed_le8)` → the constructor's 32
bytes.

## Sweep-config strategy table (delta contract: `contracts/sweep-config.md`)

Fields removed: `connection`, `acceptance`, `target_degree`, `cap_buffer`.
Fields added: `pick_count`, `bucket_count`, `accept_cap` (all optional),
`accept_unverified`, `symmetric` (bool, default false). `fanout` vocabulary
unchanged (`forward-to-relays` | `silent-relay`). Axis parameters:
`target_degree` renamed `pick_count`; `bucket_count` added. Per-participant
sampler-seed derivation from the master seed: unchanged, threaded into
`Selection.seed`.

## Deletion inventory (spec SC-008)

Types/files: `ConnectToAllCandidates`, `HashGatedConnection`, `DialNone`,
`ConnectionStrategyKind`; `AcceptFromAllCandidates`, `BoundedAcceptance`,
`HashGatedAcceptance`, `HashGatedBoundedAcceptance`, `AcceptNone`,
`AcceptanceStrategyKind`; `UniformSampler` (`SilentRelay` stays
experiments-only). Functions: `resolve_buckets`, `bucket_count(len, k)`,
`accept_cap(k, c)` (formulas → quickstart guidance),
`require_target_degree`, `validate_bucket_count` (superseded by core-domain
checks). Exports: `is_valid_edge_publisher` joins the public predicate set.
`NodeView` unchanged (`candidates_len` keeps its non-derivation consumers;
doc updated).

## Invariants

- Verifiability ⟺ bucket count present: every dialed edge under B ≥ 2 is
  acceptor-checkable regardless of pick count; bucket count absent = fully
  private selection (experiments-only on the protocol track).
- Seam agreement: one per-seam bucket-count value feeds both the dial gate
  and the acceptor's verification — the agreement condition, by
  construction at the edge.
- Per-seam draw independence: a node's relay and publisher `Selection`
  instances derive under separate domains, so their picks are uncorrelated
  even with one shared seed, ungated seams, and equal pick counts — the
  sampling twin of the edge predicate's relay/publisher domain split.
- Publisher instances are never symmetric (no flag exists; params default
  `false`), preserving ADR 0034's boundary.
- The M2 point (bucket absent, pick = RF) reproduces the formal selection
  family exactly: RF uniform picks without replacement per topic
  (min(RF, candidates) degeneracy included).
