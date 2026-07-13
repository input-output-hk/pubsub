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

## Implementation-vs-artifact spot checks (Constitution: spec fidelity verified against code)

- `lib.rs` exports match the contract's public-surface list (`LinkRole`, `LinkDirection`, `LinkState`, `Links`, `PublishStrategy`, `NoPublishLinks`, `HashGatedPublish`, `PublishStrategyKind`, `PublishParams`, `PublishAcceptanceParams`, `is_valid_edge_for`; `UpstreamState` absent) — verified by grep.
- `signed_bytes` layout pin test matches the contract's wire section (role tag after topic; `0x00`/`0x01`).
- Relay edge-domain bytes unchanged (`pubsub/bucketed-pull/edge/v1`) — SC-001's by-construction leg.
- CLI flags and defaults match data-model §7 (`--publish-strategy none`, `--publish-acceptance-strategy accept-from-all`); missing-degree error path exercised against the built binary.
- Suite: 239 tests green; clippy `-D warnings` clean; fmt clean.
