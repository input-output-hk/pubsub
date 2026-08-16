//! The unified inbound-acceptance policy: [`UnifiedAcceptance`] — gate
//! verification and the serving cap as two independent dimensions.

use super::{admit_prelude, Admission, ConnectionAcceptanceStrategy};
use crate::connection_state::LinkKind;
use crate::peer::PeerId;
use crate::strategies::edge::{
    is_valid_edge, is_valid_edge_publisher, is_valid_edge_sym, is_valid_edge_sym_ordered,
};
use crate::strategies::view::NodeView;
use crate::topic::TopicId;

/// The unified acceptance policy: one implementation whose two independent,
/// individually optional dimensions replace the four one-dimensional
/// baselines (which remain expressible as knob combinations).
///
/// - **Gate** (`gate: Option<B>`): verify the inbound request against the
///   seam's verifiable edge predicate at `B` — the same bucket count the
///   dialers use (the agreement condition verifiability rests on). A
///   predicate failure is a **silent** [`Admission::RejectIllegitimate`].
///   `None` skips verification entirely — either the seam is ungated (the
///   gate is vacuous) or the operator opted out of verification (the
///   trusting-acceptors comparison arm); both resolve to `None` at
///   construction, never as a runtime branch.
/// - **Cap** (`accept_cap: Option<C>`): bound **admissions** at `C`, refusing
///   over-budget requests with [`Admission::RejectOverCapacity`] (an explicit
///   `Rejected` reply — the dialer cleans up its pending entry). On a
///   directional instance the admitted set is exactly the kind-scoped link
///   scan, so the scan is the count; on a **symmetric** instance the mirrored
///   link set counts both roles, so the cap reads the per-epoch
///   admitted-count instead (ADR 0042): the node's own picks never spend it
///   and crossings are exempt at the handler, so `C` bounds precisely the
///   edges other peers chose. `None` is unbounded; `0` admits no one: every
///   fresh link is refused with the explicit rejection.
///
/// The decision order is prelude (membership, then the idempotent
/// already-held re-Accept) → gate → cap, so a predicate-failing request is
/// never reported as over-capacity and a re-dialed held link is re-affirmed
/// ahead of both dimensions.
///
/// The four pre-017 acceptance baselines are knob combinations:
///
/// | previous baseline | gate | cap |
/// |---|---|---|
/// | membership only (accept-from-all) | `None` | `None` |
/// | capped (bounded) | `None` | `Some(C)` |
/// | verifying (hash-gated) | `Some(B)` | `None` |
/// | verifying + capped (hash-gated-bounded) | `Some(B)` | `Some(C)` |
// 017-FR-010, 017-FR-011, 017-FR-013; research R1; data-model decision order.
pub struct UnifiedAcceptance {
    self_id: PeerId,
    kind: LinkKind,
    symmetric: bool,
    symmetric_ordered: bool,
    gate: Option<usize>,
    accept_cap: Option<usize>,
}

impl UnifiedAcceptance {
    /// Build the policy at the plane origin — no gate verification, no cap:
    /// every membership-valid request is accepted (the accept-from-all
    /// point).
    #[must_use]
    pub fn new(self_id: PeerId) -> Self {
        Self {
            self_id,
            kind: LinkKind::Relay,
            symmetric: false,
            symmetric_ordered: false,
            gate: None,
            accept_cap: None,
        }
    }

    /// Configure the bucket count the gate verifies at. Feed the seam's
    /// bucket count so acceptors verify exactly the `B` the dialers use;
    /// `None` (the constructor default) admits without predicate
    /// verification — the edge maps the verification opt-out here, so the
    /// opt-out is a construction-time resolution.
    #[must_use]
    pub fn with_gate(mut self, gate: Option<usize>) -> Self {
        self.gate = gate;
        self
    }

    /// Configure the absolute per-topic admissions bound for the instance's
    /// link kind. `None` (the constructor default) is unbounded; `Some(0)`
    /// refuses every fresh link with the explicit over-capacity rejection.
    #[must_use]
    pub fn with_accept_cap(mut self, accept_cap: Option<usize>) -> Self {
        self.accept_cap = accept_cap;
        self
    }

    /// Re-target the instance at a link kind (`Relay` is the constructor
    /// default): the kind selects the hash domain the gate verifies under
    /// and which accepted-link class the cap counts (capacities are
    /// disjoint per kind).
    #[must_use]
    pub fn for_kind(mut self, kind: LinkKind) -> Self {
        self.kind = kind;
        self
    }

