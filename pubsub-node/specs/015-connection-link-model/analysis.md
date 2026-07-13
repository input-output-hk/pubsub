# Analysis ledger — 015 unified link model & publishing links

Cross-artifact + artifact-vs-implementation consistency pass (Constitution: Development Workflow / analysis ledger), run 2026-07-13 after implementation.

## Findings

### A1 — Contract claimed a `NodeView::has_relay_downstream` helper the implementation never needed (Inconsistency, low)

`contracts/link-model-and-seams.md` and `data-model.md` §3 listed an observed-downstream helper on the view. The M3 trigger was resolved (research R6) to read the **expected** relay downstream via the public predicate, so no consumer of an observed-downstream helper exists; implementing it would have been unjustified forward shaping (Constitution: forward-compatible interfaces need a named consumer). **Resolution**: helper removed from the contract and data model; contract carries a pointer to this entry. Code untouched.

### A2 — Spec US1 "existing suite passes unchanged" is behavioural, not textual (Clarification, low)

US1's independent test says the existing suite passes "unchanged against the new model". The suite required *textual* migration — the `UpstreamState → LinkState` rename, role fields on control-message constructors, `Node::new` gaining the two publish-seam parameters — while every assertion and behaviour pin stayed as-is. **Resolution**: read as behavioural equivalence (the SC-001 sense); the parity evidence is the untouched assertions plus the full-suite green run (239 tests). No spec edit — the Success Criteria (SC-001) already state the behavioural form.

### A3 — Terminated semantics narrowed by role scoping (Deviation, documented)

Pre-015, a `Terminated{topic}` removed the pair's entries in **both** structures. Post-015 a `Terminated{topic, role}` removes only that role's entries (both directions); the other role's links between the same pair survive — required by the coexisting-links clarification (a publisher tearing down its publishing link must not sever an unrelated relay link). A counterpart pre-015 node cannot exist (the wire format changed in the same commit), so no compatibility window arises. **Resolution**: ADR 0032 (4) + state test `terminated_is_role_scoped`; FR-014's 004-era "either role" wording in the historical 004 spec is superseded for role-carrying messages, noted here rather than editing the frozen 004 artifacts.

### A4 — Shutdown notice count grows with the store (Observation, no action)

`handle_shutdown` emits one `Terminated{role}` per link entry; a pair holding Out+In of the same role receives two notices (as pre-015), and a dual-role pair up to four. The redundant ones are absorbed by the counterpart's unknown-termination rule, unchanged. Deterministic order is new (BTreeMap key order) — strictly an improvement for reproducibility.

### A5 — `--bucket-count` is shared across relay and publish seams (Observation, follow-up candidate)

The CLI's single `--bucket-count` pins `B` for the relay seams **and** `B_p` for the publish seams (the params structs each carry it, but the edge wires the same flag into both). A per-seam override (`--publish-bucket-count`) is a two-line CLI addition when an experiment first needs asymmetric pinning; not added now (no consumer). Quickstart documents the shared knob.

### A6 — Publisher binding compares key bytes across newtypes (Verified, no action)

The receive gate's publisher binding compares `PublisherId::as_public_key()` with `PeerId::as_public_key()` — deliberate: the binding is "the link peer's own key signed this message", and both newtypes expose the same underlying `PublicKey`. Type-level separation (PublisherId ≠ PeerId) is preserved everywhere else; the single comparison site is commented in `handle_dissemination`.

### A7 — Role-symmetric dial-seam naming + shared selection core (Refinement, review follow-up)

