# Checklist: Cross-artifact consistency and obligation completeness

**Purpose**: Requirements-quality gate over the three risk areas named in the
checklist request — knob-surface consistency across artifacts,
validation-contract testability, obligation completeness — before
`/speckit-tasks`.
**Created**: 2026-07-31
**Feature**: [spec.md](../spec.md) · [plan.md](../plan.md) ·
[contracts/node-cli.md](../contracts/node-cli.md) ·
[contracts/sweep-config.md](../contracts/sweep-config.md) ·
[quickstart.md](../quickstart.md)

## Knob-surface consistency

- [x] CHK001 - Is every CLI flag introduced by the feature named with
  identical spelling in the spec (FR-006, FR-011, FR-014), the node-cli
  contract, and the quickstart recipes? [Consistency, Spec FR-006]
- [x] CHK002 - Are the knob value domains (bucket count, pick count, accept
  cap) stated identically across spec FR-007, the node-cli contract's flag
  table, the sweep-config contract, and the data-model domain table?
  [Consistency, Spec FR-007]
- [x] CHK003 - Is the deliberate CLI-vs-sweep divergence for bucket count 1
  stated on **both** sides (the node-cli contract pointing at the sweep
  exception, the sweep-config contract pointing at the CLI rejection)?
  [Consistency, Spec FR-007/FR-018]
- [x] CHK004 - Is the pick count 0 boundary value's legality and meaning
  (dial none / k = 0 axis point) stated consistently across spec FR-002,
  both contracts, and the quickstart M1 recipe? [Consistency, Spec FR-002]
- [x] CHK005 - Are the publisher-seam activation rules (any knob activates;
  acceptance-side knobs alone rejected naming the accept-only spelling)
  identical in spec FR-008, the node-cli contract, and the quickstart
  cautions? [Consistency, Spec FR-008]
- [x] CHK006 - Is the deleted-flag list complete and identical between the
  spec and the node-cli contract — including the shared `--bucket-count`
  and `--cap-buffer`, not only the kind and degree flags? [Completeness,
  Spec FR-006/FR-012]
- [x] CHK007 - Is the fan-out default flip and its M5-footgun consequence
  stated consistently in spec FR-009, the node-cli contract, and the
  quickstart caution? [Consistency, Spec FR-009]
- [x] CHK008 - Is the selection-seed requiredness rule (required iff any
  seam pick count ≥ 1; rejected as unused otherwise) identical in spec
  FR-014, the node-cli validation matrix, and the quickstart? [Consistency,
  Spec FR-014]
- [x] CHK009 - Is the per-seam agreement condition (one bucket-count value
  feeding both the dial gate and acceptor verification) stated wherever the
  bucket count is defined (spec FR-011, node-cli contract, data-model
  invariants)? [Consistency, Spec FR-011]
- [x] CHK010 - Are the verification opt-out flag's spelling, per-seam
  mirrors, and its rejected-when-unconsumed rule consistent across spec
  FR-011, the Clarifications session, the node-cli contract, and the
  quickstart? [Consistency, Spec FR-011]
- [x] CHK011 - Does the sweep-config coordinate vocabulary in spec FR-017
  name **every** field the sweep-config contract adds — including the
  verification opt-out — or does the contract introduce fields the spec
  never requires? [Completeness, Spec FR-017]
- [x] CHK012 - Is the `--relay-symmetric` rename (from `--symmetric-edges`)
  and the deletion of the old symmetric-requires-hash-gated validation rule
  recorded consistently in the spec and the node-cli contract?
  [Consistency, Spec FR-006]

## Validation-contract testability

- [x] CHK013 - Is the commit-A byte-identity gate stated with the exact
  artifacts compared, the permitted differences (manifest tool commit +
  config text), and the comparison baseline named? [Measurability,
  Spec FR-026]
- [x] CHK014 - Is the commit-B statistical-agreement requirement anchored
  to a documented methodology (which bounds, whose definition) rather than
  the bare phrase "statistical agreement"? [Clarity, Spec FR-027]
- [x] CHK015 - Is the M4 baseline obligation traceable end-to-end: a shipped
  sweep configuration requirement, the recording procedure, and the
  artifact set it produces? [Traceability, Spec FR-027]