    /// Verify inbound requests against the **symmetric** edge predicate,
    /// and bound the cap against the per-epoch admitted-count instead of
    /// the (both-role) link scan (ADR 0042): both relay seams must switch
    /// together (one flag drives the dial side, the acceptance side, and
    /// the handshake vocabulary). Applies to relay instances; the publisher
    /// seam stays directional and never sets this.
    #[must_use]
    pub fn with_symmetric(mut self, symmetric: bool) -> Self {
        self.symmetric = symmetric;
        self
    }

    /// Verify a symmetric instance's inbound requests against the
    /// **ordered** comparison predicate (ADR 0043) — the dialer's own
    /// directional draw, `emitter → self`, under the dedicated ordered
    /// domain. Legal only together with the symmetric switch; the cap
    /// semantics (the admissions budget) are unchanged. Selected by the
    /// experiments configuration, never an operator option.
    #[must_use]
    pub fn with_symmetric_ordered(mut self, ordered: bool) -> Self {
        self.symmetric_ordered = ordered;
        self
    }
}

impl ConnectionAcceptanceStrategy for UnifiedAcceptance {
    fn admit(&self, emitter: &PeerId, topic: &TopicId, view: &NodeView<'_>) -> Admission {
        // Membership, then the idempotent already-held re-Accept — the
        // shared prelude every refusing policy runs first (see
        // `acceptance::admit_prelude` for the half-open-link rationale).
        let accepted_on_topic = match admit_prelude(self.kind, emitter, topic, view) {
            Ok(count) => count,
            Err(decision) => return decision,
        };
        // Gate: verify against the same edge predicate and bucket count the
        // dialer used (the agreement condition — one per-seam value feeds
        // both sides at construction). The emitter is the requester, this
        // node the candidate. `None` means no verification: the seam is
        // ungated, or the operator opted out at the edge.
        if let Some(buckets) = self.gate {
            let valid = if self.symmetric {
                if self.symmetric_ordered {
                    // ADR 0043: verify the DIALER's directional draw —
                    // emitter → self — under the ordered domain.
                    is_valid_edge_sym_ordered(
                        view.epoch_nonce,
                        topic,
                        emitter,
                        &self.self_id,
                        buckets,
                    )
                } else {
                    is_valid_edge_sym(view.epoch_nonce, topic, emitter, &self.self_id, buckets)
                }
            } else {
                match self.kind {
                    LinkKind::Relay => {
                        is_valid_edge(view.epoch_nonce, topic, emitter, &self.self_id, buckets)
                    }
                    LinkKind::Publisher => is_valid_edge_publisher(
                        view.epoch_nonce,
                        topic,
                        emitter,
                        &self.self_id,
                        buckets,
                    ),
                }
            };
            if !valid {
                return Admission::RejectIllegitimate;
            }
        }
        // Cap: the fed absolute admissions bound; 0 refuses every fresh link
        // (017-FR-013 — the explicit rejection, never a silent drop). A
        // symmetric instance bounds the per-epoch admitted-count — never the
        // link scan, which counts both roles of every mirrored edge
        // (ADR 0042); a directional instance's scan IS its admitted set.
        if let Some(cap) = self.accept_cap {
            let spent = if self.symmetric {
                view.admitted_count(self.kind, topic)
            } else {
                accepted_on_topic
            };
            if spent >= cap {
                return Admission::RejectOverCapacity;
            }
        }
        Admission::Accept
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::UnifiedAcceptance;
    use crate::connection_state::LinkKind;
    use crate::strategies::acceptance::{Admission, ConnectionAcceptanceStrategy};
    use crate::strategies::edge::{
        is_valid_edge, is_valid_edge_publisher, is_valid_edge_sym, is_valid_edge_sym_ordered,
    };
    use crate::strategies::test_support::{
        candidates, downstream, links_of, no_admissions, no_links, peer, subscriptions, topic,
        view, view_with_admitted, view_with_upstream,
    };

    /// The first generated candidate name satisfying `predicate` — the
    /// robust-to-the-exact-hash search the acceptance suites use instead of
    /// betting on a handful of names.
    fn find_candidate(predicate: impl Fn(&str) -> bool) -> String {
        (0..10_000)
            .map(|i| format!("cand-{i}"))
            .find(|n| predicate(n))
            .expect("some generated candidate satisfies the predicate")
    }

    // The prelude runs first: a request that is not membership-valid is a
    // silent RejectMembership regardless of the knobs.
    #[test]
    fn membership_invalid_is_rejected() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t2", &["a"])]);
        let policy = UnifiedAcceptance::new(peer("self"))
            .with_gate(Some(2))
            .with_accept_cap(Some(4));
        let got = policy.admit(&peer("a"), &topic("t2"), &view(&subs, &cands, no_links()));
        assert_eq!(got, Admission::RejectMembership);
    }

