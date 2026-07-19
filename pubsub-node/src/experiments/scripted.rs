//! Declarative scripted-topology builders: prepopulated populations with
//! hand-computable metrics, used by the framework's own validation tests
//! (known-topology exactness, silent-adversary miss causes, the
//! two-instrument cross-check).
//!
//! A scripted population bypasses the dial phase entirely: links are
//! installed directly as an `Active` upstream on the receiving side and a
//! downstream entry on the forwarding side, via the pre-population path.
//! Peers are alias-keyed (`n000000`, `n000001`, …) so tests reference nodes
//! by index and results stay human-legible.
// 016-FR-032; data-model §8.

use std::collections::BTreeMap;
use std::str::FromStr;

use crate::crypto::mock::MockCryptoScheme;
use crate::peer::PeerId;
use crate::topic::TopicId;

use super::population::{
    AcceptanceSpec, ConnectionSpec, FanoutSpec, Participant, ParticipantClass, Population,
    StrategySpec,
};

/// The topic every scripted population runs on.
const SCRIPTED_TOPIC: &str = "t0";

/// The peer id of scripted node `index` (alias-derived, deterministic).
#[must_use]
pub fn peer(index: usize) -> PeerId {
    PeerId::from_str(&alias(index)).expect("scripted alias is a valid peer id")
}

fn alias(index: usize) -> String {
    format!("n{index:06}")
}

/// A line of `n` nodes: consecutive nodes linked in both directions, so a
/// publish at node `k` reaches node `i` at depth `|i − k|`.
#[must_use]
pub fn line(n: usize) -> ScriptedTopology {
    let mut links = Vec::new();
    for i in 0..n.saturating_sub(1) {
        links.push((i, i + 1));
        links.push((i + 1, i));
    }
    ScriptedTopology::new(n, links)
}

/// A star of `n` nodes: node 0 is the hub, linked in both directions with
/// every leaf; a publish at a leaf reaches the hub at depth 1 and the other
/// leaves at depth 2.
#[must_use]
pub fn star(n: usize) -> ScriptedTopology {
    let mut links = Vec::new();
    for leaf in 1..n {
        links.push((0, leaf));
        links.push((leaf, 0));
    }
    ScriptedTopology::new(n, links)
}

/// A full mesh of `n` nodes: every ordered pair linked; a publish reaches
/// everyone at depth 1.
#[must_use]
pub fn full_mesh(n: usize) -> ScriptedTopology {
    let mut links = Vec::new();
    for from in 0..n {
        for to in 0..n {
            if from != to {
                links.push((from, to));
            }
        }
    }
    ScriptedTopology::new(n, links)
}

/// A scripted topology under construction: `n` nodes, directed links
/// (`from` forwards to `to`), and per-node class overrides.
pub struct ScriptedTopology {
    n: usize,
    links: Vec<(usize, usize)>,
    silent: Vec<usize>,
}

impl ScriptedTopology {
    fn new(n: usize, links: Vec<(usize, usize)>) -> Self {
        Self {
            n,
            links,
            silent: Vec::new(),
        }
    }

    /// Mark node `index` a silent (Level-1 adversarial) relay: honest
    /// transition, no forwarding.
    #[must_use]
    pub fn silent(mut self, index: usize) -> Self {
        self.silent.push(index);
        self
    }

    /// Add one extra directed link (`from` forwards to `to`) on top of the
    /// base shape.
    #[must_use]
    pub fn link(mut self, from: usize, to: usize) -> Self {
        self.links.push((from, to));
        self
    }

