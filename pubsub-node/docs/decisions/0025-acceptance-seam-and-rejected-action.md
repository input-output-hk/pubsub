# 0025 — Acceptance-seam evolution and the `Rejected` connection action

**Status**: Accepted. **Amended by ADR 0032/0033** (feature 015): `Rejected` (like every control action) carries the link role, and the prelude's downstream scan became role-scoped over the link store. `Admission` and the silent-drop/`Rejected` split stand.

**Context**: Feature 005 (US2) bounds a node's **inbound** degree. The v1 acceptance seam returned a bare `bool` (`accepts -> bool`) and the handler dropped a refused request silently. To bound the downstream degree and refuse over capacity, the handler must (a) see the node's current downstream so the policy can count, and (b) distinguish a *membership* failure (a silent drop — must not leak membership to a non-member) from an *over-capacity* refusal (which must tell the dialer so it can stop awaiting an acceptance). A bare `bool` cannot carry that distinction.

The connection-seam doc note claimed degree caps would "slot in behind this trait without a signature change." This feature finds that **false** for the acceptance side — surfaced here per Constitution Principle IV rather than worked around silently.

## Decision

1. **Reason-bearing return.** Replace `accepts(...) -> bool` with `admit(...) -> Admission`, where `Admission { Accept, RejectMembership, RejectIllegitimate, RejectOverCapacity }`. The method also takes the node's current `downstream` set (to count capacity) and the **current interval** (to recompute the edge predicate). (`RejectIllegitimate` added at the bucketed-pull redesign — the request fails the verifiable edge predicate for this interval; see ADR 0024.)
2. **New impl** (bucketed-pull redesign). `VerifiableBoundedAcceptance { genesis, self_id, target_degree, cap_c }` — replaces `BoundedAcceptance`. On a verified `Request` from `requester` on `topic` at `interval`:
   - not membership-valid (topic registered ∧ shared interest) → `RejectMembership`;
   - else `H(genesis, topic, requester, self_id, interval) mod B != 0` (`B = max(1, round(|candidates_on_topic|/target_degree))`, the same predicate the dialer used, `strategies::edge::is_valid_edge`) → `RejectIllegitimate` — the acceptor **verifies** the request; an adversary cannot force an edge the hash does not allow;
   - else downstream-on-topic ≥ `OC = ⌈target_degree + cap_c·√target_degree⌉` → `RejectOverCapacity`;
   - else `Accept`.
   `AcceptFromAllCandidates` maps its old logic onto `Accept`/`RejectMembership` and never refuses for capacity or predicate.
3. **New control action.** `ConnectionAction::Rejected { topic }` (acceptor → dialer, wire tag `0x03`), emitted **only** on `RejectOverCapacity`. Distinct from `Terminated` and from a misbehaviour severance — a rejection is a normal capacity outcome, **not** misbehaviour. `RejectMembership` **and `RejectIllegitimate` stay silent drops** (no reply — leaking nothing to a non-member or an adversary; distinct log causes `membership_validation_failed` / `illegitimate_request`). Honest dialers only request predicate-valid peers (both sides compute the same predicate), so they never hit the silent-drop path — they see only `Accepted` or over-capacity `Rejected`.
4. **Dialer handling (minimal, no back-fill).** `handle_connection_rejected` removes the matching `AwaitingAccept` upstream so the dialer stops awaiting an acceptance — that is the **only** handling. No failed-peer set, no counter, no back-fill; realized degree may under-fill; re-forming is deferred to the heartbeat-rotation layer + a future retry strategy family. A `Rejected` with no matching pending entry is a logged drop (`unsolicited_reject`).
5. **Config selector.** A case-insensitive `AcceptanceStrategyKind` (`accept-from-all` / `verifiable-bounded`) with a unique per-strategy byte-string tag, parsed at the edge; the bounded kind takes the genesis nonce + fixed `target_degree` (+ buffer `c`) — no explicit downstream-degree parameter (the cap is `⌈target_degree + c·√target_degree⌉`).

## Consequences

- The trait signature change ripples to `AcceptFromAllCandidates`, the `handle_connection_request` call site, and the test call sites — all updated in this change.
- "Rejected" is always an explicit over-capacity signal; there is no timeout/no-response notion (Clarifications: the controlled, lossless substrate answers every dial).
- The dialer's reaction is minimal (drop the pending upstream); realized upstream degree may under-fill, which is the accepted no-retry baseline (FR-014/FR-015).

## Semantics of `Rejected` (scope boundary)

A `Rejected` is a **positive liveness + capacity signal**: it means the candidate is *alive and at its per-topic out-degree cap* (its downstream count reached its per-topic cap), not that it is unreachable. In the in-memory substrate every dial is answered, so today `Rejected` is the *only* non-accept outcome a dialer observes; a true offline/unreachable candidate is unmodelled (there is no timeout — see Consequences).

This feature does **not** act on that signal beyond dropping the pending upstream. Using it to re-rank or filter candidates across intervals — treating an alive-but-full peer as a valid *future* candidate (deprioritized/retryable rather than dropped) — together with **offline detection** (a timeout that hard-filters genuinely unreachable candidates over a real/faulty transport), belongs to the future dynamic-connection-transitions / experiment layer and the retry/back-fill strategy family. Neither is in 005's scope. See [[N-029]].

> An earlier revision of this feature added a sticky `failed_upstream` set + `ConnectionSetup`-driven back-fill on the dialer side; both were removed in the PR-73 simplification (spec Clarifications, Session 2026-07-02) to start from the no-retry baseline.

## Alternatives rejected

- **Keep `bool`, move the capacity check into the handler** — splits the acceptance *policy* across the strategy and the handler, defeating the seam.
- **Overload `Terminated` for rejection** — conflates a never-established refusal with tearing down a live link, and muddies metrics.
- **In-feature back-fill (sticky failed-set + `ConnectionSetup`-driven re-dial)** — an earlier revision added it; removed for simplicity so the no-retry baseline is observed first. Retry/back-fill is a separate future strategy family (ADR 0024).
