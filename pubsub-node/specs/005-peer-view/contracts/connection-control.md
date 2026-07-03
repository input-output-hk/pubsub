# Contract: connection-control protocol addition

Extends `ConnectionAction` (`Request` / `Accepted` / `Terminated`). The new action rides the existing signed envelope and reduces to `Effect::Send` (no new effect).

## New action: `Rejected { topic }` (ADR 0025)

- **Direction**: acceptor → dialer.
- **Emitted when**: an otherwise-legitimate `Request` is refused for over-capacity (`Admission::RejectOverCapacity`) — the edge predicate held, topic is registered, both peers are members, but the acceptor is at its per-topic cap `OC = ⌈target_degree + c·√target_degree⌉`. NOT emitted for a membership failure (`RejectMembership`) or an edge-predicate failure (`RejectIllegitimate`): both stay silent drops (a reply would leak membership / the fact a slot exists to a non-legitimate requester).
- **Distinct from**: `Terminated` (tears down an *established* link) and `Effect::Misbehaved` (severance on signature failure). A rejection is a normal capacity outcome — **not** misbehaviour (FR-008). "Rejected" is always this active signal — there is no timeout/no-response notion in this feature.

## Acceptor handling — `handle_connection_request` (amended)

The acceptor recomputes the shared edge predicate (`strategies::edge::is_valid_edge`) against the node's current interval, so it **verifies** the request rather than trusting the dialer.

| Decision | Effect |
|----------|--------|
| `Accept` (predicate holds ∧ registered ∧ shared interest ∧ under `OC`) | idempotent `downstream` insert; reply `Accepted` |
| `RejectMembership` (topic unregistered / non-member) | logged drop `membership_validation_failed`; no reply |
| `RejectIllegitimate` (edge predicate fails this interval) | logged drop `illegitimate_request`; no reply |
| `RejectOverCapacity` (legitimate but at/over `OC`) | logged drop `downstream_capacity_reached`; `Effect::Send(Rejected)`; no downstream entry |

`RejectMembership` and `RejectIllegitimate` are distinct log causes but share the same silent-drop effect (no reply).

## Dialer handling — `handle_connection_rejected(emitter, topic)` (new)

| Precondition | Action |
|--------------|--------|
| `upstream[(emitter,topic)] == AwaitingAccept` | remove the entry (so the dialer stops waiting for an `Accepted`). This is the **only** action — no failed-set, no counter, no retry/back-fill. |
| no matching pending entry | logged drop `unsolicited_reject`; no state change |

## Invariants

- A `Rejected` never creates/mutates a `downstream` entry on the acceptor.
- Because both peers compute the same predicate over the same interval, an honest dialer only requests predicate-valid peers, so it never provokes a silent drop — honest nodes see only `Accepted` or an over-capacity `Rejected`.
- Handling a `Rejected` only removes the matching pending upstream; no other state changes, and the peer is neither marked nor re-dialed within 005. The realized upstream degree may consequently settle below `target_degree`; re-forming connections (retry-to-a-minimum / back-fill) is deferred to a future strategy family + the heartbeat-rotation layer, out of scope here.
- The exchange is signed/verified on the existing control path (signature failure handled as today, before action dispatch).
