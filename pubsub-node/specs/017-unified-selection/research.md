# Research — 017-unified-selection

Phase 0 decisions. Each entry: Decision / Rationale / Alternatives
considered. Code facts verified against `main` at branch time.

## R1 — One concrete type per seam trait

**Decision**: one dial-side type `Selection` (new file
`strategies/connection/selection.rs`) implementing `ConnectionStrategy`, and
one acceptance-side type `UnifiedAcceptance` (new file
`strategies/acceptance/unified.rs`) implementing
`ConnectionAcceptanceStrategy`. Fields:

- `Selection { self_id: PeerId, kind: LinkKind, symmetric: bool,
  bucket_count: Option<usize>, pick_count: Option<usize>, seed: [u8; 32] }`
- `UnifiedAcceptance { self_id: PeerId, kind: LinkKind, symmetric: bool,
  gate: Option<usize>, accept_cap: Option<usize> }`

`UnifiedAcceptance.gate` is the bucket count the acceptor verifies; `None`
means no verification — the edge maps the verification opt-out flag to
`gate: None` while the dial side keeps the seam's bucket count, so
"verification follows B, with an explicit opt-out" is a construction-time
resolution, not a runtime branch.

**Rationale**: the traits and their injection seams are load-bearing
(experiments inject `SilentRelay`; tests inject concrete instances) and stay
untouched; only the closed kind-enum layer and the per-kind types collapse.
Two fields of `Option` per type make the four legacy behaviours per seam
coordinates, matching spec FR-001/FR-010.

**Alternatives considered**: keeping the compound types and adding a uniform
kind (rejected: perpetuates four implementations and the kind enums the spec
deletes); a single type serving both traits (rejected: the two seams' inputs
and outputs differ — expected-set vs admission — and the acceptance side
never samples).

## R2 — Pick-draw mechanics and the commit-A byte-identity path

**Decision**: the draw in `Selection::expected_links`, per subscribed topic:

1. Collect survivors in the view's sorted, self-excluded order
   (`candidates_for`), filtered by the edge predicate when
   `bucket_count ≥ 2` under the seam/symmetric domain (absent ≡ B = 1: the
   predicate's existing `buckets <= 1` short-circuit admits everyone —
   filtering is skipped entirely).
2. `pick_count` absent → all survivors. Present →
   `rand::seq::index::sample(&mut ChaCha20Rng::from_seed(topic_seed),
   survivors.len(), min(pick_count, survivors.len()))`, indices mapped
   through the ordered survivor list. Pick count 0 → empty draw.

**Commit A** reproduces today's `UniformSampler::topic_seed` byte-exactly:
`topic_seed = SHA-256("experiments/uniform-sampler/v1" ‖ seed ‖
topic-bytes)` — plain concatenated `update` calls, no length prefixes, no
epoch nonce, no self-id. At (bucket count absent, pick count = K) over the
same view this reproduces `UniformSampler`'s picks value-for-value, so the
recorded M2 baselines byte-diff identical.

**Commit B** replaces it with the honest derivation:
`topic_seed = SHA-256(push_len_prefixed(domain) ‖ push_len_prefixed(seed) ‖
push_len_prefixed(self-id key bytes) ‖ nonce_le8 ‖
push_len_prefixed(topic-bytes))` under **per-seam domains** selected by the
instance's `LinkKind`, exactly as the gate selects its edge domain:
`pubsub/uniform-selection/relay/v1` and
`pubsub/uniform-selection/publisher/v1` — the crate's one length-prefix
primitive, mirroring the edge predicate's preimage conventions
(variable-width components prefixed, the nonce fixed-width). Self-id lives
in the strategy-level preimage (not only the CLI edge) so the
fleet-shared-seed independence property (spec FR-015) holds for every
construction site, driver included. The per-seam split is what keeps an
M3/M5 node's two `Selection` instances — same seed, same self-id, same
nonce, same topics — from deriving the same RNG stream: with a single
shared domain and both seams ungated, equal pick counts would make the
publisher targets *identical* to the relay upstreams, which is neither the
models' assumption nor what the gate's domain separation already
guarantees for gated selection. No symmetric draw domain exists: the
symmetric switch changes the handshake vocabulary, not the draw.

**Rationale**: the byte-identity gate demands an exact reproduction first;
the honest preimage then lands as one deliberate, re-baselined change
(plan-input two-commit mapping). Sorted-survivor index sampling keeps the
draw a pure function of the set, order-independent by construction.

