# ADR 0040: Selection randomness derivation — the seed chain, per-seam draw domains, and the privacy stand-in

**Status**: Accepted
**Date**: 2026-08-01
**Feature**: `specs/017-unified-selection/` (companion to ADR 0039 — separable
because the derivation is independently revisable from the plane itself:
changing a preimage re-baselines experiments, changing the plane reshapes
configuration)

## Context

The selection plane's pick count (ADR 0039) draws seeded uniform picks — the
first node behaviour that needs sampling randomness. Hash-gating never did:
its randomness is the public epoch nonce both endpoints hash. The draw needs
a seed the node owns, a derivation whose properties match the formal models'
assumptions where they can be met, and an explicit record of where they
cannot: the models prescribe *private, unpredictable* per-node selection
randomness, and a prototype has no secret-material provisioning.

Two constraints shaped the derivation's landing. First, the experiments
driver already injects per-participant 32-byte sampler seeds derived from a
sweep's master seed, and the recorded M2 baselines pin that path's outputs —
so the refactor that promoted the sampler had to reproduce its derivation
byte-exactly before any honest change (the A→B ordering of the feature's
validation contract). Second, the first design of the honest preimage carried
no seam component; with the CLI expanding one `--selection-seed` for both
seams, an M3/M5 node's relay and publisher instances would have derived the
same per-topic RNG stream — publisher targets identical to relay upstreams at
equal pick counts on ungated seams (the I2 finding, surfaced at the Phase 2
implementation checkpoint). That contradicts both the models'
independent-draws assumption and the independence the edge predicate's
per-seam hash domains already provide for gated selection.

## Decision

**The seed chain.** The operator flag `--selection-seed <u64>` (required iff
a seam has a pick count ≥ 1) expands at the loader edge via
`selection_seed_bytes`:

```text
constructor seed = SHA-256( lp("pubsub/selection-seed/v1") ‖ seed_le8 )
```

with `lp` the crate's one length-prefix primitive (u32 BE length ‖ bytes).
This is a pure format expansion — no identity, no nonce — so the
`Selection` constructor keeps taking 32 opaque seed bytes and the
experiments driver's per-participant injection path is untouched.

**The per-topic draw preimage.** Each `Selection` instance derives one RNG
seed per topic:

```text
topic seed = SHA-256( lp(domain) ‖ lp(seed) ‖ lp(self-id key bytes) ‖ nonce_le8 ‖ lp(topic) )
```

and draws `min(pick count, survivors)` indices over the sorted, self-excluded
survivor list from a ChaCha20 stream keyed by it. Each component carries one
property:

- **the per-seam domain**, selected by the instance's link kind —
  `pubsub/uniform-selection/relay/v1` / `pubsub/uniform-selection/publisher/v1`
  — makes one node's relay and publisher draws independent even over one
  shared seed (closing I2), mirroring the edge predicate's per-seam domain
  split. There is no symmetric draw domain: the symmetric switch changes the
  gate predicate and the handshake vocabulary, never the draw.
- **self-identity key bytes** make a fleet-shared seed value yield
  per-node-independent draws; mixed at the strategy level rather than only
  the CLI edge, so the property holds for every construction site, the
  experiments driver included.
- **the epoch nonce** re-randomises picks exactly as an epoch re-shuffles
  gated edges, and keeps heartbeat re-dials stable within an epoch
  (ADR 0031's rotation seam, exercised the day the epoch event fires).
- **the length prefixes** rule out concatenation collisions across distinct
  tuples, matching the edge predicate's preimage conventions (variable-width
  components prefixed, the nonce fixed-width).

**The two-commit derivation swap.** Commit A reproduced the deleted
`UniformSampler`'s derivation byte-exactly (`experiments/uniform-sampler/v1`,
concatenated preimage, no self-identity or nonce) so the recorded baseline
sweeps byte-diffed identical across the whole strategy collapse — the
refactor-neutrality proof. Commit B landed this ADR's derivation as the one
deliberate result change, followed by exactly one re-baseline and the
statistical m2-comparison re-run. Between the two, the seed-property battery
was written first and demonstrably failed against the commit-A derivation.

**The privacy stand-in posture.** The formal models prescribe private,
unpredictable per-node selection randomness. A low-entropy operator flag is
a deliberate prototype stand-in: reproducibility, not secrecy — anyone who
knows the seed can recompute the picks, and it is model-adequate against
oblivious adversaries only (the entire current experiment program). Before
uniform selection faces an adaptive adversary or a real deployment,
provisioning must become per-node secret material or derive from the
identity key under proper domain separation (the implementation note carries
the trigger: the first adaptive-adversary experiment or the real-crypto
identity work).

## Consequences

- Reading the two flags together: `--genesis` is the shared public
  randomness (the gate's context — both endpoints must agree on it);
  `--selection-seed` is the per-node, notionally private randomness (the
  sampler's context). Same configuration and seed ⇒ same topology.
- The M2 experiment point keeps the formal selection family's semantics
  exactly (RF uniform picks without replacement per topic); its values
  changed with the preimage, which is why the baselines were re-recorded
  once and the m2-comparison re-validated statistically rather than
  byte-wise.
- A future preimage change is a recorded event: the domain strings' version
  suffix is the revision knob (pre-release iterations keep `v1`), and any
  change re-runs the same re-baseline procedure.

## Alternatives rejected

- **One shared draw domain** (the first design). Correlates an M3/M5 node's
  two seam instances' draws — the I2 defect; undetectable by the commit-A
  byte gate (experiment populations are relay-only) and latent until the
  publisher-pair experiments feature.
- **One domain plus a kind tag as a preimage component.** Cryptographically
  equivalent; rejected because separate domain constants are the established
  edge.rs pattern for exactly this independence property.
- **Mixing self-identity only at the CLI edge.** Leaves the experiments path
  able to correlate picks if a config ever shares participant seeds; the
  property belongs to the draw itself.
- **Keeping the `experiments/uniform-sampler/v1` domain permanently.** A
  misnomer the moment the sampler is a node capability; the one re-baseline
  was already budgeted by the validation contract.
- **A 32-byte hex seed flag.** Hostile ergonomics beside `--genesis`, and
  false comfort: entropy in the flag does not make the value private. The
  u64 matches the prototype's reproducibility posture.
