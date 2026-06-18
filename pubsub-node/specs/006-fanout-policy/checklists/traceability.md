# Traceability & Correctness Checklist: Message Publishing and Fan-out Forwarding

**Purpose**: Validate that every behavioral guarantee is specified completely, consistently, and traceably across the artifacts (success criteria → functional requirements → acceptance scenarios → fan-out contract → implied test obligation), with no orphaned guarantee and no claim left without a concrete acceptance scenario. A requirements-quality gate run **before** `/speckit-tasks`.
**Created**: 2026-06-16
**Feature**: [spec.md](../spec.md)

**Note**: Evaluated in one pass against spec.md, plan.md, research.md, data-model.md, and contracts/fanout-protocol.md. `[x]` = the requirement is well-formed and traced; `[ ]` + **FLAG** = a requirements-quality gap to resolve. Findings summarized at the end.

## Guarantee Traceability (end-to-end coverage)

- [x] CHK001 Is **duplicate suppression** stated as a measurable outcome, a requirement, and a concrete scenario consistently? [Completeness, SC-003 / FR-012 / US3 AS1 / contracts §3]
- [x] CHK002 Is **loop termination** (bounded forwarding in a cyclic mesh) given a measurable criterion and a scenario? [Measurability, SC-005 / US3 AS3 / data-model §6]
- [x] CHK003 Is **receive-path split-horizon** traced from a success criterion through an FR to a scenario? [Coverage, SC-004 / FR-009 / US2 AS2 / contracts §2.3]
- [x] CHK004 Is the **no-seen-set-poisoning** guarantee (dedup runs *after* signature verification) pinned by a concrete acceptance scenario, not only an FR and an edge-case bullet? [Coverage, FR-013 / US3 AS4 / contracts §3.4 — **F1 resolved**: US3 AS4 added (same-hash invalid then genuine → recorded)]
- [x] CHK005 Is **proxy/injection** (publisher ≠ node) specified with both a permissive FR and a positive acceptance scenario? [Completeness, FR-003 / US1 AS3 / contracts §1.5]
- [x] CHK006 Is **subscriber-relay** (never relay an unsubscribed topic) anchored to a dedicated requirement or scenario, rather than only a derived property and edge-case bullet? [Traceability, contracts §2.6 / Edge Cases — **F3 resolved**: edge case now states it is a structural consequence of the 004 acceptance gate, deliberately not a new FR/scenario]
- [x] CHK007 Is **verbatim forwarding** (signature unchanged, no re-sign) backed by an acceptance scenario asserting the forwarded message is byte-identical to the original, not just an FR? [Coverage, FR-007 / US2 AS5 / contracts §2.4 — **F2 resolved**: US2 AS5 added]
- [x] CHK008 Is the **`Origin` / `received_messages()` public-surface change** specified across requirement, contract, and public-surface delta? [Consistency, FR-014 / contracts §1.3,§5 / data-model §1.1]
- [x] CHK009 Is the **`Origin::Peer(id)` value** (not just `Local`) explicitly asserted in a relay acceptance scenario? [Clarity, FR-014 / US2 AS1 — **F4 resolved**: US2 AS1 now records with origin `Peer(X)`]

## Requirement Completeness (every FR / SC has a home)

- [x] CHK010 Does every functional requirement FR-001..016 appear in the fan-out contract's traceability table? [Completeness, contracts §6]
- [x] CHK011 Do all six success criteria SC-001..006 map to at least one acceptance scenario? [Coverage, US1–US3]
- [x] CHK012 Are the **drop causes** for both paths enumerated (incl. the new `duplicate`) and consistent between spec, research, and contract? [Consistency, contracts §4 / research R8]
- [x] CHK013 Is the **publish validation order** (subscribed → registered → authorized → signature → dedup, minus connection gate) stated identically in spec, data-model, and contract? [Consistency, FR-002 / data-model §2 / contracts §1.2]
- [x] CHK014 Is the **receive validation order** (gate → … → signature → dedup) stated with dedup explicitly appended after verification? [Clarity, data-model §3 / contracts §3.2]

## Requirement Clarity (no unquantified terms)

- [x] CHK015 Is the **dedup key** unambiguously defined (`MessageHash::of(&plain)`, content hash) rather than a vague "message id"? [Clarity, FR-012 / research R2]
- [x] CHK016 Is "**fan-out target**" defined precisely (downstream peers on the topic, minus the excluded peer) rather than "the peers"? [Clarity, FR-008 / contracts §2.2]
- [x] CHK017 Is **fire-and-forget** publish made unambiguous (no return verdict; outcomes via logs/`received_messages()`)? [Clarity, FR-001 / contracts §1.1]
- [x] CHK018 Is the **`exclude` argument** semantics specified for both paths (`Some(deliverer)` on receive, `None` on publish)? [Clarity, FR-009 / data-model §4]

## Requirement Consistency (no cross-artifact conflict)