    // 017-FR-010: the (no gate, no cap) point is the accept-from-all
    // baseline — every membership-valid request is accepted, unbounded.
    #[test]
    fn ungated_uncapped_admits_every_member() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &["a"])]);
        let policy = UnifiedAcceptance::new(peer("self"));
        // Unbounded: a well-served topic still accepts.
        let held = downstream(&[("w", "t1"), ("x", "t1"), ("y", "t1"), ("z", "t1")]);
        assert_eq!(
            policy.admit(&peer("a"), &topic("t1"), &view(&subs, &cands, &held)),
            Admission::Accept,
        );
    }

    // 017-FR-011: gate present — a membership-valid request whose edge
    // predicate fails at B is a silent RejectIllegitimate.
    #[test]
    fn gate_rejects_predicate_failing_requests() {
        let t = topic("t1");
        let failing = find_candidate(|n| !is_valid_edge(0, &t, &peer(n), &peer("self"), 2));
        let cands = candidates(&[("t1", &[failing.as_str()])]);
        let subs = subscriptions(&["t1"]);
        let policy = UnifiedAcceptance::new(peer("self")).with_gate(Some(2));
        assert_eq!(
            policy.admit(&peer(&failing), &t, &view(&subs, &cands, no_links())),
            Admission::RejectIllegitimate,
        );
    }

    // 017-FR-011: gate present — a predicate-passing request is accepted.
    #[test]
    fn gate_admits_predicate_passing_requests() {
        let t = topic("t1");
        let passing = find_candidate(|n| is_valid_edge(0, &t, &peer(n), &peer("self"), 2));
        let cands = candidates(&[("t1", &[passing.as_str()])]);
        let subs = subscriptions(&["t1"]);
        let policy = UnifiedAcceptance::new(peer("self")).with_gate(Some(2));
        assert_eq!(
            policy.admit(&peer(&passing), &t, &view(&subs, &cands, no_links())),
            Admission::Accept,
        );
    }

    // 017-FR-011: gate None admits predicate-failing requests — the
    // trusting-acceptors arm (gated dialers, non-verifying acceptors) is a
    // construction-time resolution.
    #[test]
    fn gate_none_admits_predicate_failing_requests() {
        let t = topic("t1");
        let failing = find_candidate(|n| !is_valid_edge(0, &t, &peer(n), &peer("self"), 2));
        let cands = candidates(&[("t1", &[failing.as_str()])]);
        let subs = subscriptions(&["t1"]);
        let policy = UnifiedAcceptance::new(peer("self"));
        assert_eq!(
            policy.admit(&peer(&failing), &t, &view(&subs, &cands, no_links())),
            Admission::Accept,
        );
    }

    // 017-FR-010: cap present — Accept below the cap, RejectOverCapacity at
    // it (the explicit-rejection dimension, independent of the gate).
    #[test]
    fn cap_bounds_admission() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &["a"])]);
        let policy = UnifiedAcceptance::new(peer("self")).with_accept_cap(Some(4));

        let below = downstream(&[("x", "t1"), ("y", "t1"), ("z", "t1")]);
        assert_eq!(
            policy.admit(&peer("a"), &topic("t1"), &view(&subs, &cands, &below)),
            Admission::Accept,
        );

        let at = downstream(&[("w", "t1"), ("x", "t1"), ("y", "t1"), ("z", "t1")]);
        assert_eq!(
            policy.admit(&peer("a"), &topic("t1"), &view(&subs, &cands, &at)),
            Admission::RejectOverCapacity,
        );
    }

    // 017-FR-013: an accept cap of 0 refuses every new link with the
    // explicit over-capacity rejection (the deliberate change from the
    // deleted off-switch's silent drop — the dialer cleans up its pending
    // entry on the resulting Rejected).
    #[test]
    fn cap_zero_refuses_every_new_link() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &["a"])]);
        let policy = UnifiedAcceptance::new(peer("self")).with_accept_cap(Some(0));
        assert_eq!(
            policy.admit(&peer("a"), &topic("t1"), &view(&subs, &cands, no_links())),
            Admission::RejectOverCapacity,
        );
    }

    // The prelude's idempotent re-Accept fires ahead of both dimensions: a
    // re-dial of an already-held link is re-affirmed even at cap 0, so a
    // lost/late Accepted repairs the link instead of stranding it half-open.
    #[test]
    fn already_held_link_reaccepts_ahead_of_gate_and_cap() {
        let t = topic("t1");
        // No dependence on the predicate: the short-circuit fires first.
        let failing = find_candidate(|n| !is_valid_edge(0, &t, &peer(n), &peer("self"), 2));
        let cands = candidates(&[("t1", &[failing.as_str()])]);
        let subs = subscriptions(&["t1"]);
        let held = downstream(&[(failing.as_str(), "t1")]);
        let policy = UnifiedAcceptance::new(peer("self"))
            .with_gate(Some(2))
            .with_accept_cap(Some(0));
        assert_eq!(
            policy.admit(&peer(&failing), &t, &view(&subs, &cands, &held)),
            Admission::Accept,
        );
    }

    // Data-model decision order: the gate fires before the cap — a
    // predicate-failing request on a full topic reads RejectIllegitimate,
    // never RejectOverCapacity.
    #[test]
    fn gate_fires_before_the_cap() {
        let t = topic("t1");
        let failing = find_candidate(|n| !is_valid_edge(0, &t, &peer(n), &peer("self"), 2));
        let cands = candidates(&[("t1", &[failing.as_str()])]);
        let subs = subscriptions(&["t1"]);
        let policy = UnifiedAcceptance::new(peer("self"))
            .with_gate(Some(2))
            .with_accept_cap(Some(0));
        assert_eq!(
            policy.admit(&peer(&failing), &t, &view(&subs, &cands, no_links())),
            Admission::RejectIllegitimate,
        );
    }

    // 017-FR-010 (both dimensions): a predicate-passing request on a full
    // topic is refused over capacity — the gated+capped compound behaviour.
    #[test]
    fn gated_and_capped_composes() {
        let t = topic("t1");
        let passing = find_candidate(|n| is_valid_edge(0, &t, &peer(n), &peer("self"), 2));
        let cands = candidates(&[("t1", &[passing.as_str()])]);
        let subs = subscriptions(&["t1"]);
        let policy = UnifiedAcceptance::new(peer("self"))
            .with_gate(Some(2))
            .with_accept_cap(Some(1));
        let held = downstream(&[("x", "t1")]);
        assert_eq!(
            policy.admit(&peer(&passing), &t, &view(&subs, &cands, &held)),
            Admission::RejectOverCapacity,
        );
        assert_eq!(
            policy.admit(&peer(&passing), &t, &view(&subs, &cands, no_links())),
            Admission::Accept,
        );
    }

    // The per-kind capacity scan is disjoint: a publisher instance counts
    // accepted publisher upstreams and ignores relay downstream entries.
    #[test]
    fn publisher_instances_count_publisher_upstreams_only() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &["a", "p", "q"])]);
        let policy = UnifiedAcceptance::new(peer("self"))
            .for_kind(LinkKind::Publisher)
            .with_accept_cap(Some(2));

        // Two accepted publisher upstreams: at cap for the publisher class.
        let up = links_of(&[("p", "t1"), ("q", "t1")], LinkKind::Publisher);
        // A crowded relay downstream must not count against it.
        let down = downstream(&[("w", "t1"), ("x", "t1"), ("y", "t1")]);
        assert_eq!(
            policy.admit(
                &peer("a"),
                &topic("t1"),
                &view_with_upstream(&subs, &cands, &up, &down),
            ),
            Admission::RejectOverCapacity,
        );

        // One publisher upstream: below cap, the relay entries still ignored.
        let up_one = links_of(&[("p", "t1")], LinkKind::Publisher);
        assert_eq!(
            policy.admit(
                &peer("a"),
                &topic("t1"),
                &view_with_upstream(&subs, &cands, &up_one, &down),
            ),
            Admission::Accept,
        );
    }

    // A publisher-kind gate verifies under the publisher hash domain, not
    // the relay one: a request passing the relay predicate but failing the
    // publisher predicate at the same B is rejected by a publisher instance.
    #[test]
    fn publisher_instances_verify_under_the_publisher_domain() {
        let t = topic("t1");
        let name = find_candidate(|n| {
            is_valid_edge(0, &t, &peer(n), &peer("self"), 2)
                && !is_valid_edge_publisher(0, &t, &peer(n), &peer("self"), 2)
        });
        let cands = candidates(&[("t1", &[name.as_str()])]);
        let subs = subscriptions(&["t1"]);
        let empty = BTreeMap::new();
        let policy = UnifiedAcceptance::new(peer("self"))
            .for_kind(LinkKind::Publisher)
            .with_gate(Some(2));
        assert_eq!(
            policy.admit(
                &peer(&name),
                &t,
                &view_with_upstream(&subs, &cands, &empty, no_links()),
            ),
            Admission::RejectIllegitimate,
        );
    }

    // ADR 0042: a symmetric instance's cap bounds the per-epoch admissions
    // count, never the link scan — mirrored symmetric state counts both
    // roles of every edge, and the node's own picks are not admissions. A
    // crowded downstream with no budget spent admits; an empty downstream
    // with the budget spent refuses.
    #[test]
    fn symmetric_cap_reads_the_admissions_budget_not_the_scan() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &["a"])]);
        let policy = UnifiedAcceptance::new(peer("self"))
            .with_symmetric(true)
            .with_accept_cap(Some(2));

        // Five held links (own picks and their mirrors under symmetric),
        // zero admissions spent: the scan would refuse, the budget admits.
        let held = downstream(&[
            ("v", "t1"),
            ("w", "t1"),
            ("x", "t1"),
            ("y", "t1"),
            ("z", "t1"),
        ]);
        assert_eq!(
            policy.admit(
                &peer("a"),
                &topic("t1"),
                &view_with_admitted(&subs, &cands, &held, no_admissions()),
            ),
            Admission::Accept,
        );

        // No links held, budget spent: the scan would admit, the budget
        // refuses.
        let spent = BTreeMap::from([((topic("t1"), LinkKind::Relay), 2usize)]);
        assert_eq!(
            policy.admit(
                &peer("a"),
                &topic("t1"),
                &view_with_admitted(&subs, &cands, no_links(), &spent),
            ),
            Admission::RejectOverCapacity,
        );

        // A directional instance at the same knobs keeps the scan.
        let directional = UnifiedAcceptance::new(peer("self")).with_accept_cap(Some(2));
        assert_eq!(
            directional.admit(
                &peer("a"),
                &topic("t1"),
                &view_with_admitted(&subs, &cands, &held, no_admissions()),
            ),
            Admission::RejectOverCapacity,
        );
    }

    // ADR 0043: an ordered symmetric instance verifies the DIALER's
    // directional draw (emitter → self) under the ordered domain — a
    // request passing the pair draw but failing the ordered one is
    // rejected, and vice versa.
    #[test]
    fn ordered_instances_verify_the_dialers_direction() {
        let t = topic("t1");
        let policy = UnifiedAcceptance::new(peer("self"))
            .with_gate(Some(2))
            .with_symmetric(true)
            .with_symmetric_ordered(true);

        let ordered_fails_pair_passes = find_candidate(|n| {
            !is_valid_edge_sym_ordered(0, &t, &peer(n), &peer("self"), 2)
                && is_valid_edge_sym(0, &t, &peer(n), &peer("self"), 2)
        });
        let cands = candidates(&[("t1", &[ordered_fails_pair_passes.as_str()])]);
        let subs = subscriptions(&["t1"]);
        assert_eq!(
            policy.admit(
                &peer(&ordered_fails_pair_passes),
                &t,
                &view(&subs, &cands, no_links()),
            ),
            Admission::RejectIllegitimate,
        );

        let ordered_passes =
            find_candidate(|n| is_valid_edge_sym_ordered(0, &t, &peer(n), &peer("self"), 2));
        let cands = candidates(&[("t1", &[ordered_passes.as_str()])]);
        assert_eq!(
            policy.admit(&peer(&ordered_passes), &t, &view(&subs, &cands, no_links())),
            Admission::Accept,
        );
    }

    // 017-FR-004/FR-011: a symmetric instance verifies the unordered-pair
    // predicate — a request failing the symmetric draw is rejected even
    // where the directional predicate would admit it, and vice versa.
    #[test]
    fn symmetric_instances_verify_the_unordered_pair_predicate() {
        let t = topic("t1");
        let policy = UnifiedAcceptance::new(peer("self"))
            .with_gate(Some(2))
            .with_symmetric(true);

        let sym_fails_dir_passes = find_candidate(|n| {
            !is_valid_edge_sym(0, &t, &peer(n), &peer("self"), 2)
                && is_valid_edge(0, &t, &peer(n), &peer("self"), 2)
        });
        let cands = candidates(&[("t1", &[sym_fails_dir_passes.as_str()])]);
        let subs = subscriptions(&["t1"]);
        assert_eq!(
            policy.admit(
                &peer(&sym_fails_dir_passes),
                &t,
                &view(&subs, &cands, no_links()),
            ),
            Admission::RejectIllegitimate,
        );

        let sym_passes = find_candidate(|n| is_valid_edge_sym(0, &t, &peer(n), &peer("self"), 2));
        let cands = candidates(&[("t1", &[sym_passes.as_str()])]);
        assert_eq!(
            policy.admit(&peer(&sym_passes), &t, &view(&subs, &cands, no_links())),
            Admission::Accept,
        );
    }
}
