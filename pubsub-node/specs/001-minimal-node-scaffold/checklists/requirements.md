# Specification Quality Checklist: Minimal PubSub Node Scaffold

**Purpose**: Validate specification completeness and quality before proceeding to planning

**Created**: 2026-05-17

**Feature**: [spec.md](../spec.md)

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain
- [x] Requirements are testable and unambiguous
- [x] Success criteria are measurable
- [x] Success criteria are technology-agnostic (no implementation details)
- [x] All acceptance scenarios are defined
- [x] Edge cases are identified
- [x] Scope is clearly bounded
- [x] Dependencies and assumptions identified

## Feature Readiness

- [x] All functional requirements have clear acceptance criteria
- [x] User scenarios cover primary flows
- [x] Feature meets measurable outcomes defined in Success Criteria
- [x] No implementation details leak into specification

## Notes

- Q1 (Ping response semantics) resolved on 2026-05-17: option A — Ping is one-way fire-and-forget; no response message is generated; receipt is observable on the receiver per FR-006. FR-004 was updated accordingly; the `[NEEDS CLARIFICATION]` marker has been removed.
- The audience for this spec is developer-researchers contributing to the pubsub-node implementation; technical vocabulary necessary to the domain (peer descriptor, InMemory network, opaque numeric value) is retained. The "non-technical stakeholders" item is interpreted as "no gratuitous jargon or framework-specific terms," and the spec passes that reading.
- The Assumptions section forwards two planning-stage hints to `/speckit-plan` (Rust language is no longer mentioned in this iteration's description; the InMemory "hashmap of message boxes" shape from the user's description is forwarded explicitly). These are clearly marked as planning inputs, not spec requirements.

- Items marked incomplete require spec updates before `/speckit-clarify` or `/speckit-plan`.

---

# Pre-`/speckit-tasks` Readiness Pass

**Purpose**: validate requirements quality across spec.md + plan.md + research.md + data-model.md + contracts/ as a whole, in preparation for `/speckit-tasks`.

**Audience**: reviewer / co-maintainer (also usable as author self-check).

**Created**: 2026-05-18

**Scope**: ~30 items spanning post-clarification spec quality, acceptance/success criteria measurability, plan & research quality, contracts quality, cross-artifact consistency, and remaining coverage gaps.

Numbering continues from the 16 items above (last legacy item is the implicit CHK016 of the original spec-quality pass).

## Post-Clarifications Spec Quality

- [x] CHK017 - Is the Ping "fire-and-forget" wording in FR-004 reconciled with FR-013's "enqueue complete" contract so an implementer cannot read them as contradictory? [Consistency, Spec §FR-004 vs §FR-013] — _fixed 2026-05-18: FR-004 now cites FR-013 for the precise completion notion._
- [x] CHK018 - Is the receive-task's existence and lifetime described at the spec level (or is its existence only inferable from plan.md / data-model.md)? [Gap, Completeness] — _resolved 2026-05-18 as intentional deferral: the receive-task is a plan-level mechanism (research.md §6, data-model.md §5) realising FR-006 + FR-013; the spec deliberately stays silent on the impl model to avoid pinning a specific concurrency mechanism._
- [x] CHK019 - Does the spec define whether `received_messages()` returns a snapshot or a live view, or is that decision left to contracts/data-model? [Clarity, Spec §FR-006] — _fixed 2026-05-18: FR-006 now pins snapshot semantics ("stable for the caller and unaffected by subsequent receptions")._
- [x] CHK020 - Is the Edge Cases bullet about unknown-peer drops (Spec line 74) updated to match FR-010's tightened "warn-level structured log entry" wording? [Consistency, Spec §Edge Cases vs §FR-010] — _fixed 2026-05-18: Edge Cases bullet now pulls forward FR-010's "warn-level structured log entry naming the unknown identifier" wording and cross-references FR-010._
- [x] CHK021 - Is "trust-on-arrival" defined with enough precision that an implementer knows exactly which checks are omitted? [Clarity, Spec §FR-003] — _verified 2026-05-18: FR-003 enumerates the omitted checks ("authentication, authorization, or admission check") and FR-007 covers the cryptographic class; no vagueness remains._
- [x] CHK022 - Does the spec specify who attributes the sender id on a received message (the network, the sender Node, or the message itself)? [Gap, Spec §FR-006] — _fixed 2026-05-18 with a deeper refactor than the checklist item anticipated: FR-006 now commits to the **logical peer identity** (matches `PeerDescriptor::id()`) in the record while staying silent on the *how* (iteration-dependent); separately, `Network::send(from, to, message)` was removed in favour of `NetworkHandle::send(to, message)` (actor-handle pattern from Lighthouse / sc-network), so the sender id is implicit in the handle and never asserted by callers. Updated: spec FR-006; data-model.md §4 (typed tx/rx + NetworkSender), §5 (Node holds handle), §8 (re-exports), §9 (matrix); contracts/library-api.md (Network trait, new NetworkHandle section, Node send, Versioning); research.md §6 (take_receiver mention) and new §12 (full Decision/Rationale/Alternatives + sources for ADR 0007); plan.md ADR count (six → seven)._
- [x] CHK023 - Is the relationship between `--self-id` (CLI flag in FR-012) and `PeerId` validation specified at the spec level, or only inferable from data-model? [Gap, Spec §FR-012] — _fixed 2026-05-18: FR-012 now requires `--self-id` to be validated against the same identifier rules as TOML peer entries (cross-refs FR-009 and FR-001), with startup failing on invalid input analogous to US3 AS-2._

