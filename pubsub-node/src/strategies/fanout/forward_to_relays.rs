//! The default fan-out policy: [`ForwardToRelays`].

use std::collections::{BTreeMap, BTreeSet};

use super::FanoutStrategy;
use crate::connection_state::{LinkKey, LinkKind, LinkState};
use crate::peer::PeerId;
use crate::received::Origin;
use crate::topic::TopicId;

/// The default fan-out policy: **forward** held messages to every relay
/// downstream peer on the topic — and additionally **seed** a
/// locally-published message over the node's `Active` publisher links — minus
/// the split-horizon exclusion, one send per peer.
///
/// "Forward to relays" is the models' vocabulary: forwarding (relayed/held
/// traffic) runs on relay links only; the publisher-link sends are seeding,
/// which exists only for the node's own publications (M3's exclusivity rule —
/// publisher links never carry relayed traffic). A node with no publisher
/// links (the pre-015 baseline) behaves exactly as before. Degree limits and
/// sampling are deferred to later strategies.
pub struct ForwardToRelays;

impl FanoutStrategy for ForwardToRelays {
    fn targets(
        &self,
        topic: &TopicId,
        downstream: &BTreeMap<LinkKey, LinkState>,
        origin: &Origin,
        exclude: Option<&PeerId>,
    ) -> Vec<PeerId> {
        // Collect into a set first: a peer reachable as both a relay
        // destination and a publisher target receives one send.
        let mut targets: BTreeSet<&PeerId> = BTreeSet::new();
        for (key, state) in downstream {
            if &key.topic != topic {
                continue;
            }
            let selected = match key.kind {
                LinkKind::Relay => true,
                LinkKind::Publisher => *origin == Origin::Local && *state == LinkState::Active,
            };
            if selected {
                targets.insert(&key.peer);
            }
        }
        targets
            .into_iter()
            .filter(|peer| Some(*peer) != exclude)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::ForwardToRelays;
    use crate::peer::PeerId;
    use crate::received::Origin;
    use crate::strategies::fanout::FanoutStrategy;
    use crate::strategies::test_support::{downstream, peer, topic};
    use std::collections::BTreeMap;

    fn sorted(mut v: Vec<PeerId>) -> Vec<PeerId> {
        v.sort_by_key(ToString::to_string);
        v
    }

    // 006 FR-010: ForwardToRelays returns every relay downstream peer on the topic.
    #[test]
    fn forwards_to_every_downstream_on_the_topic() {
        let down = downstream(&[("a", "t1"), ("b", "t1"), ("c", "t2")]);
        let targets = ForwardToRelays.targets(&topic("t1"), &down, &Origin::Local, None);
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
        let targets = ForwardToRelays.targets(
            &topic("t1"),
            &down,
            &Origin::Peer(peer("a")),
            Some(&peer("a")),
        );
        assert_eq!(
            sorted(targets),
            vec![peer("b")],
            "the delivering peer is excluded (split-horizon)",
        );
    }

    // 006 FR-016: empty downstream → no targets.
    #[test]
    fn empty_downstream_yields_no_targets() {
        assert!(ForwardToRelays
            .targets(&topic("t1"), &BTreeMap::new(), &Origin::Local, None)
            .is_empty());
    }

    // A downstream set with no entry on the topic → no targets (subscriber-relay:
    // a node only holds downstream on topics it is a member of).
    #[test]
    fn other_topic_downstream_yields_no_targets() {
        let down = downstream(&[("a", "t2"), ("b", "t2")]);
        assert!(ForwardToRelays
            .targets(&topic("t1"), &down, &Origin::Local, None)
            .is_empty());
    }

    // The sole downstream being the excluded peer → no targets.
    #[test]
    fn sole_downstream_excluded_yields_no_targets() {
        let down = downstream(&[("a", "t1")]);
        assert!(ForwardToRelays
            .targets(
                &topic("t1"),
                &down,
                &Origin::Peer(peer("a")),
                Some(&peer("a"))
            )
            .is_empty());
    }
}