**Alternatives considered**: mixing self-id only at the CLI edge (rejected:
leaves the experiments path able to correlate picks if a config ever shares
participant seeds — the property belongs to the draw itself); keeping the
old domain string permanently (rejected: `experiments/…` becomes a misnomer
the moment the sampler is a node capability; the re-baseline is already
budgeted); a single shared draw domain (rejected: correlates the two seam
instances' draws on M3/M5 nodes — the latent defect surfaced during the
Phase 2 implementation review); one domain plus a kind-tag preimage
component (equivalent cryptographically; rejected in favour of the domain
split because separate domain constants are the established edge.rs
pattern for exactly this independence property).

## R3 — Dependencies: no manifest change

**Decision**: no `Cargo.toml` change and no dependency ADR. Verified:
`rand = "0.8"` and `rand_chacha = "0.3"` are **already unconditional**
dependencies (the `experiments` feature gates only `serde_json`, ADR 0037);
the sampler code was cfg-gated at the module level, not the manifest level.
`sha2` (seed derivation) is likewise unconditional.

**Rationale**: the plan-input flagged a possible promotion; the manifest
shows there is nothing to promote. Moving sampling into the always-built
`strategies/` tree changes which build targets compile it, not the
dependency set.

**Alternatives considered**: n/a — fact check, not a choice.

## R4 — CLI surface and validation split

**Decision**: `Args` is rebuilt around the knob surface: per-seam
`--relay-bucket-count` / `--relay-pick-count` / `--relay-accept-cap` /
`--relay-symmetric` / `--relay-accept-unverified` plus the three publisher
mirrors and `--publisher-accept-unverified`, `--selection-seed <u64>`,
`--fanout-strategy` (default `forward-to-all`), `--genesis` unchanged.
Deleted flags: `--relay-strategy`, `--relay-acceptance-strategy`,
`--relay-degree`, `--publisher-strategy`,
`--publisher-acceptance-strategy`, `--publisher-degree`, `--bucket-count`,
`--cap-buffer`, `--symmetric-edges`.

Validation lives in two layers, deliberately different:

