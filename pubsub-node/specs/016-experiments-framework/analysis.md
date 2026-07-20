# Analysis Ledger — 016-experiments-framework

Findings from `/speckit-analyze` runs and their resolutions (Constitution,
Development Workflow: the ledger, not commit messages, closes findings).

## Session 1 — 2026-07-19 (post-tasks, pre-implementation)

Scope: spec.md (post-clarify, incl. the 2026-07-18 delegation session) ↔
plan.md + research/data-model/contracts/quickstart (post-checklist
convergence) ↔ tasks.md (T001–T026, incl. the T026 amendment).
Constitution v1.2.0 gates evaluated. No CRITICAL or HIGH findings; coverage
100% (every FR/SC ≥ 1 task); duplication 0.

| ID | Category | Severity | Location(s) | Finding | Resolution |
|----|----------|----------|-------------|---------|------------|
| C1 | Coverage | MEDIUM | FR-010 ↔ T006/T007/T015 | The publishes-per-run knob (default 1; repeat publish with fresh messages, no state reset) was covered only by adjacency — no task text owned the repetition behaviour or its test | **Resolved**: named explicitly in T006 (failing-test scope), T007 (orchestration), and T015 (config surface) |
| C2 | Coverage | MEDIUM | SC-005 ↔ T012/T016 | Record boundedness (nothing O(N)) had no explicit test citation — the shape was built (T012) but never asserted against N | **Resolved**: T016 gains the boundedness assertion — two runs differing only in N at fixed target_degree; histogram lengths bounded by realised max degree/depth + 1 and near-constant across N; no array field scaling with N; structural inventory pinned by the golden serialization test. (Noted: in degenerate topologies — full mesh — max degree is N−1 and the bound holds definitionally; the test uses the bounded-degree configuration where N-independence is visible.) |
| I1 | Inconsistency | LOW | T026 ↔ plan.md structure | T026's `docs/experiments/m2-comparison.md` location was absent from the plan's structure tree (the T026 amendment postdates the plan) | **Resolved**: `docs/experiments/` added to plan.md's source-structure tree with the amendment note |
| A1 | Ambiguity | LOW | contracts/sweep-config.md | `--workers` default "number of cores, capped" leaves the cap unspecified | **Resolved** (2026-07-19, follow-up): default = available cores, no invented cap; the flag row now carries the memory guidance (each in-flight run holds a full population — size explicitly at large N); result-neutral by contract guarantee 1 |
| A2 | Terminology | LOW | FR-018 ↔ data-model §5 | "sent-to-down" (identity prose) vs `sends.down` (record field) — one concept, two spellings | **Resolved** (2026-07-19, follow-up): data-model §5 states the realisation — `sends.down` is the record field for the spec's "sent-to-down" term; one concept, one field |

Metrics: 33 FR + 8 SC · 26 tasks · coverage 100% · ambiguity 1 ·
duplication 0 · critical 0.

### Convergence pass (Session 1, re-scan after remediation) — zero findings

Re-checked the remediation edits first (T006/T007/T015 now cite FR-010's
knob consistently with the spec's fresh-messages/no-reset semantics; T016's
new assertion matches SC-005's wording and the R9 test layering; plan tree
now consistent with T026), then re-ran the detection passes over the
artifact triangle: no new findings.

## Session 2 — 2026-07-19 (after A1/A2 follow-up resolutions) — zero findings

