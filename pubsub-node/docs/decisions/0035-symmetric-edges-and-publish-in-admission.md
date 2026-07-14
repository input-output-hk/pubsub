# 0035 — Symmetric edges and publish-in admission: closing M4 and M5

**Status**: Accepted (feature 015; completes ADR 0034's model-family programme — M4/M5 land in-feature instead of as 016/017)

**Context**: ADR 0034 made the dissemination models configuration and shipped M3; M4 and M5 were deferred to follow-on features. Review discussion concluded the remaining gaps are small enough to close in-feature: M4 (`m4/README.md` — every pick a bidirectional edge, flooding on all incident links, no seeding) lacked only a **symmetric edge predicate**; M5 (`m5/README.md` — k_in pull picks + k_out outbound picks, *both* carrying every held message) lacked a **union fan-out kind** and a **relaxation of the publisher-binding receive gate** (ADR 0033 §5), which is M3's owner-exclusivity and must not apply to M5's k_out links.

## Decision

1. **Symmetric edge predicate** (`edge::is_valid_edge_sym`): hash the **unordered** peer pair (canonical byte order) under per-role symmetric domain tags (`…/edge-sym/v1`, `…/publish-edge-sym/v1` — independent draws from the directional domains for *every* pair, including already-ordered ones). Both ends compute the same expected edge set, dial each other, and each link materialises as the **Out+In pair on both sides** — the emergent bidirectionality research R10 anticipated; no stored `Both`, no new control flow, no wire change.
2. **One flag wires both seams.** `symmetric: bool` on `SelectionParams`/`AcceptanceParams`, surfaced as a single CLI flag `--symmetric-edges` applied to the relay selection **and** acceptance hash-gated kinds together — a per-seam flag would let the two sides disagree and silently drop every dial as illegitimate. Publish seams stay directional (no model needs symmetric standing links).
3. **M4 needs no fan-out kind.** Under pair emergence every neighbour is a `relay_in` entry, so the existing `forward-to-all` already floods all incident links, split-horizon is the arrival-link exclusion, and per-peer dedup covers the Out+In pair. ADR 0034's anticipated "incident-flood kind" was unnecessary.
4. **`role-agnostic` fan-out kind** (M5's send side — no link-role distinction): every held message — any origin — over `relay_in ∪ publish_out(Active)`, minus the arrival link, deduplicated per peer. Origin plays no role (the M5 semantics: k_out links relay everything).
5. **`PublishInAdmission` receive-gate policy** (M5's receive side): `OwnerOnly` (default — M3's exclusivity, the ADR 0033 §5 binding unchanged) vs `AnyVerified` (admit any payload over `publish_in` whose remaining checks pass). A per-node config value on `NodeState` (CLI `--publish-in-admission`), **not** a strategy seam: it is per-*message* admission with exactly two published-model variants — a trait would be unconsumed generality. Severance is policy-independent: an invalidly-signed payload severs the admitting link under either policy.
6. **Model recipes** (all per-node config): **M3** = `hash-gated` relay + `hash-gated` publish + `forward-to-all` + `owner-only`. **M4** = `hash-gated` relay `--symmetric-edges` + publish `none` + `forward-to-all` (+ default gate; `publish_in` never populated). **M5** = `hash-gated` relay (k_in) + `hash-gated` publish (k_out) + `role-agnostic` + `any-verified`.

## Consequences

- All three target models are end-to-end configurations of one node; integration tests pin M4 (reciprocity of the Out+In pairs + full-coverage flood over a predicate-connected graph) and M5 (a foreign-publisher message hopping a→b→c purely over standing links, admitted by the relaxed gate — the exact hop `owner-only` drops).
- The send-side kind and receive-side gate are separate knobs and must be paired network-wide for M5 (`role-agnostic` ⇄ `any-verified`); deliberately not fused into a `--model` preset (ADR 0034's rejected alternative — the axes stay independently sweepable).
- **Modelling caveat (documented, inherited)**: the verifiable-hash overlay approximates the models' private exactly-k uniform picks with binomial-around-k predicate draws — for M4 this also means expected degree ≈ RF with no min-degree guarantee, versus the model's own-picks-honoured minimum degree. This is the same approximation class the M2/M3 realisation already carries (bucketed-pull replaces private sampling with verifiable hashing); the cross-model experiments quantify the gap against the models' laws.
- `LinkRole::Publisher` naming for M5's everything-carrying k_out links remains a documented misnomer (ADR 0034 alternatives); revisit only if it confuses in practice.

## Alternatives rejected

- **Symmetric as new kind names** (`hash-gated-symmetric`, `hash-gated-bounded-symmetric`, …) — kind-name explosion across two seams for what is one orthogonal predicate mode; the flag composes.
- **Same domain tag for symmetric hashing** — pairs whose directional preimage is already in canonical order would collide with the directional draw, correlating configurations that should be independent.
- **Admission policy as a strategy seam** — per-message admission with two variants and no third consumer; a config enum is the honest shape (forward-compatible-interfaces standard).
- **Redefining `forward-to-all` as the union** — would silently break M3's owner-exclusivity and the behaviour-preserving default; the union is its own kind.
