//! Experiments-only strategy instances: [`SilentRelay`] (fan-out) and
//! [`UniformSampler`] (dial).
//!
//! Both implement the protocol's strategy seams but are available to
//! experiment configurations only — they are never protocol CLI kinds. The
//! silent relay is the models' dissemination-optimal (worst-case) adversary:
//! it accepts and records like an honest node but forwards to no one. The
//! uniform sampler is the formal M2 selection family — exactly
//! `min(target_degree, |candidates|)` uniform picks without replacement —
//! which hash-gated selection (binomial realised degree) is not; without it
//! the M2 comparison would conflate the selection-family gap with instrument
//! error.
// 016-FR-012 (silent relay), 016-FR-013 (uniform sampler); research R10.

use std::collections::{BTreeMap, BTreeSet};

use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use sha2::{Digest, Sha256};

use crate::connection_state::{LinkKey, LinkState};
use crate::peer::PeerId;
use crate::received::Origin;
use crate::strategies::connection::ConnectionStrategy;
use crate::strategies::fanout::FanoutStrategy;
use crate::strategies::view::NodeView;
use crate::topic::TopicId;

/// A fan-out policy that selects no targets: the silent relay.
///
/// A participant running it behaves exactly like an honest node on every
/// other seam — it dials, accepts, records, and dedups — but never forwards
/// a dissemination message. This is the worst-case (dissemination-optimal)
/// adversary of the analytical models.
#[derive(Clone, Copy, Debug, Default)]
pub struct SilentRelay;

impl FanoutStrategy for SilentRelay {
    fn targets(
        &self,
        _topic: &TopicId,
        _downstream: &BTreeMap<LinkKey, LinkState>,
        _origin: &Origin,
        _exclude: Option<&PeerId>,
    ) -> Vec<PeerId> {
        Vec::new()
    }
}

/// A dial policy that samples exactly `min(target_degree, |candidates|)`
/// candidates per topic, uniformly without replacement, from a fixed seed.
///
/// The seed is derived from the master seed by the population build, so the
/// pick is deterministic: the same view and seed always yield the same
/// expected set (repeated heartbeats within an epoch re-dial the same set —
/// the retry primitive holds). When fewer than `target_degree` candidates
/// exist, all of them are picked — the same degeneracy direction as the
/// hash-gated small-topic connect-to-all floor.
#[derive(Clone, Debug)]
pub struct UniformSampler {
    target_degree: usize,
    seed: [u8; 32],
}

impl UniformSampler {
    /// Construct a sampler picking `target_degree` upstreams per topic from
    /// the given derived seed.
    #[must_use]
    pub fn new(target_degree: usize, seed: [u8; 32]) -> Self {
        Self {
            target_degree,
            seed,
        }
    }

    /// The per-topic sampling seed: the sampler's seed domain-separated by the
    /// topic, so multi-topic views never reuse one stream across topics.
    fn topic_seed(&self, topic: &TopicId) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"experiments/uniform-sampler/v1");
        hasher.update(self.seed);
        hasher.update(topic.as_str().as_bytes());
        hasher.finalize().into()
    }
}