- [x] CHK016 - Is the determinism battery enumerated (its three checks) and
  its unchanged status stated as a pass/fail requirement? [Measurability,
  Spec FR-028]
- [x] CHK017 - Is "the M2 point keeps the formal selection family's
  semantics exactly" given an objective definition (exactly
  min(RF, candidates) uniform picks without replacement per topic)?
  [Measurability, Spec FR-025]
- [x] CHK018 - Are the M4 topology success numbers objectively testable —
  in particular, is "mean degree ≈ 2× pick count" quantified with a
  tolerance a test can assert? [Measurability, Spec SC-003]
- [x] CHK019 - Is the A→B commit-ordering constraint expressed as a
  requirement (not only plan narrative), so tasks can gate on it? [Clarity,
  Spec FR-026]

## Obligation completeness

- [x] CHK020 - Does the SC-008 deletion inventory exactly match the union
  of deletions required by FR-005 and FR-012 — no artifact deleted by an FR
  missing from SC-008, none in SC-008 without an owning FR? [Consistency,
  Spec SC-008]
- [x] CHK021 - Are the two implementation notes each owned by exactly one
  requirement, with content and revisit trigger stated? [Completeness,
  Spec FR-022]
- [x] CHK022 - Is the N-032 trigger update owned once and consistent with
  the edge-case text describing the symmetric × cap behaviour?
  [Consistency, Spec FR-022]
- [x] CHK023 - Is the E12 status correction's ownership unambiguous between
  FR-022 (the disposition content) and FR-024 (the docs-commit vehicle) —
  complementary by explicit cross-reference, not duplicated? [Conflict,
  Spec FR-022/FR-024]
- [x] CHK024 - Are the M4 label-upgrade sites enumerated in one owning
  requirement (quickstart, contracts, ADR 0032 caveat) and echoed by
  SC-003's no-remaining-disclaimer criterion? [Completeness, Spec FR-021]
- [x] CHK025 - Are the §1.2 rider dispositions complete: item 1 owned as a
  requirement, the dissolved items enumerated, and the stay-behind items
  (7, 11) explicitly excluded? [Completeness, Spec FR-023]
- [x] CHK026 - Is the balanced-B rejection recorded with both residues (the
  formula as operator guidance; registry-as-carrier open separately) in one
  owning requirement? [Completeness, Spec FR-022]

## Notes

- Pass records appended below after each evaluation run (multi-pass
  convergence: re-run until a recorded zero-finding pass).
- **Pass 1 (2026-07-31)**: 19/26 pass, 7 findings, all resolved by artifact
  edits in the same round:
  - CHK003 — node-cli contract lacked the pointer to the sweep-side
    bucket-count-1 divergence (sweep side already pointed back). Fixed: note
    added to the validation-matrix row.
  - CHK006/CHK020 — spec FR-006's deleted-flag list omitted the shared
    `--bucket-count`; SC-008's inventory likewise. Fixed: both amended;
    FR-005's "kind enums" corrected to the singular dial-side enum.
  - CHK011 — spec FR-017 omitted the verification opt-out from the
    sweep-config coordinate vocabulary while the contract adds the
    `accept_unverified` field. Fixed: FR-017 amended.
  - CHK014 — FR-027's "statistical agreement" not anchored. Fixed: anchored
    to the m2-comparison's recorded methodology (raw counts + Wilson 95%;
    exact-agreement checks where defined).
  - CHK018 — SC-003's "mean degree ≈ 2× pick count" had no assertable
    tolerance. Fixed: "within 5% of 2× the pick count".
  - CHK020 — FR-010 implied but did not state the acceptance-type +
    acceptance-kind-enum deletions SC-008 checks. Fixed: stated explicitly.
  - CHK023 — FR-022/FR-024 both named the E12 correction without
    cross-reference (duplicate-task risk). Fixed: FR-024 marked as the
    vehicle for FR-022's disposition ("one edit, not two").
- **Pass 2 (2026-07-31)**: re-evaluated the pass-1 edits first, then the
  full list — 26/26 pass, **zero findings**. Convergence recorded; gate
  clear for `/speckit-tasks`.
