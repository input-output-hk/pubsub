//! The v1 fan-out policy: [`ForwardToAll`].

use std::collections::HashSet;

use super::FanoutStrategy;
use crate::peer::PeerId;
use crate::topic::TopicId;

/// The v1 fan-out policy: forward to **every** downstream peer on the topic,
/// minus the split-horizon exclusion.
///
/// Returns each `peer` for which `(peer, topic)` is in `downstream` and
/// `Some(peer) != exclude`. This maintains the full per-topic fan-out; degree
/// limits and sampling are deferred to later strategies.
pub struct ForwardToAll;

impl FanoutStrategy for ForwardToAll {
    fn targets(
        &self,
        topic: &TopicId,
        downstream: &HashSet<(PeerId, TopicId)>,
        exclude: Option<&PeerId>,
    ) -> Vec<PeerId> {
        downstream
            .iter()
            .filter(|(_, t)| t == topic)
            .map(|(peer, _)| peer)
            .filter(|peer| Some(*peer) != exclude)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::ForwardToAll;
    use crate::fanout::FanoutStrategy;
    use crate::peer::PeerId;
    use crate::topic::TopicId;
    use std::collections::HashSet;
    use std::str::FromStr;

    fn peer(s: &str) -> PeerId {
        PeerId::from_str(s).expect("valid peer id")
    }

    fn topic(s: &str) -> TopicId {
        TopicId::from_str(s).expect("valid topic id")
    }

    fn downstream(entries: &[(&str, &str)]) -> HashSet<(PeerId, TopicId)> {
        entries.iter().map(|(p, t)| (peer(p), topic(t))).collect()
    }

    fn sorted(mut v: Vec<PeerId>) -> Vec<PeerId> {
        v.sort_by_key(ToString::to_string);
        v
    }

    // FR-010: ForwardToAll returns every downstream peer on the topic.
    #[test]
    fn forwards_to_every_downstream_on_the_topic() {
        let down = downstream(&[("a", "t1"), ("b", "t1"), ("c", "t2")]);
        let targets = ForwardToAll.targets(&topic("t1"), &down, None);
        assert_eq!(
            sorted(targets),
            vec![peer("a"), peer("b")],
            "only the t1 downstream peers, both of them",
        );
    }

    // FR-009 split-horizon: the excluded peer is removed from the targets.
    #[test]
    fn exclude_removes_that_peer() {
        let down = downstream(&[("a", "t1"), ("b", "t1")]);
        let targets = ForwardToAll.targets(&topic("t1"), &down, Some(&peer("a")));
        assert_eq!(
            sorted(targets),
            vec![peer("b")],
            "the delivering peer is excluded (split-horizon)",
        );
    }

    // FR-016: empty downstream → no targets.
    #[test]
    fn empty_downstream_yields_no_targets() {
        assert!(ForwardToAll
            .targets(&topic("t1"), &HashSet::new(), None)
            .is_empty());
    }

    // A downstream set with no entry on the topic → no targets (subscriber-relay:
    // a node only holds downstream on topics it is a member of).
    #[test]
    fn other_topic_downstream_yields_no_targets() {
        let down = downstream(&[("a", "t2"), ("b", "t2")]);
        assert!(ForwardToAll.targets(&topic("t1"), &down, None).is_empty());
    }

    // The sole downstream being the excluded peer → no targets.
    #[test]
    fn sole_downstream_excluded_yields_no_targets() {
        let down = downstream(&[("a", "t1")]);
        assert!(ForwardToAll
            .targets(&topic("t1"), &down, Some(&peer("a")))
            .is_empty());
    }
}
