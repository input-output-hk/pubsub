# Phase 1 Data Model: Verifiable hash-gated connection-selection and bounded acceptance

Grounded in current types: `ConnectionStrategy`/`ConnectToAllCandidates` (`strategies/connection/`), `ConnectionAcceptanceStrategy`/`AcceptFromAllCandidates` (`strategies/acceptance/`), `UpstreamState{AwaitingAccept,Active}`, `ConnectionAction{Request,Accepted,Terminated}` (`message.rs`), `Effect{Send,Misbehaved}` + `apply`/`NodeState` (`state.rs`). This feature keeps ordered structures (`BTreeSet`/`BTreeMap`) on the candidate/subscription view so effect emission is deterministic, and keeps the strategy objects pure (`genesis`/`target_degree`/`c` as fields, interval an input). It realises the overlay mechanics of `docs/extensions/bucketed-pull.md` (ADR 0024/0025/0030); the incentive/chain layer of that doc is out of scope.

## 1. New / changed types

### 1.1 Shared edge predicate — `strategies::edge` (NEW module, ADR 0024/0030)

- `is_valid_edge(genesis, topic, requester, candidate, interval, buckets) -> bool` = `H(genesis, topic, requester, candidate, interval) mod buckets == 0`. `H` is SHA-256 over a domain-separated, length-prefixed canonical encoding — fixed and cross-machine stable, **not** `DefaultHasher`. Ordered `(requester, candidate)` (directional). `buckets <= 1` ⇒ always true (the connect-to-all floor).
- `bucket_count(candidates_len, target_degree) -> usize` = `max(1, round(candidates_len / target_degree))`.
- `accept_cap(target_degree, c) -> usize` = `⌈target_degree + c·√target_degree⌉`.
- Both seams consult this one module — the dial side to *select*, the accept side to *verify* — so they can never drift.

### 1.2 `ConnectionStrategy` (dial) — hash-gated impl; interval threaded (ADR 0024/0030)

- Trait `expected_upstream(&self, view: &NodeView) -> BTreeSet<(PeerId, TopicId)>`: the read-only node state is grouped into a `NodeView { subscriptions, candidates, downstream, interval }` (re-exported as `pubsub_node::NodeView`); the interval carried by the view is new (ADR 0030), and the node hands in the current candidate view and interval directly (no failed-set pre-filter). Inputs and output are ordered structures so effect emission is order-stable; the predicate itself is order-independent by construction.
- New `HashGatedConnection { genesis: u64, self_id: PeerId, target_degree: usize }`: per joined topic `T`, `B = bucket_count(|candidates_T|, target_degree)`, select each candidate `U` with `is_valid_edge(genesis, T, self_id, U, interval, B)`. Expected out-degree ≈ the fixed target (connection) degree `target_degree`; selects all when `B = 1` (`≤ ~target_degree` candidates). Pure; deterministic; no state carried.
- `ConnectToAllCandidates` unchanged; default (FR-010).

### 1.3 `ConnectionAcceptanceStrategy` (inbound) — reason-bearing return; verify + interval (ADR 0025/0030)

- `accepts -> bool` becomes `admit(&self, emitter: &PeerId, topic: &TopicId, view: &NodeView) -> Admission`, where `NodeView` groups the read-only node state `{subscriptions, candidates, downstream, interval}`.
- `enum Admission { Accept, RejectMembership, RejectIllegitimate, RejectOverCapacity }`.
- `VerifiableBoundedAcceptance { genesis, self_id, target_degree, cap_buffer }`: `RejectMembership` if not membership-valid; else `RejectIllegitimate` if the verifiable edge predicate `is_valid_edge(genesis, topic, emitter, self_id, interval, B)` fails (the acceptor verifies, same `B` as the dialer); else `RejectOverCapacity` if downstream-on-topic ≥ `OC = accept_cap(target_degree, cap_buffer)`; else `Accept`.
- `AcceptFromAllCandidates` maps onto `Accept`/`RejectMembership`; default (FR-010).

### 1.4 `ConnectionAction::Rejected { topic }` (ADR 0025)

- New variant (acceptor → dialer), same signed `PlainConnection`/`ConnectionMessage` envelope, sent via `Effect::Send`. No new `Effect`. Sent only on over-capacity of a legitimate request.

## 2. `NodeState` additions

This feature adds one field: **`interval: u64`** (default 0), folded from `Event::Heartbeat` (§3.1) so the acceptor verifies inbound requests against the current interval (FR-006). No failed-set and no rejection counter are maintained: the dialer's reaction to a `Rejected` is minimal (drop the matching pending upstream only), so retry/back-fill — which would need such state — is deferred to a future strategy family, out of scope for 005.

