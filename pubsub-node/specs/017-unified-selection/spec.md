# Feature Specification: Unified selection plane

**Feature Branch**: `017-unified-selection`

**Created**: 2026-07-31

**Status**: Draft

**Input**: User description (verbatim):

> Unified selection plane (017). One selection implementation over two knobs
> replaces the dial-side strategy kinds and the four acceptance baselines, per
> seam (relay and publisher instances of the same type, per-seam hash domains
> as today). Vocabulary: always "the bucket count" and "the pick count" in
> words (never a bare K — it collides with the models' adversary count);
> config spelling bucket_count / pick_count; the target_degree /
> --relay-degree / --publisher-degree spellings are replaced everywhere,
> pre-release, no deprecation aliases.
>
> Semantics per topic: (1) gate — keep candidates passing H(nonce, topic,
> self, candidate) mod B == 0 under the seam's domain; bucket count absent ≡
> B = 1 (the predicate's existing short-circuit — everyone survives); (2)
> pick — draw exactly min(pick count, survivors) seeded uniform picks without
> replacement; pick count absent = dial every survivor; pick count 0 = dial
> none. B is fed, never derived: delete the derive arm of resolve_buckets and
> the bucket_count(len, target_degree) formula; auto-scaling from membership
> stays rejected (grinding surface); the small-topic connect-to-all floor
> becomes the parameter-setter's responsibility, with balanced-point guidance
> (B ≈ candidates/K) documented next to the recipes. The four dial strategies
> become plane points — connect-to-all (both absent), uniform (pick count
> only), hash-gated (bucket count only), gated+capped (both) — and their
> types are deleted, including the experiments-only UniformSampler, which
> this feature promotes to a node capability. The (bucket count absent, pick
> count = RF) point plus the constructed-reciprocity symmetric handshake
> (ADR 0034) realises the formal M4 exactly: upgrade the "M4 approximation"
> label in the 015 quickstart, contracts, and ADR 0032's modelling caveat.
>
> CLI surface: knob-only, per-seam, presence-activated; the dial and
> acceptance kind flags are deleted. --relay-bucket-count (≥ 2; the CLI
> rejects 1 — gating is signalled by providing the flag and a one-bucket gate
> is vacuous), --relay-pick-count (≥ 0), --relay-accept-cap (≥ 0),
> --relay-symmetric (renames --symmetric-edges), plus --publisher-* mirrors
> of the first three; any publisher knob activates the publisher seam. Zero
> boundary values replace the none kinds: M1 is --relay-pick-count 0; accept
> cap 0 = serve none, refusing with explicit Rejected — a deliberate
> behavioural change from the old disabled-seam silent drop, stated and
> tested. Fan-out default flips to forward-to-all; forward-to-relays becomes
> M3's explicit flag; the quickstart states the footgun loudly (publisher
> links configured + fan-out flag omitted now yields M5 semantics, not M3
> exclusivity). Unconsumed flags are rejected loudly.
>
> Acceptance: the four baselines (ADR 0031) merge into one implementation
> with two independent dimensions; the four points stay expressible as knob
> combinations. Acceptor gate verification follows the seam's bucket count
> (present ⇒ verify the same B the dialers use — the verifiability agreement
> condition; absent ⇒ vacuous), with one explicit opt-out flag (name to
> settle in this round) preserving the trusting-acceptors comparison arm. The
> accept cap is fed absolutely per seam; accept_cap(K, c) and --cap-buffer
> are deleted; the ⌈K + c·√K⌉ (c ≈ 3) headroom formula moves to documentation
> as parameter-choosing guidance.
>
> Seed: --selection-seed <u64>, required iff any seam has pick count ≥ 1,
> rejected as unused otherwise. Per-topic draw preimage H(domain ‖ seed ‖
> self-id key bytes ‖ epoch nonce ‖ topic): self-id mixed in so a
> fleet-shared seed value still yields per-node-independent draws; the epoch
> nonce mixed in so an Epoch event re-randomises picks exactly as it
> re-shuffles gated edges, and heartbeat re-dial stability holds within an
> epoch. The strategy constructor keeps taking 32 seed bytes — the
> experiments driver's per-participant injection is unchanged. Reading:
> --genesis is the shared public randomness (the gate's context),
> --selection-seed the per-node notionally-private randomness (the sampler's
> context). Implementation note: the models prescribe private, unpredictable
> selection randomness; the operator flag is a prototype stand-in,
> model-adequate against oblivious adversaries only; trigger = first
> adaptive-adversary experiment or the real-crypto identity work.
>
> Verifiable region, documented: verifiability ⟺ bucket count present —
> every dialed edge acceptor-checkable regardless of pick count (which-K
> freedom among valid edges is not a violation); bucket count absent = fully
> private selection, experiments-only on the protocol track. A gate-failing
> dial is provable misbehaviour (signed request + publicly recomputable
> predicate); v1 keeps the silent drop, and an implementation note names the
> acceptance gate as the future evidence-collection point (trigger: the
> incentive/chain layer).
>
> Experiments framework: pick_count replaces target_degree in the strategy
> table and axis vocabulary; bucket_count becomes an axis parameter (E10);
> the kind vocabularies give way to the same coordinates; the config gains
> the symmetric switch (required by this feature's own validation contract —
> the M4-completing recipe needs a recorded baseline, and baselines are
> experiment artifacts). Boundary values are legal axis points in the sweep
> config even where the CLI rejects them: bucket_count = 1 is the ungated
> point on a bucket-count axis, pick_count = 0 the k_in/k_out = 0 boundary
> (E8's reductions become plain axis values). Population construction stays
> relay-only; publisher-pair experiment configuration is the next feature.
>
> Validation contract: cross-version byte identity NOT required — outputs may
> change. The M2 point keeps the formal selection family's semantics exactly
> (RF uniform picks without replacement per topic); after landing, re-execute
> the m2-comparison, confirm statistical agreement, update the doc, and
> record fresh baseline generations per notes/experiments-baselines/README.md,
> plus a new baseline for the M4-completing recipe; the within-version
> determinism battery (value-level determinism, replay-by-seed, worker-count
> invariance of the three artifacts) is mandatory and unchanged.
> Implementation follows the two-commit shape: commit A reproduces today's
> sampler derivation exactly (old domain string, no nonce in the preimage)
> and must byte-diff identical against the recorded baselines; commit B
> renames the domain, adds the epoch nonce and self-id to the preimage, lands
> the CLI seed derivation, and re-baselines with a statistical m2-comparison
> re-run.
>
> Dispositions carried from the pre-spec round: N-032 semantics unchanged and
> acceptable (cap scan counts mirrored own-dials, gate fires on
> peer-initiated only, realised degree can exceed the cap order-dependently;
> symmetric × capped stays expressible, quickstart notes the ~2K cap
> anchoring), trigger re-pointed to the first experiment needing that
> combination, which may never arrive. Registry-computed balanced B rejected
> as mechanism (still membership-coupled — the grinding surface with the
> lever moved, not removed); the formula survives as operator guidance; the
> registry as carrier of a governance-set per-topic B stays open as a
> separate future feature. E12's "[needs: Level-1 flooding dial kind]" tag is
> superseded — the rational level-1 flooder stays inside its valid edge set
> (an invalid dial is self-incriminating evidence) and saturating it is the
> (bucket count pinned, no pick count) plane point as the adversarial
> bundle's dial coordinates with silent-relay fan-out; E12 flips to ready;
> one-line program-doc status correction rides the docs commit. §1.2 split:
> item 1 rides (uniform Active check in ForwardToRelays); items 2–6, 8–10,
> 12–13 dissolve in the rework; items 7 and 11 stay on the pickup list.
> Housekeeping riders on the first docs commit: refresh the
> pubsub-node/CLAUDE.md active-work stanza (still describes merged PR #118);
> the E12 status line; candidate optional short ADR recording the
> configuration-placement rationale (CLI flags = one node's own knobs; TOML =
> shared world state or declarative sweep definitions).
>
> Out of scope: publisher-pair experiment configuration (the immediate
> follow-up — E6/E8, per-model DisseminationModel variants); level-1 flooding
> beyond the plane point (behind E15's relevance classification); N-035
> (sampler O(N²) time term, trigger unchanged); per-direction seam asymmetry
> flags (dissolved — zero boundary values already express accept-only and
> dial-only shapes); strategy-machinery relocation (residency tension noted,
> deliberately not acted on).
>
> Full pre-spec design record: notes/017-unified-selection-pre-spec.md.

## Clarifications

### Session 2026-07-31

- Q: What does startup do when the publisher seam is activated solely by
  acceptance-side knobs (e.g. `--publisher-accept-cap` with no publisher dial
  knob)? → A: Reject at startup — activating the publisher seam requires at
  least one dial knob (`--publisher-pick-count`, 0 permitted, or
  `--publisher-bucket-count`); the error names the accept-only spelling
  (pick count 0).
- Q: The final spelling of the per-seam verification opt-out flag? → A:
  `--relay-accept-unverified` / `--publisher-accept-unverified` (the
  Assumptions default, confirmed).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Configure selection as plane coordinates (Priority: P1)

An experimenter or node operator configures a node's link selection with two
per-seam knobs — the bucket count (the hash-gate width) and the pick count
(the number of uniform picks among gate survivors) — instead of choosing
among named strategy kinds. Every selection behaviour the node previously
offered, plus two it could not express (exactly-K uniform picks; gated picks
with a real cap), is a coordinate pair; seam on/off shapes are boundary
values (pick count 0 = dial none, accept cap 0 = serve none); the publisher
seam activates by presence of any of its knobs.

**Why this priority**: This is the feature — one implementation whose points
replace four dial strategies and their kind vocabulary, fixing the standing
defect that the configured degree had no dial-side effect under a pinned
bucket count. Every other story configures behaviour through this surface.

**Independent Test**: Build nodes at each of the four plane points (both
knobs absent; pick count only; bucket count only; both) plus the boundary
values, and assert the selected upstream sets: full mesh; exactly
min(pick count, candidates) uniform picks; every predicate survivor;
min(pick count, survivors); empty.

**Acceptance Scenarios**:

1. **Given** a topic with more candidates than the pick count and no bucket
   count, **When** the node dials, **Then** it selects exactly pick-count
   candidates, uniformly without replacement, and repeated heartbeats within
   the epoch re-dial the identical set.
2. **Given** a bucket count B ≥ 2 and no pick count, **When** the node dials,
   **Then** it selects exactly the candidates passing the edge predicate at
   B (the previous hash-gated behaviour).
3. **Given** both knobs, **When** the node dials, **Then** it selects
   min(pick count, survivors) uniform picks from within the predicate
   survivors, and every dialed edge passes the predicate.
4. **Given** neither knob, **When** the node dials, **Then** it selects every
   candidate (the previous connect-to-all default, preserved as the default
   behaviour).
5. **Given** `--relay-pick-count 0`, **When** the node runs, **Then** it
   dials no relay links while its acceptance side still serves inbound
   requests (the push-only M1 shape).
6. **Given** any `--publisher-*` knob, **When** the node starts, **Then** the
   publisher seam is active with the configured coordinates; **Given** no
   publisher knobs, **Then** the node dials no publisher links and drops
   inbound publisher requests (the M2 baseline, off by construction).
7. **Given** a flag whose value nothing consumes (for example a selection
   seed with no sampling seam), **When** the node starts, **Then** startup
   fails with an actionable message.

---

### User Story 2 - The real M4: uniform picks over the symmetric handshake (Priority: P2)

A model experimenter configures the formal M4 model exactly: uniform
exactly-RF picks (pick count, no bucket count) established through the
constructed-reciprocity symmetric handshake. The "M4 approximation" label is
retired: minimum degree ≥ pick count holds by construction, and the
documentation that previously disclaimed the label now claims it.

**Why this priority**: Completing M4 is the feature's headline consequence —
it is what the deferred label in the 015 quickstart, contracts, and the
ADR 0032 modelling caveat has been waiting on, and it unblocks the E7
experiment (the M4 law: minimum-degree floor, connectivity at small RF).

**Independent Test**: Build a fleet with `--relay-pick-count RF
--relay-symmetric`, drain handshakes, and assert per node: every link present
in both directions on both ends (reciprocity), degree ≥ RF (own picks all
land — the acceptor has no cap), mean degree ≈ 2·RF.

**Acceptance Scenarios**:

1. **Given** a symmetric fleet with pick count RF and no acceptance cap,
   **When** topology settles, **Then** every node's degree is ≥ RF and every
   link is recorded in both collections on both endpoints.
2. **Given** the same configuration, **When** the topology is measured across
   seeds, **Then** mean degree ≈ 2·RF (own picks plus inbound picks).
3. **Given** the symmetric flag with a bucket count, **When** the node dials,
   **Then** the unordered-pair symmetric predicate gates candidates before
   the uniform draw (the protocol-track symmetric point remains expressible
   as coordinates).

---

### User Story 3 - Acceptance as two independent dimensions (Priority: P2)

The four acceptance baselines merge into one implementation with two
independent dimensions. Gate verification follows the seam's bucket count —
acceptors verify exactly the B the dialers use, which is the agreement
condition verifiability rests on — with one explicit opt-out preserving the
trusting-acceptors comparison arm. The serving cap is fed as an absolute
per-seam value, never computed from other parameters.

**Why this priority**: Without it the node keeps a kind enum on one seam
after deleting it on the other, and the cap formula silently anchors on a
number that no longer governs in-degree once the dial side is a real cap.

**Independent Test**: Configure acceptance at each of the four points via
knob combinations (no gating/no cap; gating via seam bucket count; cap via
accept-cap; both) and assert admission decisions against inbound requests
that are members/non-members, predicate-passing/failing, and under/over
capacity.

**Acceptance Scenarios**:

1. **Given** a seam bucket count B ≥ 2, **When** an inbound request arrives
   whose edge fails the predicate at B, **Then** it is dropped; **Given** the
   verification opt-out flag, **Then** the same request is admitted
   (membership permitting).
2. **Given** `--relay-accept-cap N`, **When** the node already serves N
   downstream links on the topic, **Then** the next request is refused with
   an explicit rejection and the dialer removes its pending entry.
3. **Given** `--relay-accept-cap 0`, **When** any request arrives, **Then**
   it is refused with the explicit rejection (deliberate change from the old
   disabled-seam silent drop: the dialer's pending entry is cleaned up).
4. **Given** no accept-cap flag, **When** requests arrive, **Then** admission
   is unbounded (membership and gate permitting).

---

### User Story 4 - Seeded selection on a real node (Priority: P3)

An operator running configurations that sample (any seam with pick count
≥ 1) supplies `--selection-seed <u64>`. Draws are stable across heartbeats
within an epoch, re-randomise when the epoch nonce changes, differ per node
even when a fleet shares one seed value, and reproduce exactly under the same
(seed, self-id, nonce, membership).

**Why this priority**: Sampling needs a randomness source the node never had;
the flag makes the unresolved privacy question impossible to miss while
keeping topologies reproducible. Depends on User Story 1 mechanics.

**Independent Test**: Run the same node twice with the same seed and inputs
(identical picks), twice with different seeds (different picks with high
probability), two nodes with the same seed (per-node-independent picks), and
one node across an epoch-nonce change (picks re-drawn).

**Acceptance Scenarios**:

1. **Given** a sampling configuration without `--selection-seed`, **When**
   the node starts, **Then** startup fails naming the missing flag; **Given**
   the flag with no sampling seam, **Then** startup fails naming the unused
   flag.
2. **Given** two nodes sharing one seed value on the same topic, **When**
   both dial, **Then** their pick sets are drawn independently (self-identity
   distinguishes the draws).
3. **Given** a settled epoch, **When** heartbeats repeat, **Then** the
   expected set is unchanged; **When** the epoch nonce changes, **Then** the
   draw changes.

---

### User Story 5 - Plane sweeps and re-validated baselines in the experiments framework (Priority: P3)

An experimenter sweeps the plane: the sweep configuration speaks the same
coordinates as the CLI (pick count, bucket count, accept cap, symmetric),
bucket count and pick count are axis parameters, and boundary values are
legal axis points (bucket count 1 = the ungated point; pick count 0 = the
k_in/k_out = 0 boundary). The M2 comparison is re-executed and fresh
baselines recorded, including the first M4 baseline.

**Why this priority**: The experiments framework is the feature's measuring
instrument and its primary consumer (E7, E10; E12 flips to ready). Depends on
Stories 1–4.

**Independent Test**: Run a sweep whose axes cross bucket count (including 1)
and pick count (including 0); verify the boundary cells reproduce the
ungated/off behaviours; re-run the m2-comparison operating point and the
determinism battery.

**Acceptance Scenarios**:

1. **Given** a sweep config with a bucket-count axis including 1, **When** it
   runs, **Then** the bucket-count-1 cell behaves identically to the ungated
   configuration (the CLI-rejected spelling is a legal axis point here).
2. **Given** the M2 operating-point config expressed in coordinates, **When**
   commit A is validated, **Then** run-records and aggregates byte-diff
   identical against the recorded baselines; **When** the final feature is
   validated, **Then** the re-executed m2-comparison agrees statistically
   with the formal values and fresh baseline generations are recorded.
3. **Given** the M4-completing recipe expressed in the sweep config
   (symmetric switch + pick count), **When** it runs, **Then** its baseline
   is recorded and the M4 topology properties hold fleet-wide.
4. **Given** any config, **When** run at different worker counts or replayed
   by seed, **Then** the three artifacts are identical (the determinism
   battery is unchanged).

---

### Edge Cases

- `--relay-bucket-count 1` (or 0): rejected at startup — gating is signalled
  by the flag's presence and a one-bucket gate is vacuous; the sweep config,
  by contrast, accepts 1 as the ungated axis point.
- Pick count exceeding the survivor count: all survivors are selected (the
  degenerate direction matches the previous small-topic behaviour); no retry
  or back-fill.
- Bucket count larger than a topic's candidate count: possibly zero
  survivors, hence zero upstreams on that topic (no retry) — the
  parameter-setter's responsibility, documented with the balanced-point
  guidance.
- Publisher links configured with the fan-out flag omitted: the node runs M5
  semantics (default forward-to-all), not M3 exclusivity — stated loudly in
  the quickstart as the deliberate consequence of the default flip.
- `--relay-symmetric` with an accept cap: expressible; the recorded N-032
  behaviour applies (the cap's scan counts mirrored own-dials, the gate fires
  only on peer-initiated requests, realised degree can exceed the cap
  arrival-order-dependently); the quickstart notes that a symmetric node's
  healthy degree is ≈ 2× the pick count, so caps there anchor on ≈ 2K.
- Verification opt-out on a seam with no bucket count: the flag consumes
  nothing (the gate is already vacuous) — rejected as unused.
- Publisher seam activated by acceptance knobs alone (e.g.
  `--publisher-accept-cap` with no publisher dial knob): rejected at startup
  — the dial side's intent is ambiguous between accept-only and the
  full-mesh default; the error names the accept-only spelling
  (`--publisher-pick-count 0`).
- A fleet sharing one selection-seed value: per-node draws remain independent
  by construction (self-identity in the derivation).
- Epoch nonce change without teardown: picks and gated edges re-randomise
  together; v1 still never fires the epoch event (add-only dialing caveats
  unchanged).

## Requirements *(mandatory)*

### Functional Requirements

**Selection semantics**

- **FR-001**: The system MUST provide one selection implementation per seam
  instance that, per topic, first gates candidates by the seam's edge
  predicate at the configured bucket count (absent ≡ 1: every candidate
  survives, via the predicate's existing short-circuit) and then draws
  exactly min(pick count, survivors) uniform picks without replacement.
- **FR-002**: Pick count absent MUST select every gate survivor; pick count 0
  MUST select nothing. Bucket count, pick count, and accept cap MUST be
  independently optional.
- **FR-003**: The bucket count MUST be fed configuration, never derived: the
  derive arm of `resolve_buckets` and the `bucket_count(len, target_degree)`
  formula are removed; no component may compute a bucket count from
  membership or view state.
- **FR-004**: The relay and publisher seams MUST use separate instances of
  the one implementation with their existing per-seam hash domains; the
  symmetric mode MUST compose with the plane on the relay seam (unordered-
  pair predicate for the gate; reciprocity remains the handshake's,
  ADR 0034, regardless of coordinates).
- **FR-005**: The four dial strategy types (connect-to-all, hash-gated,
  dial-none, and the experiments-only uniform sampler) and the dial-side
  kind enum MUST be deleted; the strategy trait and injection seams are
  unchanged.

**Node configuration surface**

- **FR-006**: The CLI MUST expose per-seam knobs `--relay-bucket-count`,
  `--relay-pick-count`, `--relay-accept-cap`, `--relay-symmetric` (renaming
  `--symmetric-edges`), and `--publisher-bucket-count`,
  `--publisher-pick-count`, `--publisher-accept-cap`; the dial and acceptance
  kind flags, the per-seam degree flags, and the shared `--bucket-count`
  (superseded by the per-seam bucket counts) are deleted, with no deprecation
  aliases.
- **FR-007**: Validation domains: bucket count ≥ 2 (the CLI rejects 0 and 1),
  pick count ≥ 0, accept cap ≥ 0. Any provided flag whose value nothing
  consumes MUST fail startup with an actionable message.
- **FR-008**: The publisher seam MUST activate on the presence of any
  `--publisher-*` knob and remain off by construction otherwise (no dial
  pass; inbound publisher requests dropped). A publisher seam activated
  solely by acceptance-side knobs MUST fail startup: activation requires at
  least one dial knob (`--publisher-pick-count`, 0 permitted, or
  `--publisher-bucket-count`), and the error names the accept-only spelling
  (pick count 0).
- **FR-009**: The fan-out default MUST become forward-to-all;
  forward-to-relays remains available as the explicit M3-exclusivity flag.

**Acceptance**

- **FR-010**: The four acceptance baselines MUST merge into one
  implementation with two independent dimensions: gate verification and the
  serving cap. The four acceptance strategy types and the acceptance-side
  kind enum are deleted; the four previous behaviours remain expressible as
  knob combinations.
- **FR-011**: Acceptor gate verification MUST follow the seam's bucket count
  (present ⇒ verify the same value the dialer uses; absent ⇒ vacuous), with
  one explicit per-seam opt-out flag (`--relay-accept-unverified` /
  `--publisher-accept-unverified`) that admits without predicate
  verification; the opt-out without a seam bucket count is rejected as
  unused.
- **FR-012**: The accept cap MUST be a fed absolute per-seam value.
  `accept_cap(target_degree, c)` and `--cap-buffer` are deleted; the
  headroom formula ⌈K + c·√K⌉ (c ≈ 3) moves to documentation as
  parameter-choosing guidance.
- **FR-013**: An accept cap of 0 MUST refuse every request with the explicit
  over-capacity rejection (the dialer cleans up its pending entry) — a
  deliberate, tested behavioural change from the previous disabled-seam
  silent drop.

**Selection seed**

- **FR-014**: The CLI MUST require `--selection-seed <u64>` iff any seam has
  pick count ≥ 1, and reject it as unused otherwise.
- **FR-015**: The per-topic draw MUST be a pure function of (the seam's
  domain, seed, self-identity key bytes, epoch nonce, topic): stable across
  heartbeats within an epoch, re-drawn when the epoch nonce changes,
  per-node independent under a fleet-shared seed value, and per-seam
  independent on one node — the relay and publisher instances draw under
  separate domains, so an M3/M5 node's publisher targets are uncorrelated
  with its relay upstreams (the same independence the edge predicate's
  per-seam hash domains already provide for gated selection).
- **FR-016**: The selection implementation's constructor MUST keep taking 32
  seed bytes so the experiments driver's per-participant seed injection is
  unchanged.

**Experiments framework**

- **FR-017**: The sweep-config strategy table MUST speak the same
  coordinates as the CLI (`pick_count` replacing `target_degree`;
  `bucket_count`; the accept cap; the verification opt-out; the symmetric
  switch), and the kind-name vocabularies (`connection`/`acceptance` kind
  strings, `uniform-sampler`) MUST be replaced accordingly.
- **FR-018**: `bucket_count` and `pick_count` MUST be sweepable axis
  parameters, and boundary values MUST be legal axis points in the sweep
  config even where the CLI rejects them: bucket count 1 (the ungated point)
  and pick count 0 (the k_in/k_out = 0 boundary).
- **FR-019**: Population construction remains relay-only; the sweep config
  MUST NOT gain publisher-pair fields in this feature.

**Documentation and record-keeping**

- **FR-020**: The verifiable region MUST be documented as: verifiability ⟺
  bucket count present (every dialed edge acceptor-checkable regardless of
  pick count; which-K freedom among valid edges is not a violation); bucket
  count absent = fully private selection, experiments-only on the protocol
  track. The same documentation names the acceptance gate as the future
  misbehaviour-evidence collection point while v1 keeps the silent drop.
- **FR-021**: The M4 label MUST be upgraded where it is currently disclaimed
  (the 015 quickstart, contracts, ADR 0032's modelling caveat), and the
  model quickstart MUST present the recipe families as plane coordinates:
  the formal models (picks only), their hash-gated versions (plus bucket
  counts), their capped versions (plus accept caps), and gated+capped —
  with M1 as the pick-count-0 case of M5 and the fan-out footgun stated.
- **FR-022**: Three implementation notes MUST be recorded: (a) gate-failing
  dials as provable-but-unrecorded evidence (trigger: the incentive/chain
  layer); (b) selection-seed privacy (the models prescribe private
  unpredictable randomness; the flag is a prototype stand-in adequate
  against oblivious adversaries; trigger: first adaptive-adversary
  experiment or real-crypto identity work); (c) sampled selection under
  view growth (a pick set is a function of the whole candidate view, so
  add-only dialing unions re-draws across view growth — the I3 ruling's
  addition; trigger: periodic heartbeats / epoch rotation, or the first
  staggered-boot fleet or experiment). Dispositions MUST be updated:
  N-032's trigger re-pointed to the first experiment needing symmetric ×
  capped; the balanced-B registry computation recorded as rejected
  (guidance only; registry-as-carrier open separately); E12's status
  corrected to ready with the flooding point identified as (bucket count
  pinned, no pick count) + silent-relay.
- **FR-023**: §1.2 item 1 rides: forwarding selection MUST require the
  active link state uniformly across fan-out policies (today's relay-kind
  match admits any state). Items 2–6, 8–10, 12–13 are absorbed by the
  rework; items 7 and 11 remain on the pickup list.
- **FR-024**: Housekeeping on the first docs commit: refresh the
  `pubsub-node/CLAUDE.md` active-work stanza; the E12 status line in the
  experiments program (the vehicle for FR-022's E12 disposition — one edit,
  not two); optionally a short ADR recording the configuration-placement
  rationale (CLI flags = one node's own knobs; TOML = shared world state or
  declarative sweep definitions).

**Validation contract**

- **FR-025**: The M2 model point MUST keep the formal selection family's
  semantics exactly: RF uniform picks without replacement per topic.
  Cross-version byte identity of experiment outputs is NOT required.
- **FR-026**: Implementation MUST follow the two-commit shape: commit A
  reproduces the current sampler derivation exactly (existing domain string,
  no epoch nonce or self-identity in the preimage) and MUST byte-diff
  identical against the recorded baselines (run records and aggregates;
  manifests may differ in tool commit and config text); commit B renames the
  domain, extends the preimage (epoch nonce, self-identity), lands the
  final CLI seed derivation (a provisional loader expansion may precede it —
  the node CLI has no recorded baselines; only the experiments-facing
  derivation is byte-identity-constrained), and re-baselines.
- **FR-027**: After landing: re-execute the m2-comparison, confirm
  statistical agreement with the formal values per that document's recorded
  methodology (raw counts with Wilson 95% intervals; exact-agreement checks
  where it defines them), update its document, and record fresh baseline
  generations per the baselines README, plus the first recorded baseline
  for the M4-completing recipe.
- **FR-028**: The within-version determinism battery is mandatory and
  unchanged: value-level determinism, replay-by-seed, and worker-count
  invariance of the three artifacts.

### Key Entities

- **Selection plane point**: a per-seam coordinate pair (bucket count, pick
  count); the four previous strategy kinds are the points (absent, absent),
  (absent, K), (B, absent), (B, K); boundary values express seam-off shapes.
- **Bucket count**: the hash-gate width B ≥ 2; fed configuration, shared per
  seam between the dialer's gate and the acceptor's verification (the
  agreement condition); absent = ungated.
- **Pick count**: the exact number of seeded uniform picks drawn among gate
  survivors; absent = all survivors; 0 = none.
- **Accept cap**: the fed absolute per-seam serving bound; absent =
  unbounded; 0 = serve none (explicit rejection).
- **Selection seed**: the operator-supplied u64 feeding the per-node draw
  derivation together with self-identity, epoch nonce, and topic; required
  exactly when sampling is configured.
- **Verifiable region**: the plane subset with bucket count present — every
  dialed edge acceptor-checkable; its complement is private selection,
  experiments-only on the protocol track.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: All twenty model recipes (M1–M5 across the four families:
  picks only, hash-gated, capped, gated+capped) are expressible as documented
  single-command configurations using only the knob surface — no strategy
  kind names anywhere in the configuration.
- **SC-002**: Commit A's M2 sweeps byte-diff identical against the recorded
  baseline artifacts; the landed feature's re-executed m2-comparison agrees
  with the formal values within the documented statistical bounds, and fresh
  baseline generations are recorded.
- **SC-003**: The M4 configuration exhibits, fleet-wide in tests: full
  reciprocity, minimum degree ≥ the pick count, and mean degree within 5%
  of 2× the pick count — and the M4 label is claimed (no un-amended
  "approximation" disclaimer remains in quickstart, contracts, or ADR
  caveats: every disclaiming site carries a dated amendment claiming the
  label, per the amendment-not-rewrite convention).
- **SC-004**: The determinism battery passes: identical artifacts under
  replay-by-seed and across worker counts.
- **SC-005**: E7 is runnable from the shipped M4 sweep configuration; E10's
  plane axes (`pick_count`, `bucket_count`) are shipped and
  boundary-verified down to run values — the E10 grid design itself (which
  crossings, at what scale) remains the experiment program's work, not this
  feature's; and E12's program status reads ready with no new machinery.
- **SC-006**: Two nodes sharing one selection-seed value produce
  statistically independent pick sets, and a node's picks are unchanged
  across repeated heartbeats but change across an epoch-nonce change.
- **SC-007**: Every misconfiguration named in this spec (bucket count < 2 on
  the CLI, sampling without a seed, a seed without sampling, verification
  opt-out without gating, a publisher seam activated without a dial knob,
  any unconsumed flag) fails startup with an actionable message, verified by
  tests.
- **SC-008**: The four dial strategy types, four acceptance strategy types,
  both kind enums, `resolve_buckets`, `bucket_count(len, target_degree)`,
  `accept_cap(K, c)`, and the `--bucket-count` / `--cap-buffer` flags no
  longer exist in the codebase.

## Assumptions

- All flag spellings are as decided in the pre-spec round; the verification
  opt-out spelling (`--relay-accept-unverified` /
  `--publisher-accept-unverified`) was confirmed in the 2026-07-31
  clarification session.
- Implementation-note numbers (the two new notes) are assigned at
  implementation time as the next free N-numbers.
- The experiments driver keeps deriving per-participant 32-byte seeds from
  the master seed; the CLI seed derivation is a separate construction site at
  the loader edge (parse-at-the-edge), and the two never interact.
- The epoch event remains unfired in v1 (single-epoch runs; add-only dialing
  caveats unchanged); the epoch nonce enters the draw preimage now so the
  existing rotation seam re-randomises picks when it is eventually exercised.
- Baseline re-recording cost is accepted as budgeted (~25 s at the operating
  point at 10 workers, per the baselines README).
- The pre-spec design record (`notes/017-unified-selection-pre-spec.md`,
  untracked) is the discussion-level rationale; this spec and the feature's
  ADRs/notes are the canonical record.