    /// Build the prepopulated population: registered open topic, full
    /// candidate sets, readiness set, and the scripted links installed
    /// directly (no dial).
    ///
    /// # Panics
    ///
    /// Panics if a link or silent override references a node ≥ `n` — a
    /// scripted-test authoring error.
    #[must_use]
    pub fn build(self) -> Population {
        let topic = TopicId::from_str(SCRIPTED_TOPIC).expect("valid scripted topic");
        let scheme = MockCryptoScheme::with_seed([0u8; 32]);

        let mut participants = BTreeMap::new();
        for index in 0..self.n {
            let class = if self.silent.contains(&index) {
                ParticipantClass::Adversarial
            } else {
                ParticipantClass::Honest
            };
            let fanout = match class {
                ParticipantClass::Honest => FanoutSpec::ForwardToAll,
                ParticipantClass::Adversarial => FanoutSpec::SilentRelay,
            };
            let spec = StrategySpec {
                connection: ConnectionSpec::connect_to_all(),
                acceptance: AcceptanceSpec::accept_from_all(),
                fanout,
            };
            let key_pair = scheme.keypair_from_alias(&alias(index));
            let participant = Participant::scripted(class, key_pair, &spec)
                .expect("scripted strategy specs are parameterless and always build");
            participants.insert(peer(index), participant);
        }

        let mut population = Population::from_parts(topic.clone(), participants);
        population.prepopulate_registration();

        for (from, to) in &self.links {
            assert!(
                *from < self.n && *to < self.n,
                "scripted link ({from}, {to}) references a node outside 0..{n}",
                n = self.n,
            );
            let (from, to) = (peer(*from), peer(*to));
            population
                .participant_mut(&from)
                .expect("scripted node exists")
                .state_mut()
                .prepopulate_downstream(to.clone(), topic.clone());
            population
                .participant_mut(&to)
                .expect("scripted node exists")
                .state_mut()
                .prepopulate_active_upstream(from, topic.clone());
        }

        population
    }
}

#[cfg(test)]
mod tests {
    use super::{full_mesh, line, peer, star};
    use crate::connection_state::LinkState;
    use crate::experiments::population::ParticipantClass;

    // 016-FR-032: line(3) installs exactly the bidirectional chain links.
    #[test]
    fn line_links_consecutive_nodes_both_ways() {
        let population = line(3).build();
        let topic = population.topic().clone();
        let middle = population.participant(&peer(1)).expect("node exists");
        assert_eq!(middle.downstream(), {
            let mut expected = vec![(peer(0), topic.clone()), (peer(2), topic.clone())];
            expected.sort();
            expected
        },);
        assert_eq!(middle.upstream().len(), 2);
        assert!(middle
            .upstream()
            .iter()
            .all(|(_, _, state)| *state == LinkState::Active));
        let end = population.participant(&peer(0)).expect("node exists");
        assert_eq!(end.downstream(), vec![(peer(1), topic.clone())]);
        assert!(end.is_synced());
        assert_eq!(end.subscriptions(), vec![topic]);
    }

    // 016-FR-032: the star's hub holds a link to every leaf; leaves hold only
    // the hub.
    #[test]
    fn star_centres_on_node_zero() {
        let population = star(4).build();
        let hub = population.participant(&peer(0)).expect("node exists");
        assert_eq!(hub.downstream().len(), 3);
        assert_eq!(hub.upstream().len(), 3);
        for leaf in 1..4 {
            let participant = population.participant(&peer(leaf)).expect("node exists");
            assert_eq!(participant.downstream().len(), 1);
            assert_eq!(participant.upstream().len(), 1);
        }
    }

    // 016-FR-032: the mesh links every ordered pair.
    #[test]
    fn full_mesh_links_every_ordered_pair() {
        let population = full_mesh(4).build();
        for (_, participant) in population.participants() {
            assert_eq!(participant.downstream().len(), 3);
            assert_eq!(participant.upstream().len(), 3);
        }
    }

    // Data-model §8: the silent override flips class and fan-out only — the
    // node still holds its links.
    #[test]
    fn silent_override_marks_the_node_adversarial() {
        let population = full_mesh(3).silent(1).build();
        let silent = population.participant(&peer(1)).expect("node exists");
        assert_eq!(silent.class(), ParticipantClass::Adversarial);
        assert!(!silent.is_down());
        assert_eq!(silent.downstream().len(), 2);
        assert_eq!(
            population
                .participants()
                .filter(|(_, p)| p.class() == ParticipantClass::Honest)
                .count(),
            2,
        );
    }
}
