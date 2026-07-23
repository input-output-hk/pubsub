//! The bounded-only inbound-acceptance policy: [`BoundedAcceptance`]
//! (one-dimensional baseline, ADR 0031).

use super::{admit_prelude, Admission, ConnectionAcceptanceStrategy};
use crate::connection_state::LinkKind;
use crate::peer::PeerId;
use crate::strategies::edge::accept_cap;
use crate::strategies::view::NodeView;
use crate::topic::TopicId;

/// Accept a verified `Request` iff it is membership-valid and the node is under
/// its per-topic downstream cap `OC = ⌈target_degree + c·√target_degree⌉` — the
/// **cap without the hash gate**, isolating the capacity dimension for the
/// empirical baseline experiments (ADR 0031).
///
/// Never returns `RejectIllegitimate` (no edge predicate is consulted); an
/// over-capacity refusal is the explicit `Rejected`, exactly as in the compound
/// [`HashGatedBoundedAcceptance`](super::HashGatedBoundedAcceptance).
pub struct BoundedAcceptance {
    target_degree: usize,
    cap_buffer: usize,
    kind: LinkKind,
}

impl BoundedAcceptance {
    /// Build the policy from already-parsed inputs (`cap_buffer` is the `c` in
    /// `OC = ⌈target_degree + c·√target_degree⌉`).
    #[must_use]
    pub fn new(target_degree: usize, cap_buffer: usize) -> Self {
        Self {
            target_degree,
            cap_buffer,
            kind: LinkKind::Relay,
        }
    }

    /// Re-target the instance at a link kind (`Relay` is the constructor
    /// default): the kind names which accepted-link class the cap counts.
    #[must_use]
    pub fn for_kind(mut self, kind: LinkKind) -> Self {
        self.kind = kind;
        self
    }
}

impl ConnectionAcceptanceStrategy for BoundedAcceptance {
    fn admit(&self, emitter: &PeerId, topic: &TopicId, view: &NodeView<'_>) -> Admission {
        let accepted_on_topic = match admit_prelude(self.kind, emitter, topic, view) {
            Ok(count) => count,
            Err(decision) => return decision,
        };
        let cap = accept_cap(self.target_degree, self.cap_buffer);
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
    use crate::connection_state::LinkKind;
    use crate::strategies::acceptance::{Admission, ConnectionAcceptanceStrategy};
    use crate::strategies::test_support::{
        candidates, downstream, links_of, peer, subscriptions, topic, view, view_with_upstream,
    };
    use std::collections::BTreeMap;

    // Membership failure takes precedence and is a silent RejectMembership.
    #[test]
    fn membership_invalid_is_rejected() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t2", &["a"])]);
        let down = BTreeMap::new();
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
        let policy = BoundedAcceptance::new(1, 3); // target_degree=1 ⇒ cap 4

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

    // 015 FR-004: relay and publisher capacities are disjoint. A publisher
    // instance counts publisher upstreams only — a relay downstream already at
    // the relay cap does not consume publisher capacity, and vice versa.
    #[test]
    fn relay_and_publisher_caps_count_independently() {
        let t = topic("t1");
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &["a"])]);

        // Relay downstream at the relay cap (target_degree=1 ⇒ cap 4).
        let relay_down = downstream(&[("w", "t1"), ("x", "t1"), ("y", "t1"), ("z", "t1")]);
        // Publisher upstream at the publisher cap.
        let publisher_up = links_of(
            &[("p", "t1"), ("q", "t1"), ("r", "t1"), ("s", "t1")],
            LinkKind::Publisher,
        );

        // The PUBLISHER instance sees a full relay downstream but an empty
        // publisher upstream — it accepts.
        let empty = BTreeMap::new();
        let publisher_policy = BoundedAcceptance::new(1, 3).for_kind(LinkKind::Publisher);
        assert_eq!(
            publisher_policy.admit(
                &peer("a"),
                &t,
                &view_with_upstream(&subs, &cands, &empty, &relay_down),
            ),
            Admission::Accept,
            "full relay downstream must not consume publisher capacity",
        );

        // With the publisher upstream at cap, the publisher instance refuses —
        // even though the relay downstream is empty.
        assert_eq!(
            publisher_policy.admit(
                &peer("a"),
                &t,
                &view_with_upstream(&subs, &cands, &publisher_up, &empty),
            ),
            Admission::RejectOverCapacity,
        );

        // And the RELAY instance ignores the publisher upstream entirely.
        let relay_policy = BoundedAcceptance::new(1, 3);
        assert_eq!(
            relay_policy.admit(
                &peer("a"),
                &t,
                &view_with_upstream(&subs, &cands, &publisher_up, &empty),
            ),
            Admission::Accept,
            "publisher upstream at cap must not consume relay capacity",
        );
    }
}
