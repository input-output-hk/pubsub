//! The bounded-only inbound-acceptance policy: [`BoundedAcceptance`]
//! (one-dimensional baseline, ADR 0031).

use super::{admit_prelude, Admission, ConnectionAcceptanceStrategy};
use crate::connection_state::LinkRole;
use crate::peer::PeerId;
use crate::strategies::edge::accept_cap;
use crate::strategies::view::NodeView;
use crate::topic::TopicId;

/// Accept a verified `Request` iff it is membership-valid and the node is under
/// its per-topic downstream cap `OC = ⌈degree + c·√degree⌉` — the
/// **cap without the hash gate**, isolating the capacity dimension for the
/// empirical baseline experiments (ADR 0031).
///
/// Never returns `RejectIllegitimate` (no edge predicate is consulted); an
/// over-capacity refusal is the explicit `Rejected`, exactly as in the compound
/// [`HashGatedBoundedAcceptance`](super::HashGatedBoundedAcceptance).
pub struct BoundedAcceptance {
    degree: usize,
    cap_buffer: usize,
    role: LinkRole,
}

impl BoundedAcceptance {
    /// Build the policy from already-parsed inputs (`cap_buffer` is the `c` in
    /// `OC = ⌈degree + c·√degree⌉`; `degree` is the serving seam's target degree
    /// — `relay_degree` or `publish_degree`). Serves the `Relay` slot by
    /// default; retarget with [`for_role`](Self::for_role).
    #[must_use]
    pub fn new(degree: usize, cap_buffer: usize) -> Self {
        Self {
            degree,
            cap_buffer,
            role: LinkRole::Relay,
        }
    }

    /// Retarget the policy at a link role's acceptance slot: the prelude scan
    /// and the cap count that role's inbound links (ADR 0033).
    #[must_use]
    pub fn for_role(mut self, role: LinkRole) -> Self {
        self.role = role;
        self
    }
}

impl ConnectionAcceptanceStrategy for BoundedAcceptance {
    fn admit(&self, emitter: &PeerId, topic: &TopicId, view: &NodeView<'_>) -> Admission {
        let accepted_on_topic = match admit_prelude(self.role, emitter, topic, view) {
            Ok(count) => count,
            Err(decision) => return decision,
        };
        let cap = accept_cap(self.degree, self.cap_buffer);
        if accepted_on_topic >= cap {
            Admission::RejectOverCapacity
        } else {
            Admission::Accept
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BoundedAcceptance;
    use crate::connection_state::LinkStore;
    use crate::strategies::acceptance::{Admission, ConnectionAcceptanceStrategy};
    use crate::strategies::test_support::{
        candidates, downstream, peer, subscriptions, topic, view,
    };

    // Membership failure takes precedence and is a silent RejectMembership.
    #[test]
    fn membership_invalid_is_rejected() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t2", &["a"])]);
        let down = LinkStore::new();
        let got = BoundedAcceptance::new(1, 3).admit(
            &peer("a"),
            &topic("t2"), // not subscribed
            &view(&subs, &cands, &down),
        );
        assert_eq!(got, Admission::RejectMembership);
    }

    // No hash gate: EVERY membership-valid request is accepted below cap —
    // including ones the compound policy's edge predicate would refuse.
    #[test]
    fn any_member_accepts_below_cap_and_rejects_at_cap() {
        let t = topic("t1");
        let names = ["a", "b", "c", "d", "e", "f"];
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &names)]);
        let policy = BoundedAcceptance::new(1, 3); // relay_degree=1 ⇒ cap 4

        // Below cap (3 held) ⇒ every member accepted, no predicate consulted.
        let below = downstream(&[("x", "t1"), ("y", "t1"), ("z", "t1")]);
        for n in names {
            assert_eq!(
                policy.admit(&peer(n), &t, &view(&subs, &cands, &below)),
                Admission::Accept,
                "member {n} must be accepted without an edge predicate",
            );
        }

        // At cap (4 held, none the requester) ⇒ RejectOverCapacity.
        let at = downstream(&[("w", "t1"), ("x", "t1"), ("y", "t1"), ("z", "t1")]);
        assert_eq!(
            policy.admit(&peer("a"), &t, &view(&subs, &cands, &at)),
            Admission::RejectOverCapacity,
        );
    }

    // The shared prelude: a re-dial of an already-held downstream re-Accepts
    // even at cap (the half-open-link repair, 005 FR-013).
    #[test]
    fn already_downstream_peer_is_reaccepted_at_cap() {
        let t = topic("t1");
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &["a"])]);
        let policy = BoundedAcceptance::new(1, 3);
        let at_cap_with_a = downstream(&[("a", "t1"), ("x", "t1"), ("y", "t1"), ("z", "t1")]);
        assert_eq!(
            policy.admit(&peer("a"), &t, &view(&subs, &cands, &at_cap_with_a)),
            Admission::Accept,
        );
    }
}
