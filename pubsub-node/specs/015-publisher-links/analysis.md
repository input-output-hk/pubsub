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

## Verification notes

- `main.rs --help` output matches contracts §5 flag-for-flag (checked at
  polish); quickstart recipes use only shipped flags.
- `lib.rs` re-exports match contracts §2–§4: `LinkKind`/`LinkKey`/`LinkState`/
  `PublisherAdmission` (+ unknown-name error), `ForwardToRelays`/`ForwardToAll`/`FanoutStrategyKind`,
  `is_valid_edge_sym`; `is_valid_edge_publisher` stays crate-internal (no
  external consumer).
- Suite at completion: 243 tests, clippy pedantic clean, fmt clean.
