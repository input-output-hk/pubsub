//! The v1 fan-out policy: [`ForwardToAll`].

use super::FanoutStrategy;
use crate::connection_state::{LinkDirection, LinkRole, LinkState, Links};
use crate::peer::PeerId;
use crate::received::Origin;
use crate::topic::TopicId;

/// The v1 fan-out policy: forward to **every** appropriate link on the topic,
/// minus the split-horizon exclusion (ADR 0021, origin-aware since ADR 0033).
///
/// Returns each peer holding a `Relay`/`In` link on `topic` (the relay fan-out
/// destinations — for **any** origin), plus, when `origin` is
/// [`Origin::Local`], each peer behind an **`Active`** `Publisher`/`Out` link
/// (the node's publishing-link injection targets; a pending `AwaitingAccept`
/// publish dial is not a target). A `Publisher` link never carries an
/// [`Origin::Peer`] message — publishing links do not relay. The split-horizon
/// `exclude` applies regardless of role. This maintains the full per-topic
/// fan-out; degree limits and sampling are deferred to later strategies.
pub struct ForwardToAll;

impl FanoutStrategy for ForwardToAll {
    fn targets(
        &self,
        topic: &TopicId,
        links: &Links,
        origin: &Origin,
        exclude: Option<&PeerId>,
    ) -> Vec<PeerId> {
        links
            .iter()
            .filter(|((_, t, _, _), _)| t == topic)
            .filter(|((_, _, role, direction), state)| match (role, direction) {
                (LinkRole::Relay, LinkDirection::In) => true,
                (LinkRole::Publisher, LinkDirection::Out) => {
                    **state == LinkState::Active && *origin == Origin::Local
                }
                _ => false,
            })
            .map(|((peer, _, _, _), _)| peer)
            .filter(|peer| Some(*peer) != exclude)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::ForwardToAll;
    use crate::connection_state::{LinkDirection, LinkRole, LinkState, Links};
    use crate::peer::PeerId;
    use crate::received::Origin;
    use crate::strategies::fanout::FanoutStrategy;
    use crate::strategies::test_support::{downstream, links, peer, topic};

    fn sorted(mut v: Vec<PeerId>) -> Vec<PeerId> {
        v.sort_by_key(ToString::to_string);
        v
    }

    fn local() -> Origin {
        Origin::Local
    }

    fn from_peer(alias: &str) -> Origin {
        Origin::Peer(peer(alias))
    }

    // 006 FR-010: ForwardToAll returns every relay downstream peer on the topic.
    #[test]
    fn forwards_to_every_downstream_on_the_topic() {
        let down = downstream(&[("a", "t1"), ("b", "t1"), ("c", "t2")]);
        let targets = ForwardToAll.targets(&topic("t1"), &down, &local(), None);
        assert_eq!(
            sorted(targets),
            vec![peer("a"), peer("b")],
            "only the t1 downstream peers, both of them",
        );
    }

    // 006 FR-009 split-horizon: the excluded peer is removed from the targets.
    #[test]
    fn exclude_removes_that_peer() {
        let down = downstream(&[("a", "t1"), ("b", "t1")]);
        let targets = ForwardToAll.targets(&topic("t1"), &down, &from_peer("a"), Some(&peer("a")));
        assert_eq!(
            sorted(targets),
            vec![peer("b")],
            "the delivering peer is excluded (split-horizon)",
        );
    }

    // 006 FR-016: empty links → no targets.
    #[test]
    fn empty_downstream_yields_no_targets() {
        assert!(ForwardToAll
            .targets(&topic("t1"), &Links::new(), &local(), None)
            .is_empty());
    }

    // A link store with no entry on the topic → no targets (subscriber-relay:
    // a node only holds downstream on topics it is a member of).
    #[test]
    fn other_topic_downstream_yields_no_targets() {
        let down = downstream(&[("a", "t2"), ("b", "t2")]);
        assert!(ForwardToAll
            .targets(&topic("t1"), &down, &local(), None)
            .is_empty());
    }

    // The sole downstream being the excluded peer → no targets.
    #[test]
    fn sole_downstream_excluded_yields_no_targets() {
        let down = downstream(&[("a", "t1")]);
        assert!(ForwardToAll
            .targets(&topic("t1"), &down, &from_peer("a"), Some(&peer("a")))
            .is_empty());
    }

    // 015 FR-005/SC-002: a locally-originated message targets the active
    // outbound publishing link AND the relay downstream.
    #[test]
    fn local_origin_targets_publish_links_and_relay_downstream() {
        let store = links(&[
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
        ]);
        let targets = ForwardToAll.targets(&topic("t1"), &store, &local(), None);
        assert_eq!(
            sorted(targets),
            vec![peer("a"), peer("b")],
            "publish + relay targets for a local origin",
        );
    }

    // 015 FR-005/SC-002: a relayed message never targets a publishing link.
    #[test]
    fn peer_origin_excludes_publish_links() {
        let store = links(&[
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
        ]);
        let targets = ForwardToAll.targets(&topic("t1"), &store, &from_peer("x"), Some(&peer("x")));
        assert_eq!(
            sorted(targets),
            vec![peer("a")],
            "publishing links do not relay",
        );
    }

    // 015: a pending (AwaitingAccept) publish dial is not yet a target.
    #[test]
    fn pending_publish_link_is_not_a_target() {
        let store = links(&[(
            "b",
            "t1",
            LinkRole::Publisher,
            LinkDirection::Out,
            LinkState::AwaitingAccept,
        )]);
        assert!(ForwardToAll
            .targets(&topic("t1"), &store, &local(), None)
            .is_empty());
    }

    // 015 edge case: a topic with only publishing links and a peer origin → no
    // targets (publishing links do not relay).
    #[test]
    fn publish_only_topic_with_peer_origin_yields_no_targets() {
        let store = links(&[(
            "b",
            "t1",
            LinkRole::Publisher,
            LinkDirection::Out,
            LinkState::Active,
        )]);
        assert!(ForwardToAll
            .targets(&topic("t1"), &store, &from_peer("x"), None)
            .is_empty());
    }

    // 015: inbound publishing links (Publisher/In) are message sources, never
    // fan-out targets — for either origin.
    #[test]
    fn inbound_publish_links_are_never_targets() {
        let store = links(&[(
            "b",
            "t1",
            LinkRole::Publisher,
            LinkDirection::In,
            LinkState::Active,
        )]);
        assert!(ForwardToAll
            .targets(&topic("t1"), &store, &local(), None)
            .is_empty());
        assert!(ForwardToAll
            .targets(&topic("t1"), &store, &from_peer("x"), None)
            .is_empty());
    }

    // 015: outbound relay links (Relay/Out — upstream sources) are never
    // fan-out targets.
    #[test]
    fn relay_upstream_links_are_never_targets() {
        let store = links(&[(
            "b",
            "t1",
            LinkRole::Relay,
            LinkDirection::Out,
            LinkState::Active,
        )]);
        assert!(ForwardToAll
            .targets(&topic("t1"), &store, &local(), None)
            .is_empty());
    }
}