On review it was decided to resolve A1 and A2 in the artifacts rather than
carry them as implementation notes. Re-checked those edits first (the
`--workers` default is now specific and consistent with the quickstart's
memory guidance and the plan's risk note; the `sends.down` realisation
clause is consistent with the spec's identity prose and the output
contract's field listing), then re-ran the full detection passes:
**zero findings — converged.** Metrics: ambiguity 0 · duplication 0 ·
critical 0 · coverage 100%.

Ready for `/speckit-implement` (fresh session per project convention). A
post-implementation analyze round remains required by the constitution's
spec-fidelity rule (artifact claims verified against code once code
exists).

## Session 3 — 2026-07-19 (post-implementation, spec-fidelity)

Scope: artifact claims verified **against the implemented code** (public
API surface, record/artifact schemas, config surface, quickstart
procedure), not only against each other — spec.md, plan.md, research.md,
data-model.md, both contracts, quickstart.md, the shipped configs +
`configs/experiments/README.md`, `docs/experiments/m2-comparison.md`, and
ADRs 0032–0034, checked against `src/experiments/*`,
`src/bin/experiments.rs`, `src/lib.rs`, `Cargo.toml`, `src/state.rs`, and
the framework test suite. The quickstart procedure was executed (smoke
config end-to-end, worker-count byte-identity, per-node detail, the
`[[axes]]` example, the suite smoke test), not just read. The raw sweep
artifacts cited by m2-comparison.md are intentionally uncommitted
(reproducible byte-for-byte from the recorded master seeds + tool
commits) — by design, not a finding.

| ID | Category | Severity | Location(s) | Finding | Resolution |
|----|----------|----------|-------------|---------|------------|
| F1 | Fidelity | HIGH | contracts/sweep-config.md (axes example + rules); quickstart "Run a single experiment" | The contract's illustrative axes shape was a single `[axes]` table keyed by parameter name; the implementation deserializes an `[[axes]]` array of tables (`parameter` + `values`, `deny_unknown_fields`) — a file written as the contract showed would fail to parse. The array-of-tables shape is the deliberate choice: declaration order is load-bearing (cross-product, first-declared axis slowest) and plain TOML tables don't preserve key order | **Resolved**: contract example and rules rewritten to the `[[axes]]` form with the ordering rationale; quickstart's "omit `[axes]`" aligned. Doc follows code |
| F2 | Fidelity | MEDIUM | contracts/output-artifacts.md guarantee 4; data-model §6; spec FR-028 + glossary | The manifest was described as carrying "fixed parameters, axes" as fields; `SweepManifest` serializes exactly tool commit, master seed, seed-derivation rule, runs-per-experiment, and the expanded experiment list — axes and fixed parameters appear only expanded into the per-experiment resolved parameter sets | **Resolved**: all four artifacts now state the expansion is the representation; the self-description mandate is met by the fully resolved experiment list |
| F3 | Fidelity | MEDIUM | contracts/sweep-config.md strategy tables; configs/experiments/README.md | `bucket_count` (optional pinned B, hash-gated kinds) and `cap_buffer` (default 3, bounded kinds) are accepted, validated, and embedded in the manifest's strategy tables, but were undocumented on the config surface | **Resolved**: rule bullet added to the contract; parameter note added to the README |
| F4 | Fidelity | LOW | contracts/sweep-config.md validation list; data-model §7 | "More than one topic" / "multi-topic requests" was listed as a validation rejection, but no such validator exists — the schema admits exactly one `topic` (a multi-topic request is a serde parse error, not a semantic check). Conversely, the enforced conflicting count/fraction rejection (`adversarial` vs `adversarial_fraction`, `churn` vs `churn_count`) was unlisted | **Resolved**: both lists reworded — one-topic stated as a schema property, conflict rule added |
| F5 | Fidelity | LOW | data-model §3; research R6 | Seed rule spelled `truncate(SHA-256(master_seed \|\| run_index))`; the code hashes with domain prefix `experiments/run-seed/v1` and keeps the full digest | **Resolved**: both artifacts corrected; the manifest's verbatim `seed_derivation` string noted as the normative spelling. (plan-input.md keeps the old wording — verbatim record, exempt) |
| F6 | Fidelity | LOW | contracts/sweep-config.md intro | "The binary owns file I/O and argument parsing" — the binary owns argument parsing and config reading; the three output artifacts are written by the library sweep layer (its module doc: the only layer performing I/O) | **Resolved**: intro reworded |
| F7 | Fidelity | LOW | data-model §6 | `ExperimentAggregates` also serializes bookkeeping fields `experiment` (manifest index), `runs`, `publishes` — absent from the documented inventory | **Resolved**: added |
| F8 | Fidelity | LOW | data-model §5 | `PerNodeDetail` rows are keyed per (publish, node) and carry `publish` + `node` fields — absent from the documented inventory | **Resolved**: stated |
| F9 | Fidelity | LOW | quickstart build commands + M2 step 3 | (a) The `--bins` line was annotated as what builds the binary, but `required-features` means the plain feature build already builds it (verified empirically); (b) step 3 cited `sends.honest` mean as an aggregates field — the aggregates spell it `sends_honest_mean`, copies-per-honest-node is derived, and the depth distribution is `depth_hist_pooled` | **Resolved**: one build line with an accurate annotation; exact aggregate field names cited |
| I2 | Consistency | LOW | m2-operating-point.toml; quickstart M2 step 1; README; m2-comparison.md | Operating-point runtime drift: "up to ~1 h" (TOML) / "≲ 1 h" (quickstart) vs the measured ~13 min at `--workers 1` (m2-comparison) and README's ~15 min | **Resolved**: harmonized to ~15 min at `--workers 1` (release); memory figures were already consistent |
| I3 | Consistency | LOW | plan.md Scale/Scope | "Eight submodules under `src/experiments/`" vs the nine the plan's own structure tree lists (and the code has) | **Resolved**: nine |
| P1 | Process | MEDIUM | configs/experiments/README.md | The directory README was authored at wrap-up but never committed (untracked; its three TOMLs landed in `e14a3f2`). Content verified accurate against code this session apart from the F3/I2 touches above | **Resolved**: reconciled and committed with this round |

Notes (not findings): the **no-core-changes claim** (plan, FR-027, SC-008)
holds for the feature's own commits — the `docs-experiments-program...016`
diff touches core only via a `#[cfg(feature = "experiments")] pub(crate)`
accessor block in `src/state.rs` and the gated module declaration in
`src/lib.rs`. A naive diff against `main` additionally shows the inherited
rustdoc intra-doc-link commit (`25353de`, PR #74 lineage), which the
planned rebase onto `main` removes from the 016 diff.

Verified accurate against code (spot list): feature gate + optional
`serde_json` + `required-features` binary (ADR 0034); the two
experiments-only strategies incl. the `min(target_degree, available)`
degeneracy; wavefront canonicalisation and the run phase order (ADR 0032);
good ⟺ one SCC, pre/post-churn passes, condensation-sink
min-publisher-coverage, M2-only dispatch; `{count, runs, p, wilson95}`
estimates and the `full_coverage.count ≥ good.count` fold assertion (ADR
0033); the accounting identity and miss-cause decomposition; per-node
detail opt-in result-neutrality; byte-identity across worker counts
(tested); shipped TOML parameter values vs m2-comparison.md and the
formal-spec citations; CLI flag surface and defaults.

### Convergence pass (Session 3, re-scan after remediation) — zero findings

Re-checked the remediation edits first, each against the code fact it now
states: the `[[axes]]` example matches the deserializer's own test shape;
the aggregate field spellings (`sends_honest_mean`, `depth_hist_pooled`)
and the `CountEstimate` shape match `statistics.rs`; the seed rule matches
the `SEED_DERIVATION_RULE` string embedded in every manifest; the
`SweepManifest`/`ExperimentAggregates`/`PerNodeDetail` inventories match
the serde structs field-for-field; `cap_buffer` default 3 and optional
`bucket_count` match `StrategyTable`; the validation list matches the
parse-time checks (incl. the conflicting count/fraction rejection). Then
re-scanned all 016 artifacts + shipped configs + m2-comparison.md for
residues of every finding: remaining occurrences are only the intended new
wording and the verbatim-exempt records (plan-input.md, spec.md's Input
blockquotes). `cargo fmt --check` clean; full suite green under
`--features experiments` (only doc/comment files changed).
**Zero findings — converged.** Metrics: fidelity 9 · consistency 2 ·
process 1 · critical 0 · all resolved in-session.
