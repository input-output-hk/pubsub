# Contract: connection-control protocol addition

Extends `ConnectionAction` (`Request` / `Accepted` / `Terminated`). The new action rides the existing signed envelope and reduces to `Effect::Send` (no new effect).

## New action: `Rejected { topic }` (ADR 0025)

- **Direction**: acceptor → dialer.
- **Emitted when**: a verified `Request` is refused for over-capacity (`Admission::RejectOverCapacity`). NOT emitted for membership failure (stays a silent drop — does not leak membership to non-members).
- **Distinct from**: `Terminated` (tears down an *established* link) and `Effect::Misbehaved` (severance on signature failure). A rejection is a normal capacity outcome — **not** misbehaviour (FR-011). "Rejected" is always this active signal — there is no timeout/no-response notion in this feature.

## Acceptor handling — `handle_connection_request` (amended)

| Decision | Effect |
|----------|--------|
| `Accept` | idempotent `downstream` insert; reply `Accepted` |
| `RejectMembership` | logged drop `membership_validation_failed`; no reply |
| `RejectOverCapacity` | logged drop `downstream_capacity_reached`; `Effect::Send(Rejected)`; no downstream entry |

## Dialer handling — `handle_connection_rejected(emitter, topic)` (new)

| Precondition | Action |
|--------------|--------|
| `upstream[(emitter,topic)] == AwaitingAccept` | remove the entry (so the dialer stops waiting for an `Accepted`). This is the **only** action — no failed-set, no counter, no retry/back-fill. |
| no matching pending entry | logged drop `unsolicited_reject`; no state change |

## Invariants

- A `Rejected` never creates/mutates a `downstream` entry on the acceptor.
- Handling a `Rejected` only removes the matching pending upstream; no other state changes, and the peer is neither marked nor re-dialed within 005. The realized upstream degree may consequently settle below target; re-forming connections (retry-to-a-minimum / back-fill) is deferred to a future strategy family (`BackfillingSeededBoundedConnection` / `RetryingSeededBoundedConnection`), out of scope here.
- The exchange is signed/verified on the existing control path (signature failure handled as today, before action dispatch).
