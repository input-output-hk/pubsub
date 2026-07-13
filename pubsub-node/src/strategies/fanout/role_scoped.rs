//! The M3 partition fan-out policy: [`RoleScopedFanout`].

use super::FanoutStrategy;
use crate::connection_state::{LinkState, LinkStore};
use crate::peer::PeerId;
use crate::received::Origin;
use crate::topic::TopicId;

/// The strict M3 partition (ADR 0034): a locally-published message goes over
/// the node's **standing initiation links only** (`Active` publish-out cell);
/// a relayed message goes over the **relay downstream only** (relay-in cell,
/// minus the split-horizon exclusion). Neither role ever carries the other's
/// traffic — the publisher's relay downstream receive its publications through
/// the flood from the initiation targets, not directly.
///
/// This is the `m3/README.md` link semantics taken strictly ("initiation
/// links carry only their owner's own publications — they are never part of
/// the relay graph"); [`ForwardToAll`](super::ForwardToAll) is the union
/// reading in which a publisher also serves its own message to its
/// requesters. The experiments cross-validate both against the model's
/// coverage laws (analysis A8).
///
/// Caution: on a node with **no** initiation links configured, a local
/// publish selects **no targets** — the partition makes `--publish-strategy
/// none` a mute-publisher configuration. Pair this fan-out with an
/// established publish slot.
pub struct RoleScopedFanout;

impl FanoutStrategy for RoleScopedFanout {
    fn targets(
        &self,
        topic: &TopicId,
        links: &LinkStore,
        origin: &Origin,
        exclude: Option<&PeerId>,
    ) -> Vec<PeerId> {
        let cell = match origin {
            Origin::Local => links.publish_out(),
            Origin::Peer(_) => links.relay_in(),
        };
        cell.iter()
            .filter(|((_, t), _)| t == topic)
            .filter(|(_, state)| match origin {
                // Initiation targets must be established; relay-in entries are
                // Active by construction.
                Origin::Local => **state == LinkState::Active,
                Origin::Peer(_) => true,
            })
            .map(|((peer, _), _)| peer)
            .filter(|peer| Some(*peer) != exclude)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::RoleScopedFanout;
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

    // ADR 0034 / M3 partition: a local publication goes over initiation links
    // ONLY — the relay downstream is excluded.
    #[test]
    fn local_origin_targets_initiation_links_only() {
        let targets = RoleScopedFanout.targets(&topic("t1"), &store(), &Origin::Local, None);
        assert_eq!(sorted(targets), vec![peer("b")], "initiation links only");
    }

    // ADR 0034 / M3 partition: a relayed message goes over the relay
    // downstream ONLY — initiation links never relay.
    #[test]
    fn peer_origin_targets_relay_downstream_only() {
        let targets = RoleScopedFanout.targets(
            &topic("t1"),
            &store(),
            &Origin::Peer(peer("x")),
            Some(&peer("x")),
        );
        assert_eq!(sorted(targets), vec![peer("a")], "relay downstream only");
    }

    // Split-horizon still applies on the relay path.
    #[test]
    fn split_horizon_excludes_the_deliverer() {
        let targets = RoleScopedFanout.targets(
            &topic("t1"),
            &store(),
            &Origin::Peer(peer("a")),
            Some(&peer("a")),
        );
        assert!(targets.is_empty(), "the sole downstream is the deliverer");
    }

    // The mute-publisher configuration: no initiation links → a local publish
    // has no targets under the partition.
    #[test]
    fn local_origin_without_initiation_links_selects_nothing() {
        let store = links(&[(
            "a",
            "t1",
            LinkRole::Relay,
            LinkDirection::In,
            LinkState::Active,
        )]);
        assert!(RoleScopedFanout
            .targets(&topic("t1"), &store, &Origin::Local, None)
            .is_empty());
    }
}