Existing `upstream`/`downstream`/`candidates`/`subscriptions` reused; `subscriptions: BTreeSet<TopicId>` and `candidates: BTreeMap<TopicId, BTreeSet<PeerId>>` supply the canonical order for deterministic effect emission (`downstream` stays a `HashSet` — acceptance only *counts* it per topic, so it is order-independent). Strategy objects stay pure (genesis / `target_degree` / `c` as construction fields, FR-014) and keep the current injection.

## 3. Transitions

### 3.1 `handle_heartbeat` — interval fold + dial (ADR 0030, replaces `handle_connection_setup`)

`Event::ConnectionSetup` is renamed `Event::Heartbeat { interval: u64 }` (an advancing 0-based counter, driver-fired, no wall-clock). `handle_heartbeat`:

1. Store `state.interval = interval` (so the acceptor verifies against it).
2. `expected = selection.expected_upstream(&view)` where `view: NodeView` groups `{subscriptions, candidates, downstream, interval}` (hash-gated → the predicate-valid subset per topic), over `candidates`.
3. Diff vs `upstream`: dial each expected pair not already held (insert `AwaitingAccept`, emit `Request`); never remove.

v1 fires **one** heartbeat (interval 0) on the readiness edge; periodic heartbeats + cross-interval rotation/teardown are deferred (FR-012).

### 3.2 `handle_connection_request` — verify + capacity + `Rejected`

1. `admit(emitter, topic, &view)` where `view: NodeView` groups `{subscriptions, candidates, downstream, interval}`.
2. `Accept` → idempotent `downstream` insert + reply `Accepted` (unchanged).
3. `RejectMembership` → silent logged drop `membership_validation_failed`.
4. `RejectIllegitimate` → silent logged drop `illegitimate_request` (predicate fails this interval — an adversary cannot force an edge).
5. `RejectOverCapacity` → logged drop `downstream_capacity_reached` + `Effect::Send(Rejected)`; no downstream entry.

### 3.3 `handle_connection_rejected` — dialer side

1. Matching `AwaitingAccept` upstream → remove it (so the dialer stops waiting for an `Accepted` that will never come). This is the **only** handling — no failed-set, no counter, no retry/back-fill (deferred to a future strategy family). The realized upstream degree may therefore settle below `target_degree`.
2. No matching pending entry → logged drop `unsolicited_reject`, no state change.

### 3.4 `handle_connection_accepted` — unchanged

- Activates the matching `AwaitingAccept` → `Active`.

## 4. Observability (getters, not logs)

- `upstream_connections()` / `downstream_connections()` (existing) → realized degree ≈ `target_degree`, verifiability, under-fill, the `OC` bound (SC-001/SC-004/SC-006/SC-007). These snapshots are the whole observability surface for this feature (FR-013); no rejection-count getter is exposed.
- All assertions read getters/snapshots; `message_dropped`/severance logs stay operator-only.

## 5. Validation rules (from requirements)

- Predicate selection: per joined topic, exactly the candidates for which `is_valid_edge(genesis, self_id, candidate, topic, interval, B) == true`, `B = bucket_count(|candidates_T|, target_degree)` (FR-001); `B = 1` ⇒ connect-to-all for small topics (FR-003).
- Verifiability: identical `(genesis, requester, candidate, topic, interval)` gives the acceptor the same predicate result as the dialer — the edge is verifiable (FR-002, SC-002).
- Determinism / order-independence: the fixed SHA-256 hash + modulus make selection a pure function of the set; ordered structures supply order-stable effect emission (FR-002/FR-014, SC-001).
- Fixed `target_degree`: applied uniformly for the run (not derived from network size); expected out-degree ≈ `target_degree` (FR-003, SC-004).
- Acceptance: additive gate — predicate ∧ topic-registered ∧ shared-interest ∧ under `OC = ⌈target_degree + c·√target_degree⌉` (FR-007); no node exceeds `OC` (SC-004).
- Silent drop vs explicit `Rejected`: membership OR predicate failure is a silent drop; only over-capacity of a legitimate request sends `Rejected` (FR-008). Not misbehaviour; no severance/`Terminated`.
- No wall-clock / no entropy at decision time (the predicate is a pure hash; the interval is an input, FR-006); strategy objects pure with genesis/`target_degree`/`c` as fields (FR-014).
- Under-fill just settles: after rejections the realized degree may be below `target_degree`, with no error and no back-fill (FR-009, SC-007).

## 6. Upstream-entry lifecycle

```text
(absent) --select(edge predicate over candidates)--> AwaitingAccept --Accepted--> Active
                                                       |                            |
                                             Rejected (over-capacity)            Terminated
                                                       v                            v
                                                   removed                       removed
                              (no retry/back-fill; degree may under-fill below target_degree —
                               re-forming deferred to a future strategy family + rotation layer)
```