- [x] CHK019 Does the spec's "no new effect variant" claim match the contract and data-model (fan-out reuses `Effect::Send`)? [Consistency, FR-011 / contracts §2.4]
- [x] CHK020 Is the **severance-vs-plain-drop** distinction consistent (receive path severs on bad signature; publish path is a plain drop, no upstream)? [Consistency, FR-005 / data-model §2,§3]
- [x] CHK021 Is **dedup placement relative to severance** non-conflicting (severance fires before dedup; a tampered message never marks seen)? [Consistency, data-model §3]
- [x] CHK022 Does the determinism claim (set-deterministic targets, unspecified order) align with the out-of-scope pick-k rationale across spec, research, and ADR? [Consistency, research R1 / ADR 0021]

## Acceptance Criteria Quality (objectively verifiable)

- [x] CHK023 Can SC-001 ("100% of downstream record") be objectively measured against `received_messages()`? [Measurability, SC-001]
- [x] CHK024 Can SC-002 ("all N members record") be verified without implementation detail? [Measurability, SC-002]
- [x] CHK025 Are US3's "exactly once" / "finite forwards" claims phrased as observable outcomes? [Measurability, US3 AS3]
- [x] CHK026 Is the acyclic-vs-cyclic distinction in US2 AS4 / US3 unambiguous enough to keep US2 testable without dedup (the round-2 independence fix)? [Consistency, Clarifications 2026-06-16]

## Edge Case & Scenario Coverage

- [x] CHK027 Are the **empty-downstream** (publish + receive) cases specified? [Coverage, FR-016 / US1 AS2 / Edge Cases]
- [x] CHK028 Is the **split-horizon-collapses-to-no-op** boundary (deliverer is sole downstream) addressed? [Edge Case, US2 AS3 / Edge Cases]
- [x] CHK029 Is the **re-publish-identical-content** case covered by dedup, and is that stated? [Coverage, contracts §1.6]
- [x] CHK030 Is **equivocation** explicitly scoped out with a rationale (distinct content ⇒ distinct hash ⇒ both propagate)? [Coverage, Edge Cases / data-model §7 D3]

## Structural Requirements (validated by contract, not behavioral scenario)

- [x] CHK031 Is the **`FanoutStrategy` seam** (injected at construction, like `ConnectionStrategy`) specified in the public-surface delta even though it has no behavioral scenario? [Completeness, FR-010 / contracts §5 — acceptable: structural, exercised by the `ForwardToAll` unit test + construction]
- [x] CHK032 Is the **unbounded `seen` set** documented as a deliberate deferral with a follow-up home? [Assumption, data-model §7 D1 / ADR 0021]

## Notes

- Items are `[x]` (well-formed + traced) or `[ ]` + **FLAG** (gap to resolve). Pass 1 raised 4 flags (F1–F4), all coverage/clarity — no conflicts or contradictions. Pass 2: all four resolved (see below); 0 open flags.

### Findings (resolved 2026-06-16)

- **F1 (FR-013, no-poisoning)** — *coverage*. **Resolved**: added US3 AS4 — a same-content-hash message with an invalid signature is dropped at verification, then the genuine valid message is still recorded (failed verification did not pre-seed `seen`). Wording pins the same-hash construction (a content-altered "tampered" message would hash differently and could not poison).
- **F2 (FR-007, verbatim forwarding)** — *coverage, most substantive*. **Resolved**: added US2 AS5 — the forwarded message is byte-identical to the received one, signature unchanged, no re-sign.
- **F3 (subscriber-relay)** — *traceability, mild*. **Resolved**: the off-topic edge case now states the property is a structural consequence of the 004 connection-acceptance gate (downstream only on member topics), deliberately not a new FR/scenario.
- **F4 (`Origin::Peer`)** — *clarity, minor*. **Resolved**: US2 AS1 now records with origin `Peer(X)`.

**Profile**: 32 items, **32 pass, 0 open flags** after pass 2. Spec acceptance scenarios tightened pre-tasks; the new scenarios (US2 AS5, US3 AS4) are explicit TDD obligations for `/speckit-tasks`.

### Pass 3 — confirmation re-audit of the pass-2 edits (2026-06-16)

Re-checked the four edits for newly-introduced inconsistency (the clarify round-2 style scan). One **new** issue found and fixed:

- **F5 (US3 AS4 conflated with severance)** — the no-poisoning scenario originally had the invalid-signature copy arrive *over an Active upstream*, which is 004's misbehavior trigger: it **severs** the upstream, so the genuine follow-up would be dropped `not_connected` — the scenario asserted a false outcome and demonstrated severance, not dedup ordering. **Resolved**: the invalid copy is now **published** (a plain drop, no upstream to sever), isolating the dedup-after-verification property. US2 AS5, US2 AS1, and the F3 note re-checked — no conflicts.

**Profile after pass 3**: 0 open flags; decreasing finding profile across passes (4 → 0, then 1 → 0). Converged.
