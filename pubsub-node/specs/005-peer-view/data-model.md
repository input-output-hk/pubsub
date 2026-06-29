# Phase 1 Data Model: Seeded bounded connection-selection and acceptance strategies

Grounded in current types: `ConnectionStrategy`/`ConnectToAllCandidates` (`connection.rs`), `ConnectionAcceptanceStrategy`/`AcceptFromAllCandidates` (`acceptance.rs`), `UpstreamState{AwaitingAccept,Active}`, `ConnectionAction{Request,Accepted,Terminated}` (`message.rs`), `Effect{Send,Misbehaved}` + `apply`/`NodeState` (`state.rs`). This feature applies **ordered structures (`BTreeSet`/`BTreeMap`) to the state it introduces/touches** itself and keeps strategy objects pure; it retains the current strategy injection (does not depend on the strategies-as-`apply`-arguments refactor) — see research R6.

## 1. New / changed types

### 1.1 `ConnectionStrategy` (dial) — bounded impl, signature stable (ADR 0024)

- Trait `expected_upstream(subscriptions, candidates) -> Set<(PeerId, TopicId)>` unchanged in shape; the node hands in the **viable** candidate view (candidates minus failed).
- New `SeededBoundedSelection { seed: u64, self_id: PeerId, out_degree: usize }`: per joined topic, rank candidates by stable SHA-256 of `(seed, self_id, topic, candidate_id)`, take lowest `out_degree`, tie-break on `candidate_id`. Selects all when candidates ≤ `out_degree`. Pure; deterministic; no RNG.
- `ConnectToAllCandidates` unchanged; default (FR-013).

### 1.2 `ConnectionAcceptanceStrategy` (inbound) — reason-bearing return (ADR 0025)

- `accepts -> bool` becomes `admit(emitter, topic, subscriptions, candidates, downstream) -> Admission`.
- `enum Admission { Accept, RejectMembership, RejectOverCapacity }`.
- `BoundedAcceptance { in_degree: usize }`: `RejectMembership` if not membership-valid; else `RejectOverCapacity` if the topic's downstream count ≥ `in_degree`; else `Accept`.
- `AcceptFromAllCandidates` maps onto `Accept`/`RejectMembership`; default (FR-013).

### 1.3 `ConnectionAction::Rejected { topic }` (ADR 0025)

- New variant (acceptor → dialer), same signed `PlainConnection`/`ConnectionMessage` envelope, sent via `Effect::Send`. No new `Effect`.

## 2. `NodeState` additions (ordered structures per the refactor)

| Field | Type | Purpose |
|-------|------|---------|
| `failed_upstream` | ordered set of `(PeerId, TopicId)` (`BTreeSet`) | Peers a dial was rejected by; excluded from the viable view before selection (R3). **Sticky** — populated on `Rejected`, never reset within the run. |
| `rejections_received` | counter (per topic or aggregate) | Observability for the explicit-rejection count (FR-016, SC-007) via a getter. |

Existing `upstream`/`downstream`/`candidates`/`subscriptions` reused (migrated to ordered types as 005 touches them). Strategy objects stay pure (seed/bounds as construction fields, FR-018) and keep the current injection; if/when the parallel refactor moves strategies to `apply` arguments, they migrate unchanged.

## 3. Transitions

### 3.1 `handle_connection_setup` — bounded + back-fill aware

1. viable = `candidates` minus `failed_upstream`.
2. `expected = selection.expected_upstream(subscriptions, viable)` (bounded → top-`out_degree`/topic).
3. Diff vs `upstream`: dial each expected pair not already held (insert `AwaitingAccept`, emit `Request`); never remove. Re-invocation after a rejection back-fills the freed slot with the next-ranked candidate.

### 3.2 `handle_connection_request` — capacity + `Rejected`

1. `admit(emitter, topic, subscriptions, candidates, downstream_on_topic)`.
2. `Accept` → idempotent `downstream` insert + reply `Accepted` (unchanged).
3. `RejectMembership` → silent logged drop `membership_validation_failed` (unchanged).
4. `RejectOverCapacity` → logged drop `downstream_capacity_reached` + `Effect::Send(Rejected)`; no downstream entry; `rejections_received` is a dialer-side counter, so nothing incremented here.

### 3.3 `handle_connection_rejected` — NEW (dialer side)

1. Matching `AwaitingAccept` upstream → remove it, insert `(peer, topic)` into `failed_upstream`, increment `rejections_received`. (Back-fill happens on the next `ConnectionSetup`.)
2. No matching pending entry → logged drop `unsolicited_reject`, no state change.

### 3.4 `handle_connection_accepted` — unchanged

- Activates the matching `AwaitingAccept` → `Active`.

## 4. Observability (getters, not logs)

- `upstream_connections()` / `downstream_connections()` (existing) → realized degree, convergence, under-fill (SC-002/SC-006).
- New getter: explicit-rejection count (`rejections_received`) (FR-016, SC-007).
- All assertions read getters/snapshots; `message_dropped`/severance logs stay operator-only.

## 5. Validation rules (from requirements)

- Bound is a ceiling: select all when candidates ≤ bound (FR-002); never exceed bound per topic (SC-002).
- Determinism: identical `(seed, self_id, topic, viable)` → identical selection, iteration-order-independent (FR-003); stable digest + `candidate_id` tie-break (FR-008); ordered structures (FR-017).
- Failed set is sticky for the run; "rejected" = explicit over-capacity only, no timeout (Clarifications; FR-014).
- Under-fill on exhaustion is terminal + observable (FR-015).
- Rejection is not misbehaviour; no severance/`Terminated` (FR-011).
- No wall-clock / no RNG in any transition (FR-009); strategy objects pure with seed/bounds as construction fields (FR-018).

## 6. Upstream-entry lifecycle

```text
(absent) --select(top-k over viable)--> AwaitingAccept --Accepted--> Active
                                            |                            |
                                      Rejected (over-capacity)        Terminated
                                            v                            v
                              removed + failed_upstream (sticky)       removed
                                            |
                          next ConnectionSetup re-selects next-ranked (back-fill)
```
