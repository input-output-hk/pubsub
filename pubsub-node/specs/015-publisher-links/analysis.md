# Analysis ledger — 015-publisher-links

Cross-artifact + code-verification pass after implementation (constitution:
Development Workflow — spec fidelity verified against code when code exists).
Categories follow the analyze convention: findings are numbered, each with a
resolution; a finding without an entry here is not closed.

## Coverage summary

- FR-001..015 ↔ tests: FR-001/015 (`terminated_is_kind_scoped`,
  `topic_removal_cascades_over_publisher_links`), FR-002
  (`publisher_dials_fire_unconditionally`, integration establishment barrier
  with zero relay links), FR-003 (wire layout pin + tamper-kind test), FR-004
  (`relay_and_publisher_caps_count_independently`), FR-005/011
  (`fanout_splits_on_origin`, `dual_kind_peer_receives_one_send`), FR-006
  (`publisher_link_admits_owner_only`, integration owner-binding contrast),
  FR-007/008 (`all_links_fanout_unions_both_kinds_for_any_origin`,
  `any_verified_admits_foreign_publisher_over_publisher_link`, M5 chain),
  FR-009 (edge sym tests + M4 reciprocity/exact-edge-set), FR-010
  (`tampered_payload_severs_the_admitting_publisher_link` — both admitting
  paths), FR-012/013/014 (CLI axes + four getters + disabled-slot tests).
- SC-001..006: SC-001/002/003 by the three integration files; SC-004 audited
  in A4; SC-005 = contracts §6 recipes, all flags shipped (verified against
  `--help`); SC-006 — every carried correctness requirement has a named test.

## Findings

**A1 — TDD sequencing deviation on plumbing (process).** The kind-dispatched
control handlers, the `ForwardToAll` publisher facet, and the kind-aware cap
scan landed inside the Phase-2 reshape commit before their US1 tests were
written; those tests were then written and passed immediately (pins, not
drivers). The three genuinely novel protocol behaviours — the publisher
heartbeat pass, the publisher receive-gate admission, and admitting-link
severance — did have failing tests first (three red tests at the US1
checkpoint, then green). *Resolution*: recorded; the plumbing was
behaviour-preserving symmetric mechanics whose observable claims are all
pinned by the US1 suite. No spec change.

**A2 — "filtered helpers" from the spec Input realised differently.** The
Input sketch mentioned state helpers serving filtered per-kind views for
strategies to iterate. Shipped shape: the fan-out strategies select and
per-peer-deduplicate from the downstream map themselves (`BTreeSet` collect),
and acceptance uses the kind-aware `link_scan`; no additional public filtered
helper exists because nothing consumes one (constitution: consumer-justified
interfaces). *Resolution*: contracts §2 documents the strategy-internal
selection; no code change.

**A3 — wire layout-pin test edited (deliberate).** The signed-bytes layout
gained the trailing kind byte, so `signed_bytes_layout_is_stable` and the
tamper tests were re-pinned in the same commit (research R3, ADR 0032). The
only behavioural test edit in the feature.