impl ConnectionStrategy for UniformSampler {
    fn expected_links(&self, view: &NodeView<'_>) -> BTreeSet<(PeerId, TopicId)> {
        let mut expected = BTreeSet::new();
        for topic in view.subscriptions {
            let Some(candidates) = view.candidates.get(topic) else {
                continue;
            };
            if candidates.is_empty() {
                continue;
            }
            // Candidates iterate in sorted order (BTreeSet), so index sampling
            // over the ordered list is deterministic in (seed, candidate set).
            let ordered: Vec<&PeerId> = candidates.iter().collect();
            let amount = self.target_degree.min(ordered.len());
            let mut rng = ChaCha20Rng::from_seed(self.topic_seed(topic));
            for index in rand::seq::index::sample(&mut rng, ordered.len(), amount) {
                expected.insert((ordered[index].clone(), topic.clone()));
            }
        }
        expected
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::str::FromStr;

    use super::{SilentRelay, UniformSampler};
    use crate::connection_state::{LinkKey, LinkKind, LinkState};
    use crate::peer::PeerId;
    use crate::received::Origin;
    use crate::strategies::connection::ConnectionStrategy;
    use crate::strategies::fanout::FanoutStrategy;
    use crate::strategies::view::NodeView;
    use crate::topic::TopicId;

    fn peer(s: &str) -> PeerId {
        PeerId::from_str(s).expect("valid peer id")
    }

    fn topic(s: &str) -> TopicId {
        TopicId::from_str(s).expect("valid topic id")
    }

    type ViewFixture = (
        BTreeSet<TopicId>,
        BTreeMap<TopicId, BTreeSet<PeerId>>,
        BTreeMap<LinkKey, LinkState>,
    );

    fn view_fixture(candidates: usize) -> ViewFixture {
        let subscriptions: BTreeSet<TopicId> = [topic("t0")].into_iter().collect();
        let peers: BTreeSet<PeerId> = (0..candidates).map(|i| peer(&format!("c{i:03}"))).collect();
        let candidate_map: BTreeMap<TopicId, BTreeSet<PeerId>> =
            [(topic("t0"), peers)].into_iter().collect();
        (subscriptions, candidate_map, BTreeMap::new())
    }

    fn expected_of(sampler: &UniformSampler, candidates: usize) -> BTreeSet<(PeerId, TopicId)> {
        let (subscriptions, candidate_map, no_links) = view_fixture(candidates);
        let view = NodeView {
            subscriptions: &subscriptions,
            candidates: &candidate_map,
            upstream: &no_links,
            downstream: &no_links,
            epoch_nonce: 0,
        };
        sampler.expected_links(&view)
    }

    // 016-FR-012: the silent relay selects no targets, whatever downstream
    // holds and whatever the delivery origin.
    #[test]
    fn silent_relay_selects_no_targets() {
        let downstream: BTreeMap<LinkKey, LinkState> = [
            (
                LinkKey::new(topic("t0"), peer("a"), LinkKind::Relay),
                LinkState::Active,
            ),
            (
                LinkKey::new(topic("t0"), peer("b"), LinkKind::Relay),
                LinkState::Active,
            ),
        ]
        .into_iter()
        .collect();
        assert!(SilentRelay
            .targets(&topic("t0"), &downstream, &Origin::Local, None)
            .is_empty());
        assert!(SilentRelay
            .targets(
                &topic("t0"),
                &downstream,
                &Origin::Peer(peer("a")),
                Some(&peer("a"))
            )
            .is_empty());
    }

    // 016-FR-013: exactly target_degree picks, without replacement, when enough
    // candidates exist. (The BTreeSet result deduplicates, so its size proves
    // the without-replacement property.)
    #[test]
    fn samples_exactly_target_degree_without_replacement() {
        let sampler = UniformSampler::new(5, [7u8; 32]);
        let expected = expected_of(&sampler, 20);
        assert_eq!(expected.len(), 5);
    }

    // 016-FR-013: min(target_degree, |candidates|) degeneracy — fewer candidates
    // than the target degree means all of them are picked.
    #[test]
    fn degenerates_to_all_candidates_when_short() {
        let sampler = UniformSampler::new(8, [7u8; 32]);
        let expected = expected_of(&sampler, 3);
        assert_eq!(expected.len(), 3);
    }

    // The retry primitive: the same seed and view yield the same expected set on
    // every call (a repeated heartbeat re-dials the SAME set).
    #[test]
    fn repeated_calls_yield_the_same_set() {
        let sampler = UniformSampler::new(5, [9u8; 32]);
        assert_eq!(expected_of(&sampler, 20), expected_of(&sampler, 20));
    }

    // Distinct seeds draw distinct sets (deterministically checked pair).
    #[test]
    fn distinct_seeds_draw_distinct_sets() {
        let a = expected_of(&UniformSampler::new(5, [0u8; 32]), 40);
        let b = expected_of(&UniformSampler::new(5, [1u8; 32]), 40);
        assert_ne!(a, b);
    }

    // An unsubscribed or candidate-less view selects nothing.
    #[test]
    fn empty_views_select_nothing() {
        let sampler = UniformSampler::new(5, [7u8; 32]);
        assert!(expected_of(&sampler, 0).is_empty());
    }
}