Review question: should the publish seam's method be `expected_downstream` to mirror `expected_upstream`? Resolved the opposite way — "downstream" already canonically means relay `In`-links (getters, `OC` cap, the M3 trigger's own phrasing), and publish targets carry only `Origin::Local` traffic, so the orientation word both collides and overstates. Instead the **relay** method was renamed to match the role-based convention: `ConnectionStrategy::expected_upstream` → **`expected_relay`**, pairing with `expected_publish` (the meeting's relaying-/publishing-links vocabulary; orientation stays derived per ADR 0032 — and `expected_relay` remains correct under 016's symmetric predicate where a relay link is no longer purely upstream). Alongside, the duplicated ~15-line selection loop in `HashGatedConnection`/`HashGatedPublish` was extracted into a shared `edge::hash_gated_selection` helper. Tactical per Constitution III (local rewrite to reverse); recorded here, no ADR. *(Addendum, post-A8: the 0034 unification left the helper with a single caller, so it was inlined back into `HashGatedSelection::expected_links` — `edge.rs` returns to pure predicates; 016's symmetric variant re-extracts it when a second consumer exists.)*

### A8 — Model-family alignment: trigger removed, seams unified, fan-out is the model knob (Revision, maintainer + formal-spec driven)

Denis's executable models (M1–M5) landed on `main` after this branch was cut and are the authoritative source. Three revisions followed (ADR 0034): **(1)** the M3 trigger was removed — `m3/README.md` opens the s−1 standing initiation links **unconditionally**, contradicting the earlier clarify answer; `HashGatedSelection` selects with no relay-side condition and the trigger machinery (incl. its `relay_degree` parameter on the publish strategy) is gone. **(2)** The per-role strategy types merged: one `LinkSelectionStrategy` family + one role-parameterised acceptance family; the store became cell-structured (`LinkStore`) so strategies select/union model-prescribed fields. **(3)** The maintainer's strict-partition reading of M3 ("publishing only uses publishing links, relaying only relaying links") vs the model text's "a forwarder relays every message it holds" (under which a publisher also serves its own message to its requesters) is resolved by **configuration, not code**: `forward-to-all` (default, union reading, behaviour-preserving) and `role-scoped` (strict partition) are both fan-out kinds; the experiments cross-validate each against the model's coverage laws. The `role-scoped` + `--publish-strategy none` combination is a documented **mute-publisher** configuration. M4 = 016's symmetric predicate + an incident-flood kind; M5 = a union kind + relaxing the 0033 §5 publisher binding — both deferred with named consumers.

### A9 — PR-review findings and fixes (2026-07-13)

A full-diff review of PR #77 surfaced and fixed: **(1)** a severance gap — an invalidly-signed payload admitted via an inbound initiation link emitted `Misbehaved` but removed the (absent) relay cell entry, leaving the misbehaving publisher's standing link alive; the severance now removes the **admitting** link (relay upstream when the relay gate passed, else the `publish_in` link), with regression tests for both paths and for non-collateral severance. **(2)** duplicate wire sends under `forward-to-all` when a peer sat in both the relay-in and publish-out cells for a local origin — targets are now deduplicated per peer (ordered set; the duplicate would have skewed the models' expected-message metric). **(3)** `docs/experiment-readiness.md` (spike-branch meeting notes) had been swept in by a bulk `git add`; removed from the PR. **(4)** `plan.md`/`research.md` gained supersession banners for the 0034 rework (this ledger's A8). Minor: `LinkCell` exported (it appears in public signatures); redundant topic clone in the severance arm dropped.

### A10 — Fan-out kind labelling corrected: `forward-to-all` IS M3 (Refinement, maintainer re-read)

The maintainer's re-read of `m3/README.md` settled the question A8 had left to "both readings, experiments arbitrate": relay links carry **both** relayed traffic and the node's own publications ("a forwarder relays every message it holds to its requesters"), while initiation links are owner-exclusive. That is exactly `forward-to-all` — the default kind is the M3 semantics, and **no code behaviour changed** with this finding. `role-scoped` is relabelled from "the strict M3 partition" to a strict-partition **experimental variant prescribed by no published model**, retained as an experiment lever (it isolates the initiation links' marginal contribution to coverage by removing publisher→requester direct serving). ADR 0034's decisions 4–5 amended in place (same feature, unmerged branch); code docs, CLI help, spec session, quickstart, data-model, and contract relabelled.

### A11 — M4 and M5 closed in-feature (maintainer-directed scope extension)

Maintainer direction ("fill those gaps, not document them") pulled the M4/M5 work in-feature (ADR 0035): the **symmetric edge predicate** (`is_valid_edge_sym`, unordered-pair hashing under `…/edge-sym/v1` domains; one `--symmetric-edges` flag wiring relay selection + acceptance so the seams cannot disagree; Out+In pair emergence per R10 — and no M4 fan-out kind needed, `forward-to-all` floods all incident links under pair emergence, retiring ADR 0034's anticipated "incident-flood kind"); the **`flood-all`** fan-out kind (M5's union send side); and the **`PublishInAdmission`** receive-gate policy (`owner-only` default | `any-verified`) as a config enum, not a seam (two published variants, per-message admission). FR-005's cross-kind invariant is now explicitly waived under the M5 pairing (FR-015). End-to-end pins: M4 reciprocity + full-coverage flood over a predicate-connected 12-node graph; M5 foreign-publisher relay a→b→c purely over standing links (the exact hop `owner-only` drops). Documented modelling caveat: verifiable-hash draws are binomial-around-k vs the models' exactly-k private picks (M4 additionally loses the min-degree guarantee) — the same approximation class the M2/M3 realisation already carries; the experiments quantify it.

## Implementation-vs-artifact spot checks (Constitution: spec fidelity verified against code)

- `lib.rs` exports match the contract's public-surface list (`LinkRole`, `LinkDirection`, `LinkState`, `LinkStore`, `LinkSelectionStrategy`/`Kind`, `HashGatedSelection`, `NoLinks`, `FanoutStrategyKind`, `RoleScopedFanout`, `SelectionParams`, role-carrying `AcceptanceParams`, `is_valid_edge_for`; `UpstreamState`/`Links`/`PublishStrategy` absent) — verified by grep.
- `signed_bytes` layout pin test matches the contract's wire section (role tag after topic; `0x00`/`0x01`).
- Relay edge-domain bytes unchanged (`pubsub/bucketed-pull/edge/v1`) — SC-001's by-construction leg.
- CLI flags and defaults match data-model §7 (`--publish-strategy none`, `--publish-acceptance-strategy accept-from-all`); missing-degree error path exercised against the built binary.
- Suite: 231 tests green post-rework (the trigger tests retired with the trigger); clippy `-D warnings` clean; fmt clean.