**A4 — SC-004 audit ("pre-existing tests untouched except mechanical
rename").** `git diff origin/main` over pre-existing test files shows:
getter renames, `UpstreamState`→`LinkState`, tuple-key seeding →
`LinkKey::new(…)`, and `Node::new`/`NodeState::new` construction updated to
the `NodeStrategies` aggregate + admission default. The construction-shape
change is slightly more than a name swap but is mechanical at every site
(same collaborators, new grouping); **no assertion changed** in any
pre-existing test. New coverage is additive files only.

**A5 — archive tests ported as scenarios, not verbatim.** The M4/M5 and
publisher-link integration tests re-express the archive branch's scenarios
against the minimal API (`ConnectToExplicit` chains instead of the archive's
strategy family; content-polling instead of origin-matching for flood
coverage). The archive lesson "await Active links before publishing — no
retry means an early publish is lost" is encoded as the establishment
barriers in all three fixtures.

**A6 — shared parameters across seams.** `--bucket-count` and `--cap-buffer`
apply to relay *and* publisher instances (one B, one c). Documented in
contracts §5; independent per-seam values are a future flag split if an
experiment needs them (no consumer today).

**A7 — docs/decisions index.** tasks T010 mentioned adding ADR 0032 to a
`docs/decisions/README.md` index; no such index exists on `main` (it was an
archive-branch artifact). Creating one for this PR would be unrelated churn —
deferred; ADR 0032 stands alone like 0024–0031 do.

**A8 — review round 2 (in-chat PR review, 2026-07-17).** Four findings, all
addressed in one follow-up commit:
1. *M3 parameter-mapping off-by-one (docs)*: the recipe wrote
   `--publisher-degree S`, but the model's *s* counts the publisher **plus**
   its s−1 targets — the flag is the link count. Contracts §6 and the
   quickstart now state `S_LINKS = s − 1` explicitly.
2. *Latent flake in `tests/publisher_links.rs`*: the fleet triggered its retry
   heartbeat without awaiting candidate convergence (the M4 test did). A
   candidate barrier now precedes the trigger.
3. *Silent no-op flag combinations*: per maintainer direction, misconfigured
   flags now **fail at startup** (`validate_flag_combinations` in `main.rs`,
   exit 2): `--publisher-degree` without a publisher seam, `--symmetric-edges`
   without any hash-gated relay seam, `any-verified` without publisher
   acceptance. Relatedly, the degree-missing build errors now name the actual
   flag per seam (`--relay-degree` / `--publisher-degree`) instead of the
   retired `--target-degree`.
4. *Severed-link observability*: `Effect::Misbehaved` now carries the admitting
   link's `LinkKind`; the `connection_severed` log gains `link_kind`, and the
   severance tests pin the kind.
Also from the same review: `NodeStrategies.connection`/`.acceptance` renamed to
`relay_connection`/`relay_acceptance` for naming symmetry with the publisher
pair (maintainer-requested; mechanical at all construction sites), and the M4
comment wording fixed earlier (`6a11f6c`) after the maintainer caught the
"publisher links stay directional" phrasing contradicting `m4/README.md`.

**A9 — review round 3 (maintainer read-through, 2026-07-17).**
1. *Fan-out kind names*: `forward-to-all`/`all-links` sounded interchangeable.
   Renamed (maintainer's proposal): `forward-to-relays` (default — forwards
   held messages to relay downstream only; publisher links carry just the
   node's own publications, i.e. seeding) and `forward-to-all` (M5 — every
   held message over all Active links). Two-step type rename
   (`ForwardToAll`→`ForwardToRelays`, then `AllLinks`→`ForwardToAll`) so no
   call site could silently keep the old name with the new semantics.
2. *`EDGE_DOMAIN` scope*: since 015 the tag is relay-exclusive; renamed const
   to `RELAY_EDGE_DOMAIN` **and** — per maintainer, no experiment results
   exist yet to keep reproducible — the tag string itself to
   `pubsub/bucketed-pull/relay-edge/v1`.
3. *`NodeStrategiesBuilder`*: fields/params renamed `relay_connection`/
   `relay_acceptance` to match the A8 struct rename.

**A10 — review round 4 (in-depth harness-readiness review, 2026-07-17).**
1. *`k_in = 0` inexpressible / M1*: added `none` relay selection
   (`DialNone`) and acceptance (`AcceptNone`) kinds — push-only M1 (and M5
   `k_in`-sweeps to zero) are now CLI recipes; the M5/M1-shaped integration
   fleets use them instead of the test-only explicit-empty strategy.
2. *M4 + capped acceptance*: rejected at startup (one-sided capacity refusals
   silently break symmetric-pair reciprocity; the model has no caps).
3. Deferred with rationale: epoch link *rotation* (teardown of stale
   predicate edges) — the heartbeat diff only adds, so sims are
   one-epoch-per-run until a rotation feature lands; `seen` bounding stays
   N-021. Harness access to the `pub(crate)` pure core is **resolved**: the
   experiments framework will live inside the `pubsub-node` crate (harness
   owner's choice, 2026-07-17), i.e. a module in the library tree — note
   that same-package bins/tests/examples are separate crates and see only
   the public API, so the framework must be a `src/` module (a feature-gated
   `sim` module keeps the default build lean); no 015 change required.

**A11 — review round 5 (architecture review, EzequielPostan, 2026-07-22).**
Changes-requested review; the kinded-key data model (A-series baseline)
explicitly stands. Addressed in one code commit + this docs pass (ADR 0034):
1. *"M4" was not M4 (the request-changes item)*: the symmetric-predicate
   realisation drew unordered pairs i.i.d. — reciprocity *emergent* and
   conditional on B-agreement, no minimum-degree floor, expected total degree
   ≈ RF where the formal M4 has mean ≈ 2·RF / min ≥ RF. Fixed the mechanics
   half: bidirectionality now lives in the **symmetric handshake** (acceptor
   records both directions, dialer mirrors on Accepted, teardown/severance
   remove both halves) — reciprocity unconditional, composing with any
   selection. The remaining half — the uniform exactly-RF selection kind
   ((B = 1, K = RF)) that restores the min-degree floor — is a **follow-up
   feature** (belongs with the 005-lineage B-as-parameter fix); until it
   lands the quickstart/contracts label the recipe an **approximation**, not
   M4. The three compensating artifacts the review predicted deletable were
   deleted: the capped-acceptance startup prohibition (+ its ADR bullet and
   contracts note), the seam-coupling *rationale* (the single flag remains
   as convenience), and the reliance on four-message emergent pairing.
2. *Kind field → message vocabulary*: `Message::{Relay,Publisher,Symmetric}Connection`
   route to `state/handlers/{relay,publisher,symmetric}.rs`; `PlainConnection`
   loses `kind`; `signed_bytes(HandshakeKind)` binds the tag from the variant
   (relay/publisher preimages byte-identical; symmetric `0x02` is new — the
   pre-split symmetric mode spoke relay bytes, a nuance the review's
   "byte-level free" skipped; flagged in the reply, cheap pre-009). Shared
   lifecycle mechanics became composed helpers; the stored `LinkKind` stays
   (traffic class vs establishment protocol — deliberately not 1:1).
3. *Module organization*: handlers named by link class (not MX number), per
   the review; per-vocabulary unit suites mirror the split
   (`state/tests/symmetric_links.rs`).
4. *Docs*: ADR 0034 authored; ADR 0032 amended (§2/§6 supersession, capped
   bullet, M4-label deferral, relay-edge/v1 rename recorded); contracts §1/§3/§6/§7,
   research R3/R6, data-model §6, quickstart M4 section corrected (the stale
   "`edge/v1` unchanged" pins the review caught).
*Process note*: the symmetric-handshake behaviours were implemented together
with the vocabulary refactor in one commit; the new `symmetric_links` suite
pins all nine behavioural claims (both-maps accept, dialer mirror, crossing
idempotency, unsolicited drop, mirror teardown, mirror severance,
whole-edge capacity refusal, fail-closed on directional nodes, dial-side
vocabulary), and the M4 integration test's assertions were untouched.

**A12 — post-round-5 self-audit (model-by-model alignment pass, 2026-07-22).**
Each M1–M5 definition (`formal_spec/hybrid_dissemination/models/`) was
re-checked against the recipes, mechanics, and tests. M1 (as the M5
`k_in = 0` boundary), M2 (serving-set-only seeding via relay downstream), M3
(both sides of initiation exclusivity), and M5 (`ForwardToAll` = own k_out ∪
in-pickers, split-horizon) align with their definitions, modulo the two
documented global deviations (binomial hash-gated draws vs exactly-k uniform
picks — all models, not only the symmetric case; single-epoch runs, A10.3).
Fixes from the audit:
1. *Mixed-vocabulary leftover*: a symmetric node accepted inbound **relay**
   handshakes into `downstream` only — a one-way link on a node whose
   teardown/severance assume every relay link is mirrored. Now dropped
   (`relay_handshake_disabled`), the mutual guard of
   `symmetric_edges_disabled`; ADR 0034 §4 amended; pinned by
   `relay_request_on_symmetric_node_is_dropped`.
2. *Dead code*: `HandshakeKind::name()` had no consumer — removed
   (constitution: consumer-justified interfaces).
3. *Operator text*: the `--symmetric-edges` hash-gate startup error claimed
   "has no effect" — no longer literally true post-ADR-0034; reworded to name
   the actual requirement (the predicate only the hash-gated strategies
   evaluate). `AcceptanceParams::symmetric` doc similarly updated (a seam
   mismatch now costs dropped dials, never one-way links).
4. The PR description was rewritten to the post-round-5 state (constructed
   reciprocity, vocabulary wire shape, M4-approximation label, ADR 0034).
5. *Shutdown notices deduplicated by `LinkKey`* (follow-up from the audit
   discussion): a link held in both collections — every symmetric link, by
   construction — previously produced two `Terminated` notices to the same
   peer (the second absorbed as `unknown_termination`). One link now gets
   one notice; a peer with links of both kinds still gets one per kind.
   **Deliberate behavioural test edit**: `shutdown_notifies_each_role_of_a_
   both_roles_pair` (which pinned the two-notice behaviour) re-pinned as
   `shutdown_notifies_a_both_roles_pair_once`; a symmetric-vocabulary
   shutdown test added beside it.

Raised with the maintainers rather than ledgered as decisions (design
questions the model text cannot answer): whether M3's initiation-link
owner-binding is identity-based or origination-based (proxy/submit-API
publishing — three-option Slack thread), and what a capped acceptance should
bound on a symmetric node (total degree vs peer-initiated edges; no published
recipe combines caps with symmetric edges today).

**A13 — maintainer answers to the two A12 questions (2026-07-23).**
1. *Owner-binding: removed entirely* (the answer was "neither option" — a
   publisher-link arrival is validated **exactly like any message**:
   publisher signature, topic registration, publisher authorization,
   subscription, dedup; no receive-side owner check). Two independent
   grounds, recorded in the ADR 0032 §5 supersession note: semantically the
   binding only narrowed the publisher channel below what any Active relay
   upstream already admits (M3's exclusivity is the *sender-side* fan-out
   rule — honest-behaviour compliance, and the model's adversaries are
   silent); mechanically it compared the signed `publisher_id` against the
   **unsigned** frame sender (spoofable under a real transport — the
   frame-sender-authentication transport obligation is now recorded in
   N-013) and erased the `PublisherId`/`PeerId` type distinction via raw-key
   comparison (breaks silently when publisher identity diverges from node
   identity, N-009). Implementation: the receive gate collapsed to one
   kind-agnostic lookup (any `Active` upstream entry for `(topic, from)`
   admits — which also *tightened* the publisher arm from key-presence to
   `Active`); `PublisherAdmission`, `UnknownPublisherAdmission`, the
   `--publisher-admission` flag, and its startup check are deleted (the
   enum never ships as public API). **Deliberate behavioural test edits**:
   `publisher_link_admits_owner_only` re-pinned as
   `publisher_link_admits_any_authentic_message`; the integration pin
   `foreign_publisher_is_dropped_at_publisher_links` re-pinned as
   `foreign_publisher_is_admitted_over_publisher_links`; the US3
   `any_verified_…` unit test folded into the re-pinned FR-006 test (its
   behaviour is now universal); the M5 chain test drops the flag — its M3
   contrast run still stops at b, now provably via the sender side alone.
   Spec FR-006 amended / FR-008 removed; R8 superseded; recipe table: M3 and
   M5 now differ **only** in `--fanout-strategy` (stated deliberately, per
   the maintainer's knock-on note).
2. *Capped acceptance × symmetric: deferred without resolution* per the
   maintainer — no consumer, so no semantics ahead of one. The current
   behaviour (cap scan counts the whole mirrored set; the gate fires only on
   peer-initiated requests) and the revisit trigger (first experiment
   needing a capped bidirectional strategy) are recorded as **N-032**; no
   code change.

## Verification notes

- `main.rs --help` output matches contracts §5 flag-for-flag (checked at
  polish); quickstart recipes use only shipped flags.
- `lib.rs` re-exports match contracts §2–§4: `LinkKind`/`LinkKey`/`LinkState`
  (`PublisherAdmission` + its unknown-name error deleted at A13),
  `ForwardToRelays`/`ForwardToAll`/`FanoutStrategyKind`,
  `is_valid_edge_sym`; `is_valid_edge_publisher` stays crate-internal (no
  external consumer).
- Suite at completion: 243 tests, clippy pedantic clean, fmt clean; after the
  A11 round: 255 tests, clippy pedantic + fmt clean; after A13: 252 tests
  (one unit test folded into a re-pinned one), clippy pedantic + fmt clean.