## Acceptance Criteria & Success Criteria Quality

- [x] CHK024 - Is US1 AS-1's "exactly one Ping(42)" robust against a duplicate delivery — i.e., is at-most-once/exactly-once delivery specified anywhere? [Completeness, Spec §US1 AS-1] — _fixed 2026-05-18: FR-013 now pins exactly-once delivery as a v1 property conditional on the Trust + Liveness assumptions, with an explicit forward-disclaimer that the guarantee is NOT expected to survive into networked iterations (where failure handling + delivery semantics will be re-specified)._
- [x] CHK025 - Is SC-004's "without consulting any other document" unambiguous about whether `quickstart.md` is "another document" or the canonical entry point? [Ambiguity, Spec §SC-004] — _fixed 2026-05-18: SC-004 now names `quickstart.md` as the canonical entry point and is explicit that plan/research/contracts/ADRs/external docs are the things the contributor SHOULD NOT need to consult._
- [x] CHK026 - Does SC-005 specify how `N` should vary across the 100 sends (random / sequential / deterministic seed) so the assertion is reproducible? [Clarity, Spec §SC-005] — _fixed 2026-05-18: SC-005 now requires either a deterministic sequence (e.g., `0..100`) or seeded randomness, citing Engineering Standards "Reproducible tests"._
- [x] CHK027 - Is "local execution time (excluding build/compile time)" in SC-001 measurable from the test harness without manual stopwatch use? [Measurability, Spec §SC-001] — _verified 2026-05-18: `cargo test`'s own `finished in X.XXs` line is the measurement; build/compile time is separately reported by Cargo and excluded by definition. No spec edit needed._
- [x] CHK028 - Are the US2 demonstration bounds (2 ≤ N ≤ 10) reflected anywhere as an enforceable assertion or test parameter? [Traceability, Spec §US2] — _resolved 2026-05-18 as intentional non-enforcement: the bounds are scope markers ("for demonstration purposes"), not runtime invariants. The Independent Test fixes N=4 (star with A→{B,C,D}) and that is the concrete demonstration; enforcing N ≤ 10 anywhere in the system would actively contradict the trait-based design (an InMemoryNetwork with 100 nodes is perfectly correct, just outside this iteration's demo scope). N ≥ 2 is enforced naturally by the two-node test shape itself._

## Plan & Research Quality

- [x] CHK029 - Is each of the six planned ADR slots (0001–0006) tied to a single, named structural decision (no slot covering more than one decision implicitly)? [Completeness, research.md §"ADR slot summary"] — _verified 2026-05-18: now seven slots (0007 added during CHK022 resolution). 0001–0005 are atomic single decisions; 0006 explicitly bundles receive-task + registration timing as "conjoined" in the table; 0007 explicitly bundles three aspects of the actor-handle pattern in its title. No implicit / hidden bundlings remain._
- [x] CHK030 - Does the "Open follow-ups" section in research.md account for every plan-level item the spec deferred (mailbox bounding, duplicate-id detection, identity evolution, log shipping)? [Coverage, research.md §"Open follow-ups"] — _fixed 2026-05-18: expanded from 4 to 9 entries to cover all spec-flagged deferrals — added topic/sequence/chain message semantics, peer discovery + dissemination protocols, broadcast/multicast send, peer-set dynamics (FR-008), and failure-mode delivery semantics (FR-013's CHK024 disclaimer). Each new entry includes its spec trace._
- [x] CHK031 - Is the per-sender FIFO ordering claim in research.md §9 traceable to a specific Network impl property (e.g., the `mpsc` per-channel guarantee), not just asserted? [Traceability, research.md §9] — _verified 2026-05-18: research §9 cites `tokio::sync::mpsc`'s per-channel FIFO guarantee directly; the post-CHK022 typed `rx: UnboundedReceiver<Envelope>` field in data-model.md §4 makes the binding visible at the type level. One-hop traceability is intact._
- [x] CHK032 - Is the await-on-delivery helper's default timeout policy specified (e.g., 1 second), or is it left entirely to per-test discretion? [Clarity, research.md §10 vs contracts/library-api.md] — _verified 2026-05-18: policy is explicit at two layers — the helper API has no implicit default (mandatory `timeout: Duration` argument per research §10 + library-api.md, citing Engineering Standards "Reproducible tests"), and a recommended convention of 1 second for integration tests is documented in contracts/library-api.md. No edit needed._
- [x] CHK033 - Are the bounded-vs-unbounded mailbox trade-offs documented with an explicit v2 trigger condition (so the swap is not a surprise)? [Completeness, research.md §7] — _verified 2026-05-18: research §7 names the trade-off (drop/block/error policy implied by bounded), commits to unbounded with rationale (Trust + Liveness + FR-013 simplicity), and pins the v2 trigger ("when a real transport introduces backpressure") with both a `// FUTURE:` code-level marker and a matching Open Follow-ups entry. Triple-recorded so the swap cannot be silently rediscovered._
- [x] CHK034 - Is the project structure in plan.md §"Project Structure" consistent with the module imports implied by data-model.md §8's dependency graph? [Consistency, plan.md vs data-model.md §8] — _fixed 2026-05-18: data-model.md §8 now shows `error.rs` (matching plan.md's `src/error.rs`) as its own node, and `network.rs` now reflects the post-CHK022 surface (NetworkSender added). Errors moved out of the per-module exports and into the dedicated error.rs row, matching the file layout._

## Contracts Quality

- [x] CHK035 - Are CLI exit codes (0 / 1 / 2 / 64) each traceable back to a spec-level error scenario or user story? [Traceability, contracts/cli.md] — _documented 2026-05-18: contracts/cli.md's Exit-codes table now carries a "Spec trace" column. Code 2 cites US3 AS-2 + FR-012 (post-CHK023 symmetry); codes 0/1/64 are annotated as best-practice POSIX-convention exits with no spec-level scenario, which the user noted is implementation territory rather than spec gap._
- [x] CHK036 - Does the `Network::send` "drop + warn-log + Ok(())" branch in library-api.md exactly match FR-010's required wording, including the "warn-level structured log entry that names the unknown identifier" part? [Consistency, contracts/library-api.md vs Spec §FR-010] — _verified 2026-05-18 (now applies to `NetworkHandle::send` post-CHK022): all five FR-010 obligations (drop, warn-level, structured log, names the unknown id, no synchronous error to the caller) are reflected in contracts/library-api.md, with a more specific event shape in data-model.md §4. Consistent across all three layers._
- [x] CHK037 - Is the TOML schema's `deny_unknown_fields` decision (peer-list.toml.md §"Forward-compatibility") traced to a spec requirement, or is it a contract-only choice that should be flagged as such? [Traceability, contracts/peer-list.toml.md] — _flagged 2026-05-18: peer-list.toml.md's Forward-compatibility section now carries an explicit "Spec trace: deliberately none" annotation explaining the strict-parsing choice is a contract-level best-practice decision (both strict and lenient would satisfy FR-001 + US3 AS-2). Future schema changes are routed through ADRs per Principle III._
- [x] CHK038 - Are the library-api.md "Versioning" rules (e.g., adding `Message` variants is non-breaking under `#[non_exhaustive]`) consistent with how the spec talks about future message kinds? [Consistency, contracts/library-api.md §Versioning] — _verified 2026-05-18: three-way chain holds — spec (Key Entities + Assumptions "no protocol semantics beyond connectivity") signals future message kinds → data-model.md §2 marks `Message` with `#[non_exhaustive]` → library-api.md commits to non-breaking variant additions. research.md Open follow-ups also cross-references the hook. Nuance "external consumers only" is captured by library-api's "for consumers that match non-exhaustively"._
- [x] CHK039 - Are `PeerId` validation rules (non-empty UTF-8, no internal NULs) stated identically across data-model.md §1, contracts/library-api.md `PeerId`, and contracts/peer-list.toml.md? [Consistency] — _verified 2026-05-18: all three rules (non-empty, UTF-8, no internal NULs) are stated in all three layers; library-api.md additionally names the failure variants `PeerIdError::Empty` / `PeerIdError::ContainsNul`. Spec-side FR-009 stays type-shape-agnostic (talks about uniqueness, not character class), and post-CHK023 FR-012 preserves CLI ↔ TOML validation symmetry by reference to "the same identifier rules"._

## Cross-Artifact Consistency

- [x] CHK040 - Does FR-012's CLI surface ("`--self-id` and a config-path flag") exactly match the three-flag CLI in contracts/cli.md (which adds `--log-level`), or is `--log-level` undocumented in the spec? [Consistency, Spec §FR-012 vs contracts/cli.md] — _fixed 2026-05-18: FR-012 now lists all three flags, ties `--log-level` to the FR-006 + FR-010 logging requirements, and adds a spec-level constraint that the default level MUST surface FR-010's warn-level drop events without explicit configuration. Concrete default value (`info`) stays in contracts/cli.md._
- [x] CHK041 - Are all 13 FRs covered in data-model.md §9's cross-reference matrix without omission? [Coverage, data-model.md §9] — _verified + cleaned 2026-05-18: all 13 FRs (FR-001 through FR-013) are listed, no omissions. Three rows (FR-010, FR-011, FR-013) were also swept to replace pre-CHK022 `Network::send` / `InMemoryNetwork::send` references with the post-refactor `NetworkHandle::send`._
- [x] CHK042 - Is the Node's Drop semantics (`recv_task.abort()` in data-model.md §5) reflected anywhere in the spec, or is shutdown behaviour an implicit plan-level decision? [Gap, data-model.md §5 vs Spec] — _resolved 2026-05-18 as intentional non-elevation: the spec deliberately doesn't model lifetimes or shutdown in v1 (Liveness assumption explicitly carves out failure handling), and no US/AS/SC depends on shutdown ordering. `JoinHandle::abort` on Drop is an idiomatic Rust implementation detail; data-model.md §5 is its right home. Shutdown semantics earn a spec FR when failure handling arrives — recorded as a follow-up under CHK030's "Peer-set dynamics" and "Delivery semantics under failure" entries._
- [x] CHK043 - Does quickstart.md §3 accurately reflect that the two CLI binaries register on *separate* in-memory networks and do NOT exchange messages — i.e., is the "single-process scope" assumption visible to a contributor running the CLI demo? [Clarity, quickstart.md §3 vs Spec §Assumptions] — _verified 2026-05-18: quickstart.md §3 makes the three load-bearing points explicit (separate networks; "two CLI processes do NOT exchange messages"; cited cause = Single-process-scope assumption) and frames what the CLI demo *is* for (FR-001, US3 AS-1, AS-2) vs *not* for (cross-process pubsub). No correctness gap._

## Coverage & Edge-Case Gaps

- [x] CHK044 - Are concurrent `Node::new` constructions against the same `Arc<InMemoryNetwork>` specified to be race-free (registration is the only shared mutation point)? [Coverage, Gap] — _resolved 2026-05-18 as intentional contract-only commitment: library-api.md states "`register` MUST be safe to call concurrently from multiple async tasks" and data-model.md §4 shows the registry behind an async `RwLock`. The spec is silent (it doesn't model concurrency anywhere else, and US1–US3 all construct nodes sequentially). The right layer for the guarantee is the library contract; if a property test later races constructions, that test promotes the invariant naturally._
- [x] CHK045 - Are scenarios where `received_messages()` is called during active inbound traffic specified (snapshot atomicity)? [Edge Case, Gap] — _verified 2026-05-18: substantively resolved by CHK019. FR-006's "stable for the caller and unaffected by subsequent receptions" sentence is the snapshot-atomicity guarantee, echoed at the impl layer in data-model.md §5 (acquire mutex, clone vector, release) and library-api.md ("Returned values are clones"). Three-layer commitment._
- [x] CHK046 - Is the empty-peer-set Edge Case (Spec line 73) explicitly traced to an integration test in `tests/two_node_ping.rs` per quickstart.md §2 / US1 AS-3? [Traceability, Spec §Edge Cases vs quickstart.md] — _verified 2026-05-18: quickstart.md §2 lists the test `empty_peer_set_cannot_originate` in `tests/two_node_ping.rs`, which traces directly to both the Edge Case bullet (Spec line 73) and US1 AS-3. `/speckit-tasks` will materialise the test-authoring task with this name._
- [x] CHK047 - Is the malformed-config exit code (`2`) for US3 AS-2 specified anywhere in the spec, or only in contracts/cli.md? [Gap, Spec §US3 AS-2 vs contracts/cli.md] — _resolved 2026-05-18 as intentional layering: US3 AS-2 commits at the spec level ("fail with a clear, actionable error; do not start partial"); contracts/cli.md realises this at the POSIX layer with exit code `2`, and the post-CHK035 "Spec trace" column on the exit-codes table provides bidirectional traceability. Pinning `2` in the spec would over-constrain the binary (a future TUI / JSON-RPC surface might not use POSIX exit codes); the abstraction split is right._
- [x] CHK048 - Is the duplicate-id detection behaviour (research.md §11 says "may still surface it later") consistent with FR-009's "duplicate ids on the same network are not supported and need not be detected"? Are these statements operationally compatible? [Consistency, Spec §FR-009 vs research.md §11] — _verified 2026-05-18: the "may surface" wording actually sits in research.md §8 (registration timing), not §11. FR-009's "need not be detected" + §8's "may still surface" form a MAY-not-MUST relationship — fully compatible. `NetworkError::DuplicateRegistration` is pre-defined in data-model.md §7 (currently unused, kept available), and CHK030's expanded Open Follow-ups lists "Duplicate-id detection on register" with its v2+ trigger condition. Forward-compatible chain is in place._

## Notes for this pass

- Numbering: CHK017–CHK048 (32 new items), continuing globally from the 16 implicit items in the first pass above.
- Marker conventions: `[Gap]` = requirement absent; `[Clarity]` / `[Ambiguity]` = present but underspecified; `[Consistency]` = two sources disagree or risk doing so; `[Coverage]` = a scenario class is unaddressed; `[Traceability]` = the link between artifacts is implicit and should be made explicit; `[Measurability]` = SC item is not objectively checkable as written.
- Failure resolution: items failing review are addressed by editing the relevant artifact (spec, plan, research, data-model, contracts), or by opening an ADR / issue if the question is genuinely structural per Constitution Principle III.
- Out of scope here: implementation correctness (that's `/speckit-implement`'s output and the integration tests' job).

---

# Second Readiness Pass (2026-05-19)

**Purpose**: re-sweep all areas after the CHK017–CHK048 walkthrough to surface quality issues *newly introduced* by that iteration's 14 substantive edits. Focus: cross-artifact wording drift produced by amendments, implicit knowledge from the new architectural surface (NetworkHandle / logical-peer-identity / exactly-once-conditional), and measurability of just-added constraints.

**Audience**: reviewer / co-maintainer.

**Created**: 2026-05-19

**Scope**: 13 items spanning post-walkthrough artifact consistency, newly-introduced definitions, operator-choice gaps, cross-reference integrity, measurability of new constraints, new architectural surface discoverability, and forward maintenance of artifacts named in the spec.

Numbering continues globally from CHK048.

## Post-Walkthrough Consistency

- [x] CHK049 - Does data-model.md §4's `InMemoryNetwork` subsection (after the CHK022 NetworkHandle refactor) still reference any pre-refactor surface such as `Network::send` or `InMemoryNetwork::send`, or has every mention been swept to the post-refactor shape? [Consistency, data-model.md §4] — _fixed 2026-05-19: the Failure modes row in §4.2 attributed `send` to InMemoryNetwork directly; updated to `NetworkHandle::send` with explicit "whose dispatch is backed by this InMemoryNetwork's registry" framing. Other rows (Shape, Sharing, Spec note, code block) were already clean._
- [x] CHK050 - Does library-api.md's quoted phrase *"FR-006 'logical peer identity supplied by the network at delivery time'"* exactly match wording present in FR-006, or is it a paraphrase presented as a verbatim quote? [Consistency, contracts/library-api.md vs Spec §FR-006] — _fixed 2026-05-19: the phrase was a paraphrase masquerading as a verbatim quote (FR-006 mentions "logical peer identity" and "supplies it at delivery time" as separate statements, not strung together). Both library-api.md NetworkHandle send contract and data-model.md §4 NetworkHandle send contract now reference FR-006's "logical-peer-identity requirement" with a parenthetical paraphrase (no quote marks), removing the misleading grep-target._

## Newly Introduced Definitions

- [x] CHK051 - Is the term "logical peer identity" (introduced in FR-006 during CHK022) defined once authoritatively, or is it used across spec / data-model.md / contracts without a single anchor a reader can cite? [Clarity, Spec §FR-006] — _verified 2026-05-19: term is anchored exactly once (spec.md FR-006 with inline definition); every downstream use (data-model.md §4 + §9, contracts/library-api.md, research.md §12) cites FR-006 explicitly. No near-synonyms or redefinitions appear. Single-term-with-one-anchor pattern doesn't justify a glossary at v1._
- [x] CHK052 - Is the log-level ordering relationship (e.g. `info` ≤ `warn` ≤ `error`, with each lower level surfacing higher-severity events) documented anywhere so FR-012's "default level MUST surface FR-010 warn events" is verifiable from the artifacts alone? [Clarity, Spec §FR-012 vs contracts/cli.md] — _fixed 2026-05-19: contracts/cli.md's `--log-level` description now states the lower-bound-threshold rule explicitly and ties it inline to FR-012's constraint, so a reviewer can close the spec ↔ contract trace from artifacts alone._

## New Operator-Choice Gaps

- [x] CHK053 - What is the contract when an operator passes `--log-level error` (suppressing FR-010's warn-level drop events) — is this operator override permitted, or does FR-010 require visibility regardless of the flag? Currently the spec adds a default-level constraint (FR-012) without saying what an explicit override is allowed to do. [Gap, Spec §FR-010 vs §FR-012 vs contracts/cli.md] — _fixed 2026-05-19: FR-012 now states explicit operator overrides are honoured (a more restrictive level MAY suppress warn-drop events); the rationale that FR-010 governs **emission level** ("warn" because the event is neither informational nor erroneous) rather than human-delivery is made explicit. contracts/cli.md mirrors the wording. Consistent with the Trust assumption — no reinvention of log levels/filtering._

## Cross-Reference Integrity

- [x] CHK054 - Does FR-013's "see spec §Assumptions" reference cite a stable anchor — i.e., are the named assumptions (Trust, Liveness) called out by name, so wording shifts in §Assumptions don't silently break the cross-reference? [Traceability, Spec §FR-013 vs §Assumptions] — _verified 2026-05-19: dual-structure cite (named labels "Trust and Liveness assumptions" + paraphrased content "no peer failures, all peers up, no adversarial behaviour") is robust against reordering and partial renaming of the Assumptions section, and the paraphrase preserves meaning even if the cross-reference dangles._
- [x] CHK055 - Are the public URLs in research.md §12 (libp2p, Lighthouse, Substrate, tokio docs) stable canonical references that won't rot, or should they be pinned to a specific commit / tag / version so the ADR-0007 source material survives upstream restructurings? [Stability, research.md §12] — _flagged 2026-05-19: added a "Note on URL stability" annotation explaining the floating-reference choice is deliberate (URLs survey a pattern, not a specific algorithm), and instructing ADR 0007's author to re-walk + pin at authoring time. Pre-emptive pinning rejected because pinned links can themselves go stale (renamed types in older commits) while the pattern claim remains valid._

## Measurability of New Constraints

- [x] CHK056 - Does SC-005's "randomly generated from a recorded seed" specify *where* the seed should be recorded (in-test constant, fixture file, CI log) so a failure can be reproduced by a future reader without context-switching? [Clarity, Spec §SC-005] — _fixed 2026-05-19: SC-005 now requires the seed to appear in the test source (as a `const` or in a doc-comment), so reproduction is possible without external context. Keeps Engineering Standards' generic phrasing intact at the constitution level._
- [x] CHK057 - Does FR-013's "exactly-once delivery under Trust + Liveness" wording specify how a test could falsify the guarantee — i.e., what observable behaviour would demonstrate a violation (duplicate `ReceivedDelivery` entries with the same `(from, message)`? a missing `(from, message)` after N sends?) — so the property is objectively testable? [Measurability, Spec §FR-013] — _fixed 2026-05-19: FR-013 now names both violation modes (duplication: ≥2 `ReceivedDelivery` entries with matching `(from, message)` for a single send; loss: missing `(from, message)` after `await_delivery` resolved). Ties the falsifiability rule to existing artifacts (`received_messages()`, `await_delivery`, `ReceivedDelivery`) and acknowledges SC-005 as the at-scale check._

## New Architectural Surface

- [x] CHK058 - Is the `take_receiver()` crate-internal method described in any agent-discoverable place beyond data-model.md §4 (e.g., research.md §6 or §12, or library-api.md as the canonical `Node::new` consumption pattern)? Without redundancy, an implementer reading library-api alone could miss the take-once pattern. [Clarity, data-model.md §4] — _verified 2026-05-19: `take_receiver()` appears in data-model.md §4 (twice — impl block + prose), research.md §6 (Receive-side processing model), and research.md §12 (actor-handle decision). Deliberately absent from contracts/library-api.md because it's `pub(crate)` — keeping the public-API contract clean of crate-internal methods is the correct boundary._
- [x] CHK059 - Does library-api.md call out the actor-handle pattern's structural consequence — `NetworkHandle` is NOT `Clone` — as an explicit design trade-off, or is the non-Clone constraint only implicit (visible from the type's lack of `#[derive(Clone)]`)? Single-consumer recv discipline is the central reason; saying so up front avoids future "why can't I clone this?" friction. [Completeness, contracts/library-api.md] — _verified 2026-05-19: library-api.md's NetworkHandle Design-pattern paragraph states "The handle itself is **not** `Clone` — single-consumer recv discipline" with a one-step pointer to research.md §12 for the deeper rationale. Answer to "why can't I clone this?" is in the first document a public-API consumer reads._

## Forward Discoverability

- [x] CHK060 - Does each new entry in research.md's expanded Open Follow-ups (post-CHK030) carry a v2+ trigger condition (so it can be re-opened deterministically when the conditions arrive), or is at least one entry open-ended in a way that risks permanent deferral? [Completeness, research.md §"Open follow-ups"] — _fixed 2026-05-19: four entries (Logging, Topic/sequence/chain on Message, Peer discovery + dissemination, Peer-set dynamics) lacked concrete triggers. Each now carries an explicit "Trigger: …" phrase naming a detectable event (deployment artifact, non-Ping variant proposed, parent-project ADR landed, programmatic add/remove use case). All nine entries now have triggers — none can be permanently deferred without notice._
- [x] CHK061 - Does the spec acknowledge that `quickstart.md` (named as the canonical contributor entry point in SC-004 post-CHK025) must be kept in sync with future spec changes — i.e., is there a maintenance commitment between the two artifacts, or could spec amendments silently invalidate the quickstart over time? [Gap, Spec §SC-004 vs quickstart.md] — _fixed 2026-05-19: SC-004 now requires spec amendments affecting contributor-facing behaviour to land alongside corresponding `quickstart.md` updates in the same commit, with a SHOULD-reject directive for reviewers. Makes the maintenance link a process commitment with a concrete review gate._

## Notes for this pass

- Numbering: CHK049–CHK061 (13 new items), continuing globally from CHK048.
- Marker conventions: same as previous passes.
- Focus selection rationale: every item targets something the CHK017–CHK048 walkthrough either *introduced* (e.g., the logical-peer-identity term in CHK022, the exactly-once disclaimer in CHK024, the SC-004 quickstart anchor in CHK025) or *touched* (e.g., the InMemoryNetwork subsection that the CHK022 refactor flowed through). Items that were already verified or deferred in the prior passes are deliberately not re-asked.
- Failure resolution: same buckets as previous passes — Bucket 1 (real gap → edit), Bucket 2 (verify → tick), Bucket 3 (defer-with-rationale → tick).
- Out of scope: implementation correctness (the `/speckit-implement` step's domain).

