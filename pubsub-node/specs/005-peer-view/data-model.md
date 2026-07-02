# Phase 1 Data Model: Seeded bounded connection-selection and acceptance strategies

Grounded in current types: `ConnectionStrategy`/`ConnectToAllCandidates` (`connection.rs`), `ConnectionAcceptanceStrategy`/`AcceptFromAllCandidates` (`acceptance.rs`), `UpstreamState{AwaitingAccept,Active}`, `ConnectionAction{Request,Accepted,Terminated}` (`message.rs`), `Effect{Send,Misbehaved}` + `apply`/`NodeState` (`state.rs`). This feature applies **ordered structures (`BTreeSet`/`BTreeMap`) to the state it introduces/touches** itself and keeps strategy objects pure; it retains the current strategy injection (does not depend on the strategies-as-`apply`-arguments refactor) — see research R6.

## 1. New / changed types

### 1.1 `ConnectionStrategy` (dial) — bounded impl, signature stable (ADR 0024)

- Trait `expected_upstream(subscriptions, candidates) -> Set<(PeerId, TopicId)>` unchanged in shape; the node hands in the current candidate view directly (no failed-set pre-filter).
- New `SeededBoundedConnection { seed: u64, self_id: PeerId, upstream_degree: usize }`: per joined topic, rank candidates by stable SHA-256 of `(seed, self_id, topic, candidate_id)`, take lowest `upstream_degree`, tie-break on `candidate_id`. Selects all when candidates ≤ `upstream_degree`. Pure; deterministic; no RNG.
- `ConnectToAllCandidates` unchanged; default (FR-013).

### 1.2 `ConnectionAcceptanceStrategy` (inbound) — reason-bearing return (ADR 0025)

- `accepts -> bool` becomes `admit(emitter, topic, subscriptions, candidates, downstream) -> Admission`.
- `enum Admission { Accept, RejectMembership, RejectOverCapacity }`.
- `BoundedAcceptance { downstream_degree: usize }`: `RejectMembership` if not membership-valid; else `RejectOverCapacity` if the topic's downstream count ≥ `downstream_degree`; else `Accept`.
- `AcceptFromAllCandidates` maps onto `Accept`/`RejectMembership`; default (FR-013).

### 1.3 `ConnectionAction::Rejected { topic }` (ADR 0025)

- New variant (acceptor → dialer), same signed `PlainConnection`/`ConnectionMessage` envelope, sent via `Effect::Send`. No new `Effect`.

## 2. `NodeState` additions (ordered structures per the refactor)

This feature adds **no new persistent ordered state** to `NodeState` (FR-017). The earlier `failed_upstream` set and `rejections_received` counter were **removed** in the PR-73 simplification: the dialer's reaction to a `Rejected` is now minimal (drop the matching pending upstream only), so no failed-set or rejection counter is maintained. Retry-to-a-minimum (back-fill) — which would have needed such state — is deferred to a future strategy family (`BackfillingSeededBoundedConnection` / `RetryingSeededBoundedConnection`), out of scope for 005.

Existing `upstream`/`downstream`/`candidates`/`subscriptions` reused (migrated to ordered types as 005 touches them). Strategy objects stay pure (seed/bounds as construction fields, FR-018) and keep the current injection; if/when the parallel refactor moves strategies to `apply` arguments, they migrate unchanged.

## 3. Transitions

### 3.1 `handle_connection_setup` — bounded

1. `expected = selection.expected_upstream(subscriptions, candidates)` (bounded → top-`upstream_degree`/topic), selected straight over `candidates`.
2. Diff vs `upstream`: dial each expected pair not already held (insert `AwaitingAccept`, emit `Request`); never remove.

### 3.2 `handle_connection_request` — capacity + `Rejected`

1. `admit(emitter, topic, subscriptions, candidates, downstream_on_topic)`.
2. `Accept` → idempotent `downstream` insert + reply `Accepted` (unchanged).
3. `RejectMembership` → silent logged drop `membership_validation_failed` (unchanged).
4. `RejectOverCapacity` → logged drop `downstream_capacity_reached` + `Effect::Send(Rejected)`; no downstream entry.

### 3.3 `handle_connection_rejected` — NEW (dialer side)

1. Matching `AwaitingAccept` upstream → remove it (so the dialer stops waiting for an `Accepted` that will never come). This is the **only** handling — no failed-set, no counter, no retry/back-fill (deferred to a future strategy family). The realized upstream degree may therefore settle below target.
2. No matching pending entry → logged drop `unsolicited_reject`, no state change.

### 3.4 `handle_connection_accepted` — unchanged

- Activates the matching `AwaitingAccept` → `Active`.

## 4. Observability (getters, not logs)

- `upstream_connections()` / `downstream_connections()` (existing) → realized degree, convergence, under-fill (SC-002/SC-006). These snapshots are the whole observability surface for this feature (FR-016); no rejection-count getter is exposed.
- All assertions read getters/snapshots; `message_dropped`/severance logs stay operator-only.

## 5. Validation rules (from requirements)

- Bound is a ceiling: select all when candidates ≤ bound (FR-002); never exceed bound per topic (SC-002).
- Determinism: identical `(seed, self_id, topic, candidates)` → identical selection, iteration-order-independent (FR-003); stable digest + `candidate_id` tie-break (FR-008); ordered structures (FR-017).
- "Rejected" = explicit over-capacity only, no timeout (Clarifications); on receipt the dialer drops the matching pending upstream and does nothing further — no retry/back-fill (deferred to a future strategy family) (FR-014).
- Under-fill just settles: after rejections the realized degree may be below the bound, with no error and no back-fill (FR-015).
- Rejection is not misbehaviour; no severance/`Terminated` (FR-011).
- No wall-clock / no RNG in any transition (FR-009); strategy objects pure with seed/bounds as construction fields (FR-018).

## 6. Upstream-entry lifecycle

```text
(absent) --select(top-k over candidates)--> AwaitingAccept --Accepted--> Active
                                              |                            |
                                        Rejected (over-capacity)        Terminated
                                              v                            v
                                          removed                       removed
                              (no retry/back-fill; degree may under-fill —
                               re-forming deferred to a future strategy family)
```
