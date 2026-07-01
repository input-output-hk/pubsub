# 0025 — Acceptance-seam evolution and the `Rejected` connection action

**Status**: Accepted

**Context**: Feature 005 (US2) bounds a node's **inbound** degree. The v1 acceptance seam returned a bare `bool` (`accepts -> bool`) and the handler dropped a refused request silently. To bound the downstream degree and refuse over capacity, the handler must (a) see the node's current downstream so the policy can count, and (b) distinguish a *membership* failure (a silent drop — must not leak membership to a non-member) from an *over-capacity* refusal (which must tell the dialer so it can back-fill). A bare `bool` cannot carry that distinction.

The connection-seam doc note claimed degree caps would "slot in behind this trait without a signature change." This feature finds that **false** for the acceptance side — surfaced here per Constitution Principle IV rather than worked around silently.

## Decision

1. **Reason-bearing return.** Replace `accepts(...) -> bool` with `admit(...) -> Admission`, where `Admission { Accept, RejectMembership, RejectOverCapacity }`. The method also takes the node's current `downstream` set so a policy can enforce a per-topic downstream degree bound. (Type named `Admission` — admission control — chosen over `AcceptanceDecision`/`ConnectionResult`.)
2. **New impl.** `BoundedAcceptance { downstream_degree }`: `RejectMembership` if not membership-valid; else `RejectOverCapacity` once the topic's downstream count reaches `downstream_degree`; else `Accept`. `AcceptFromAllCandidates` maps its old logic onto `Accept`/`RejectMembership` and never refuses for capacity.
3. **New control action.** `ConnectionAction::Rejected { topic }` (acceptor → dialer, wire tag `0x03`), emitted on `RejectOverCapacity`. Distinct from `Terminated` (tears down an *established* link) and from a misbehaviour severance — a rejection is a normal capacity outcome and is **not** treated as misbehaviour. `RejectMembership` stays a silent drop (no reply).
4. **Dialer back-fill.** `handle_connection_rejected` removes the matching `AwaitingAccept` upstream, inserts the peer into the **sticky** `failed_upstream` set (never re-dialed for that topic this run), and increments a `rejections_received` counter (exposed via a getter — the observability surface, asserted through state, not logs). A subsequent `ConnectionSetup` re-invocation selects over the viable view (candidates minus failed) and so back-fills the next-ranked candidate — no new round/timer event. A `Rejected` with no matching pending entry is a logged drop (`unsolicited_reject`).
5. **Config selector.** Acceptance strategy selection mirrors the connection side: a case-insensitive `AcceptanceStrategyKind` (`accept-from-all` / `bounded`) with a unique per-strategy byte-string tag, parsed at the edge (`--acceptance-strategy` + `--downstream-degree`).

## Consequences

- The trait signature change ripples to `AcceptFromAllCandidates`, the `handle_connection_request` call site, and the test call sites — all updated in this change.
- "Rejected" is always an explicit over-capacity signal; there is no timeout/no-response notion (Clarifications: the controlled, lossless substrate answers every dial).
- `rejections_received` gives the rejection-rate observability (FR-016/SC-007); back-fill keeps realized upstream degree near target despite refusals (FR-014).

## Alternatives rejected

- **Keep `bool`, move the capacity check into the handler** — splits the acceptance *policy* across the strategy and the handler, defeating the seam.
- **Overload `Terminated` for rejection** — conflates a never-established refusal with tearing down a live link, and muddies metrics.
- **A new round/tick event to drive back-fill** — rejected; re-dial is `ConnectionSetup` re-invocation (ADR 0024).
