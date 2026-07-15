//! The M5 fan-out policy: [`AllLinks`].

use std::collections::{BTreeMap, BTreeSet};

use super::FanoutStrategy;
use crate::connection_state::{LinkKey, LinkState};
use crate::peer::PeerId;
use crate::received::Origin;
use crate::topic::TopicId;

/// The M5 fan-out policy: forward **every** held message — any origin — to
/// the union of relay downstream peers and `Active` publisher links, minus
/// the split-horizon exclusion, one send per peer.
///
/// This is M5's send side (`m5/README.md`): both link classes carry every
/// message; origin plays no role. Pair it network-wide with the
/// `any-verified` publisher admission — under the `owner-only` default the
/// receive side drops exactly the foreign-publisher hops this policy emits.
pub struct AllLinks;

impl FanoutStrategy for AllLinks {
    fn targets(
        &self,
        topic: &TopicId,
        downstream: &BTreeMap<LinkKey, LinkState>,
        _origin: &Origin,
        exclude: Option<&PeerId>,
    ) -> Vec<PeerId> {
        // One entry per peer: a peer reachable over both kinds gets one send.
        let mut targets: BTreeSet<&PeerId> = BTreeSet::new();
        for (key, state) in downstream {
            if &key.topic == topic && *state == LinkState::Active {
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
    use super::AllLinks;
    use crate::connection_state::{LinkKey, LinkKind, LinkState};
    use crate::received::Origin;
    use crate::strategies::fanout::FanoutStrategy;
    use crate::strategies::test_support::{downstream, links_of, peer, topic};

    // M5: peer-origin messages ride publisher links too — the union, deduped.
    #[test]
    fn unions_relay_and_publisher_for_peer_origin() {
        let mut down = downstream(&[("a", "t1"), ("b", "t1")]);
        down.extend(links_of(&[("b", "t1"), ("c", "t1")], LinkKind::Publisher));
        let targets = AllLinks.targets(&topic("t1"), &down, &Origin::Peer(peer("x")), None);
        let mut got: Vec<String> = targets.iter().map(ToString::to_string).collect();
        got.sort();
        assert_eq!(got, vec!["a", "b", "c"], "union, one send per peer");
    }

    // A pending (AwaitingAccept) publisher dial carries nothing.
    #[test]
    fn pending_links_are_not_targets() {
        let mut down = downstream(&[]);
        down.insert(
            LinkKey::new(topic("t1"), peer("d"), LinkKind::Publisher),
            LinkState::AwaitingAccept,
        );
        assert!(AllLinks
            .targets(&topic("t1"), &down, &Origin::Local, None)
            .is_empty());
    }

    // Split-horizon still applies.
    #[test]
    fn exclude_removes_the_deliverer() {
        let down = links_of(&[("a", "t1")], LinkKind::Publisher);
        assert!(AllLinks
            .targets(
                &topic("t1"),
                &down,
                &Origin::Peer(peer("a")),
                Some(&peer("a"))
            )
            .is_empty());
    }
}
