# ADR 0039: The unified selection plane — one implementation per seam over two fed knobs

**Status**: Accepted
**Date**: 2026-08-01
**Feature**: `specs/017-unified-selection/` (the PR #77/#102 §1.1 follow-up)

## Context

After 015/016 the crate carried four dial-side selection strategies
(connect-to-all, hash-gated, dial-none, and the experiments-only uniform
sampler) and four acceptance baselines (accept-from-all, bounded,
hash-gated, hash-gated-bounded, plus an accept-none off-switch), each pair
wired through a kind enum, per-kind CLI flags, and a two-phase builder.
Three defects had accumulated around that shape:

- **The knobs fought instead of composing.** Under a pinned bucket count the
  configured degree had no dial-side effect at all (hash-gated selection
  dialed every predicate survivor), and no configuration could express
  exactly-K uniform picks — the formal models' selection family — outside
  the experiments feature gate.
- **The derived bucket count was a security and correctness liability.**
  Deriving `B` from the live membership count is adversary-influenceable (a
  grinding surface: joining/leaving shifts everyone's gate) and only
  verifiable while both endpoints see identical candidate sets — the
  B-agreement assumption that the readiness gate and the pinned
  `--bucket-count` flag existed to paper over.
- **The cap formula anchored on a number that no longer governs.**
  `accept_cap(target_degree, c)` is only honest when a target degree
  actually predicts expected in-degree; once the dial side draws exact
  picks, a formula-computed cap silently misstates the operator's serving
  commitment.

## Decision

**One selection implementation per seam over two independently optional fed
knobs.** `Selection` (dial) gates candidates by the seam's verifiable edge
predicate at **the bucket count** (absent ≡ B = 1: everyone survives), then
draws exactly `min(pick count, survivors)` seeded uniform picks without
replacement (**the pick count** absent = all survivors; 0 = none). The four
dial strategies are its coordinate points — (absent, absent), (absent, K),
(B, absent), (B, K) — and their types, both kind enums, and the two-phase
builder are deleted. The trait seams (`ConnectionStrategy`,
`ConnectionAcceptanceStrategy`) and injection sites are unchanged.

**The bucket count is fed, never derived.** The `resolve_buckets` derive arm
and the `bucket_count(len, target_degree)` formula are removed; no component
may compute a bucket count from membership or view state. The small-topic
connect-to-all floor becomes the parameter-setter's responsibility, with the
balanced-point guidance (B ≈ candidates/K) documented beside the model
recipes.

**Acceptance merges into `UnifiedAcceptance` with two independent
dimensions.** Gate verification follows the seam's bucket count — acceptors
verify exactly the `B` the dialers use, the agreement condition
verifiability rests on — with one explicit per-seam opt-out
(`--*-accept-unverified`, resolved to `gate: None` at construction, never a
runtime branch). The serving cap is a **fed absolute per-seam value**;
`accept_cap(K, c)` and `--cap-buffer` are deleted and the ⌈K + c·√K⌉
headroom formula moves to documentation as parameter-choosing guidance.

**The CLI is knob-only, per-seam, presence-activated, with zero boundary
values replacing the `none` kinds.** `--relay-{bucket-count,pick-count,
accept-cap}` (+ `--relay-symmetric`, renaming `--symmetric-edges`, and
`--relay-accept-unverified`) and the publisher mirrors replace the kind and
degree flags with no aliases. Bucket counts are ≥ 2 on the CLI (gating is
signalled by the flag's presence; a one-bucket gate is vacuous), while core
construction and the sweep config accept 1 as the ungated boundary axis
point. Pick count 0 = dial none (M1's relay-off); accept cap 0 = serve none
via the explicit over-capacity rejection — a deliberate behavioural change
from the deleted off-switch's silent drop (the dialer's pending entry is now
cleaned up). The publisher seam activates on any of its knobs but requires a
dial knob (`--publisher-pick-count`, 0 permitted, or
`--publisher-bucket-count`); unconsumed flags fail startup.

**The fan-out default flips to `forward-to-all`.** M1/M2/M4/M5 need no
fan-out flag; `forward-to-relays` remains as the explicit M3-exclusivity
switch — M3 is the model *defined* by its exclusivity rule, so it carries
the mark. A node with publisher links and no fan-out flag runs M5 semantics,
stated as a footgun in the help and quickstart.

**The verifiable region is: bucket count present.** With B ≥ 2 every dialed
edge is acceptor-checkable regardless of the pick count — dialing fewer than
all valid edges is a private choice within the verifiable edge set, not a
violation (which-K freedom is unverifiable by construction). Bucket count
absent = fully private selection: the formal family, experiments-only on the
protocol track. A gate-failing dial is provable misbehaviour (signed request
+ publicly recomputable predicate); v1 keeps the silent drop, and the
acceptance gate is the future evidence-collection point (N-036 records the
trigger: the incentive/chain layer).

## Consequences

- Every selection behaviour the node previously offered, plus two it could
  not express (exactly-K uniform picks as a node capability; gated picks
  under a real cap), is a documented knob combination; the twenty model
  recipes (M1–M5 × four families) are single-command configurations with no
  kind names anywhere.
- The (bucket count absent, pick count = RF) point plus the constructed-
  reciprocity symmetric handshake (ADR 0034) realises the formal M4 exactly
  — minimum degree ≥ RF by construction — so the "M4 approximation" label in
  the 015 quickstart, contracts, and ADR 0032's modelling caveat is upgraded
  to a claim.
- Sampling needs randomness the node never had: `--selection-seed` and its
  derivation chain are a separate decision (ADR 0040), with the refactor
  itself pinned byte-identical to the recorded experiment baselines first
  (commit A) and the derivation change landing as one deliberate,
  re-baselined commit (commit B).
- Supersession notes: the strategy seams of ADR 0018/0023 are unchanged, but
  their named v1 implementors (`ConnectToAllCandidates`,
  `AcceptFromAllCandidates`) are deleted — both behaviours are the plane
  origin. This ADR supersedes the strategy-kind and builder wording of
  ADR 0028 (two-phase construction — the construction-with-validation
  principle survives in `NodeStrategies::new`; the key-resolution phase is
  gone) and the four-baseline decomposition of ADR 0031 (the four points
  survive as knob combinations; the one-strategy-per-file layout of ADR 0029
  now holds one policy per seam). ADR 0024/0025's predicate, admission
  vocabulary, and silent-drop/`Rejected` split are unchanged; their
  derived-B wording (`bucket_count`, `resolve_buckets`, the B-agreement
  caveat as a runtime assumption) is superseded — agreement is now by
  construction at the configuration edge. ADR 0030's shared-predicate
  placement is unchanged.
- The experiments framework's config speaks the same coordinates (the
  `uniform-sampler` kind and `target_degree`/`cap_buffer` fields are gone),
  gains the symmetric switch, and keeps boundary values as legal axis points
  (`bucket_count = 1`, `pick_count = 0`) that the operator CLI rejects — two
  edges, each honest to its consumer, over one core meaning.

## Alternatives rejected

- **Keeping the kind enums and adding a uniform kind.** Perpetuates four
  implementations per seam plus a fifth, and the defect that the knobs are
  properties of kinds rather than dimensions of one policy.
- **Deriving the balanced bucket count in the topic registry.** Computing
  B = S_T/K in the registry would fix local derivation's verifiability
  problem (one agreed source) but not the security problem — it is still B
  recomputed from live membership, the same grinding surface with the lever
  moved rather than removed. Rejected as mechanism; the balanced-point
  formula survives as operator guidance only. The registry as a **carrier**
  of a governance-set per-topic B (agreed data, never computed from
  membership) stays open as a separate future feature.
- **Auto-scaling B from membership at the node** (the deleted derive arm's
  continuation). Rejected for the same grinding surface, plus the
  B-agreement fragility under any future discovery/view-sampling layer.
- **Keeping `forward-to-relays` as the default.** Preserves 015's
  conservative posture but makes four of the five models carry a flag while
  the one model defined by its exclusivity rule reads as the natural
  default; overridden in favour of flag-free M1/M2/M4/M5 with the footgun
  stated loudly.
- **A formula-computed accept cap with a pick-count anchor.** Honest again
  now that a pick count exists, but it re-couples two independently
  meaningful commitments (how many I dial vs how many I serve) and hides the
  operator's serving bound behind arithmetic; fed absolute caps keep the
  commitment explicit.
