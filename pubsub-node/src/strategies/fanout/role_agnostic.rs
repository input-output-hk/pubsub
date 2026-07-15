//! The M5 union fan-out policy: [`RoleAgnosticFanout`].

use super::FanoutStrategy;
use crate::connection_state::{LinkState, LinkStore};
use crate::peer::PeerId;
use crate::received::Origin;
use crate::topic::TopicId;

/// The M5 dissemination semantics — **no link-role distinction**
/// (`formal_spec/hybrid_dissemination/models/m5/README.md`): a node relays
/// every message it holds — any origin — on **every outgoing propagation
/// edge**: its relay downstream (the peers that picked it as forwarder) **and**
/// its own `Active` outbound standing links (`Publisher`/Out — the model's
/// `k_out` picks), except back on the arrival link. Targets are deduplicated
/// per peer, as in [`ForwardToAll`](super::ForwardToAll).
///
/// The role axis in one line: [`ForwardToAll`](super::ForwardToAll) reserves
/// initiation links for the node's **own** publications (M3);
/// [`RoleScopedFanout`](super::RoleScopedFanout) additionally keeps own
/// publications **off** the relay links (strict partition); this policy
/// ignores roles entirely.
///
/// Pair with `--publish-in-admission any-verified` network-wide: the targets
/// of the outbound standing links must admit relayed traffic over their
/// `Publisher`/In cells, or every such forward is dropped
/// (`relay_over_publish_link`).
pub struct RoleAgnosticFanout;

impl FanoutStrategy for RoleAgnosticFanout {
    fn targets(
        &self,
        topic: &TopicId,
        links: &LinkStore,
        origin: &Origin,
        exclude: Option<&PeerId>,
    ) -> Vec<PeerId> {
        let _ = origin; // every origin floods identically (M5)
                        // One pass over the topic's sinks (ADR 0036): any facet makes the
                        // peer a target — relay accepted, or an Active initiation link.
        links
            .sinks_on(topic)
            .filter(|(_, relay_accepted, push)| *relay_accepted || *push == Some(LinkState::Active))
            .map(|(peer, _, _)| peer)
            .filter(|peer| Some(*peer) != exclude)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::RoleAgnosticFanout;
    use crate::connection_state::{LinkDirection, LinkRole, LinkState};
    use crate::peer::PeerId;
    use crate::received::Origin;
    use crate::strategies::fanout::FanoutStrategy;
    use crate::strategies::test_support::{links, peer, topic};

    fn sorted(mut v: Vec<PeerId>) -> Vec<PeerId> {
        v.sort_by_key(ToString::to_string);
        v
    }

    fn store() -> crate::connection_state::LinkStore {
        links(&[
            (
                "a",
                "t1",
                LinkRole::Relay,
                LinkDirection::In,
                LinkState::Active,
            ),
            (
                "b",
                "t1",
                LinkRole::Publisher,
                LinkDirection::Out,
                LinkState::Active,
            ),
        ])
    }

    // ADR 0035 / M5: a RELAYED message floods over the relay downstream AND
    // the outbound standing links — no role or origin distinction.
    #[test]
    fn peer_origin_floods_both_cells() {
        let targets = RoleAgnosticFanout.targets(
            &topic("t1"),
            &store(),
            &Origin::Peer(peer("x")),
            Some(&peer("x")),
        );
        assert_eq!(
            sorted(targets),
            vec![peer("a"), peer("b")],
            "M5: k_out links carry relayed traffic too",
        );
    }

    // A local publish floods identically.
    #[test]
    fn local_origin_floods_both_cells() {
        let targets = RoleAgnosticFanout.targets(&topic("t1"), &store(), &Origin::Local, None);
        assert_eq!(sorted(targets), vec![peer("a"), peer("b")]);
    }

    // The arrival link is excluded even when it is an outbound standing link.
    #[test]
    fn arrival_link_excluded_regardless_of_cell() {
        let targets = RoleAgnosticFanout.targets(
            &topic("t1"),
            &store(),
            &Origin::Peer(peer("b")),
            Some(&peer("b")),
        );
        assert_eq!(sorted(targets), vec![peer("a")], "no echo on arrival link");
    }
}