- **Core (construction) domains**, shared by every entry point: bucket
  count ≥ 1 (0 rejected — division by zero in the predicate; 1 legal ≡
  ungated, the sweep config's axis point), pick count and accept cap any
  value (all of `usize` meaningful).
- **CLI-edge rules** (`validate_flag_combinations` rewrite): bucket counts
  must be ≥ 2 (1 rejected — gating is signalled by the flag's presence);
  `--selection-seed` required iff any seam has pick count ≥ 1, rejected as
  unused otherwise; publisher seam activated solely by acceptance-side
  knobs rejected (names the `--publisher-pick-count 0` accept-only
  spelling); `--*-accept-unverified` without that seam's bucket count
  rejected as unused. The old `--symmetric-edges`-requires-hash-gated rule
  is **deleted**: symmetric composes with every plane point (uniform + 
  symmetric is exactly the real M4).

**Rationale**: spec FR-006–FR-009 and the clarification session; the
two-layer split is what lets the sweep config accept boundary values the
operator CLI refuses (spec FR-018) without duplicating semantics — the core
type means the same thing everywhere, the edges enforce their own consumers'
intent.

**Alternatives considered**: rejecting B = 1 in the core (rejected: E10's
bucket-count axis needs 1 as a numeric point); accepting B = 1 at the CLI
(rejected in the clarified spec: either it is not hash gating — no flag —
or B ≥ 2).

## R5 — Construction: the two-phase builder loses phase 1

**Decision**: with no kinds to resolve, `NodeStrategies::builder(kind, kind)`
disappears. `NodeStrategies` gains one constructor taking the new per-seam
param structs — `SelectionParams { self_id, kind, symmetric, bucket_count,
pick_count, seed }` and `AcceptanceParams { self_id, kind, symmetric,
bucket_count, accept_cap }` (the opt-out already folded into `bucket_count:
None` by the edge) — building the relay pair always and the publisher pair
from `Option<(SelectionParams, AcceptanceParams)>`, behind one
`Result<_, StrategyConfigError>`. This absorbs §1.2 item 6 (the publisher
pair no longer bypasses the builder; the error-map site count drops to one).
`require_target_degree` and `validate_bucket_count` are replaced by the R4
core-domain checks; `NodeStrategies::relay_only` survives unchanged as the
test/driver convenience.

**Rationale**: ADR 0028's point was "construction and required-parameter
validation live with the strategy" — that point survives; only the
now-empty key-resolution phase goes. Spec FR-005/FR-010; plan-input module
layout.

**Alternatives considered**: keeping a vestigial phase-1 builder with no
inputs (rejected: ceremony without content).

## R6 — Experiments config: coordinates in, kind strings out

**Decision**: `StrategyTable` drops `connection`/`acceptance` kind strings
and `target_degree`/`cap_buffer`; it gains `pick_count: Option<usize>`,
`bucket_count: Option<usize>` (≥ 1 legal — 1 is the ungated axis point),
`accept_cap: Option<usize>`, `accept_unverified: bool` (default false),
`symmetric: bool` (default false). `fanout` keeps its current vocabulary
(`forward-to-relays`, `silent-relay`; `forward-to-all` stays rejected —
extensionally identical in relay-only populations, and its acceptance rides
the publisher-pair follow-up). The axis vocabulary renames `target_degree`
→ `pick_count` and adds `bucket_count`; axis values map into the same
fields. Population construction feeds `symmetric` into the relay params and
`NodeStrategies.symmetric_edges` (today hardcoded `false` at
population.rs:291/345) and keeps `relay_only` shape otherwise. The
per-participant sampler seeds (master-seed derivation chain) are unchanged
and are now threaded into `Selection` whenever `pick_count` is set.

**Rationale**: spec FR-017/FR-018/FR-019; the config gains the symmetric
switch because the validation contract requires a recorded M4 baseline and
baselines are experiment artifacts.

**Alternatives considered**: keeping `uniform-sampler` as a config alias for
(bucket absent, pick present) (rejected: two spellings for one point is the
kind-vocabulary problem again); accepting `forward-to-all` now (rejected:
no publisher links can exist in a 017 population, so it measures nothing
new and its real consumer is the next feature).

## R7 — M4 baseline needs no analytics variant

**Decision**: the M4-completing recipe runs under the existing
`DisseminationModel::M2` extraction. On a symmetric population every relay
link is mirrored, so the extracted propagation digraph (relay `downstream`
edges between up-honest peers) is symmetric by construction; the one-SCC
goodness criterion and the publish-drain metrics apply unchanged. No new
dispatch variant lands in 017 (M3/M5 variants are the publisher-pair
feature's work).

**Rationale**: the M4 baseline requirement (spec FR-027) must be satisfiable
inside 017's scope; verifying the extraction is model-correct for symmetric
populations is a test, not new machinery.

**Alternatives considered**: adding an `M4` dispatch name now (rejected:
it would alias M2's extraction exactly — a name without a semantic
difference; the dispatch grows when extraction rules actually differ).

## R8 — CLI seed expansion

**Decision**: the loader expands `--selection-seed <u64>` to the
constructor's 32 bytes as `SHA-256(push_len_prefixed("pubsub/selection-seed/v1")
‖ seed_le8)`. Self-id and epoch nonce are **not** mixed here — they enter in
the strategy's per-topic preimage (R2), so the edge derivation stays a pure
format expansion and the independence/re-randomisation properties live in
one place.

**Rationale**: parse at the edge (the constructor keeps taking 32 bytes;
the driver's injection path is untouched); one derivation site per property.

**Alternatives considered**: taking 32-byte hex on the CLI (rejected:
hostile ergonomics next to `--genesis`, and the u64 matches the prototype's
reproducibility-not-secrecy posture — recorded in the privacy
implementation note).

## R9 — Fan-out default flip and the absorbed §1.2 items

**Decision**: the clap default for `--fanout-strategy` becomes
`forward-to-all`; `FanoutStrategyKind` docs swap which variant is "the
default" and name `forward-to-relays` as the M3-exclusivity switch. In the
same files: `ForwardToRelays::targets` gains the uniform
`LinkState::Active` check on relay entries (§1.2 item 1 — today
`LinkKind::Relay => true` admits any state; safe only by the
insert-Active invariant), and the `fanout/mod.rs` module doc's wrong
submodule link + pre-015 text are corrected (§1.2 item 9). The M5-footgun
warning (publisher links + omitted fan-out flag ⇒ M5 semantics) lands in
the CLI help and quickstart.

**Rationale**: spec FR-009/FR-023; the library's `NodeStrategies` takes
fan-out as an explicit injected instance, so "default" is a CLI-edge fact —
no library behaviour changes outside the Active check.

**Alternatives considered**: keeping `forward-to-relays` as default
(overridden by the maintainer decision: M1/M2/M4/M5 become flag-free and
M3 — the model *defined* by its exclusivity rule — becomes the marked
case).

## R10 — Deletion inventory and edge-module residue

**Decision**: deleted with the rework — `strategies/connection/`
{`connect_to_all.rs`, `hash_gated.rs`, `none.rs`, `kind.rs`},
`strategies/acceptance/` {`accept_from_all.rs`, `bounded.rs`,
`hash_gated.rs`, `hash_gated_bounded.rs`, `none.rs`, `kind.rs`}, the
`UniformSampler` half of `experiments/strategies.rs` (`SilentRelay` stays),
and in `strategies/edge.rs`: `resolve_buckets`, `bucket_count`,
`accept_cap` (formulas move to quickstart guidance). `is_valid_edge_publisher`
joins its siblings in the public export set (§1.2 item 8's residue).
`NodeView` is unchanged; `candidates_len` loses its bucket-derivation
consumer and keeps its doc updated (still used by tests/diagnostics).
`Admission`, `admit_prelude`, `link_scan`, `is_membership_valid` are reused
verbatim by `UnifiedAcceptance`. The receive path, handlers, and `NodeState`
are untouched — an accept cap of 0 flows through the existing
`RejectOverCapacity` → explicit `Rejected` path, which is precisely spec
FR-013's deliberate change from the deleted `AcceptNone`'s silent drop
(the *unconfigured* publisher seam still silently drops — that lives at the
seam-off level in the handlers, not in acceptance policy, and is unchanged).

**Rationale**: spec FR-005/FR-012/SC-008; keeping the shared acceptance
mechanics makes the merge an implementation collapse, not a semantics
change.

**Alternatives considered**: relocating `admit_prelude` into the unified
type (rejected: it is the documented shared-invariant site; one caller
today, but the seam contract says policy composes it).

## R11 — ADRs and implementation notes

**Decision**: two ADRs — **0039 — the unified selection plane** (one
implementation per seam over fed bucket count + pick count; kind enums and
per-kind types deleted; acceptance merged with verification-follows-B + the
opt-out; caps fed absolutely; knob-only presence-activated CLI with zero
boundary values; fan-out default flip; the verifiable region ⟺ bucket count
present; supersedes the relevant parts of ADRs 0018/0023/0024/0025/0028/
0031 wording and upgrades ADR 0032/0034's deferred M4 label) and **0040 —
selection randomness derivation** (the seed chain: u64 flag → 32 bytes →
per-topic preimage with self-id + epoch nonce under
`pubsub/uniform-selection/v1`; the privacy stand-in posture; the two-commit
derivation swap). Three implementation notes at the next free numbers
(N-036: gate-failing dials as provable-but-unrecorded evidence, trigger the
incentive/chain layer; N-037: selection-seed privacy, trigger the first
adaptive-adversary experiment or real-crypto identity work; N-038: sampled
selection under view growth — added by the I3 ruling during
implementation, trigger periodic heartbeats/epoch rotation or the first
staggered-boot fleet or experiment). N-032's
trigger text is re-pointed (first experiment needing symmetric × capped —
may never arrive). The optional configuration-placement ADR (flags vs TOML)
is carried as a docs-commit candidate, not a commitment.

**Rationale**: Principle III (structural decisions), spec FR-022; the seed
derivation is separately reversible from the plane (different blast
radius), so two ADRs rather than one.

**Alternatives considered**: one omnibus ADR (rejected: the seed
derivation's alternatives and revisit trigger are independent of the plane's).

## R12 — Test strategy and TDD-critical designation

**Decision**: critical — TDD required — for: the `Selection` draw semantics
(exactly-min(K, survivors), gate-then-pick composition, order-independence,
boundary values, heartbeat stability, epoch re-randomisation, fleet-shared
seed independence, the commit-A `UniformSampler` equivalence pin);
`UnifiedAcceptance` decisions (the 2×2 admission matrix, cap 0 → explicit
`Rejected`, opt-out, symmetric predicate); and the experiments-side
byte-identity (commit A baseline byte-diff) + determinism battery (existing
tests, must stay green unmodified). Tests-with (non-critical): CLI
validation matrix, config parsing, docs examples. Existing test files are
reworked in place: strategy unit suites collapse alongside their types;
`tests/model_family.rs` recipes move to knob construction (absorbing §1.2
items 12–13 — stale comments and the `no_links()` fixture adoption);
`tests/publisher_links.rs` keeps its scenarios under the new construction.
The M4 topology properties (reciprocity, min degree ≥ K, mean ≈ 2K) join
`model_family.rs` as the label-upgrade evidence (spec SC-003). Per the
no-log-assertions standard, every startup-validation test asserts on exit
behaviour/`Result` values, not stderr text.

**Rationale**: Principle II — this feature carries protocol-behaviour
claims (the M2 selection family exactly; the M4 floor); the byte-identity
pin is the strongest available regression harness and is mandated by the
spec's validation contract.

**Alternatives considered**: new parallel test files (rejected: the
constitution's declarative-test-construction and the existing suites'
coverage make in-place rework cheaper and keep the parity story auditable).
