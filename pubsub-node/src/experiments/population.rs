//! The population layer: participants, classes, the seeded population build,
//! and the two registration setup modes' inputs — direct state pre-population
//! (the fast path) and faithful-fold event scripts (the fidelity check).
//!
//! A [`Population`] owns one driver-side node core per participant. Every
//! participant runs the real transition function; an adversarial (Level-1)
//! participant differs only in its strategy bundle — the driver never
//! branches on class when delivering events.
// 016-FR-004, 016-FR-008, 016-FR-011, 016-FR-031; data-model §1.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;

use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use sha2::{Digest, Sha256};

use crate::connection_state::{LinkKind, LinkState};
use crate::crypto::mock::{KeyPair, TestSigner, TestVerifier};
use crate::crypto::MessageHash;
use crate::event::Event;
use crate::peer::PeerId;
use crate::state::NodeState;
use crate::strategies::acceptance::{AcceptanceStrategyKind, ConnectionAcceptanceStrategy};
use crate::strategies::config::{
    AcceptanceParams, ConnectionParams, NodeStrategies, StrategyConfigError,
};
use crate::strategies::connection::{ConnectionStrategy, ConnectionStrategyKind};
use crate::strategies::fanout::{FanoutStrategy, ForwardToRelays};
use crate::subscription_registry::MembershipEvent;
use crate::topic::TopicId;
use crate::topic_registry::TopicRegistryEvent;

use super::strategies::{SilentRelay, UniformSampler};

/// A participant's class, assigned at population build by the seeded class
/// draw. Adversarial participants are Level-1 in v1: they run the honest
/// transition with a hostile strategy bundle. The enum shape admits a future
/// protocol-violating (Level-2) variant without reworking storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ParticipantClass {
    /// Runs the honest strategy bundle; counted in coverage/goodness
    /// denominators.
    Honest,
    /// Runs a hostile strategy bundle over the honest transition (Level-1);
    /// excluded from coverage/goodness denominators.
    Adversarial,
}

/// One population member: its class, liveness mark, identity keys, and the
/// real node core the driver steps.
///
/// `down` is set only by the churn draw, only on honest participants, only
/// between the dial drain and the publish drain. **Up-honest** means
/// `class == Honest && !down`.
pub struct Participant {
    class: ParticipantClass,
    down: bool,
    key_pair: KeyPair,
    state: NodeState,
}

impl Participant {
    /// Wire one participant's node core from its class, keys, and strategy
    /// spec — the single construction site both the seeded build and the
    /// scripted-topology path use.
    fn from_spec(
        class: ParticipantClass,
        key_pair: KeyPair,
        spec: &StrategySpec,
        sampler_seed: [u8; 32],
        verifier: &Arc<TestVerifier>,
    ) -> Result<Self, StrategyConfigError> {
        let peer = PeerId::new(key_pair.public.clone());
        let state = NodeState::new(
            peer.clone(),
            BTreeSet::new(),
            0,
            Arc::clone(verifier) as Arc<dyn crate::crypto::Verifier>,
            Arc::new(TestSigner::new(key_pair.private.clone())),
            NodeStrategies::relay_only(
                spec.connection.build(&peer, sampler_seed)?,
                spec.acceptance.build(&peer)?,
            ),
            spec.fanout.build(),
        );
        Ok(Self {
            class,
            down: false,
            key_pair,
            state,
        })
    }

    /// Build a participant for the scripted-topology path: fixed sampler seed
    /// (scripted topologies install links directly and never dial).
    pub(crate) fn scripted(
        class: ParticipantClass,
        key_pair: KeyPair,
        spec: &StrategySpec,
    ) -> Result<Self, StrategyConfigError> {
        Self::from_spec(class, key_pair, spec, [0u8; 32], &Arc::new(TestVerifier))
    }

    /// The participant's class.
    #[must_use]
    pub fn class(&self) -> ParticipantClass {
        self.class
    }

    /// Whether the churn draw marked this participant down.
    #[must_use]
    pub fn is_down(&self) -> bool {
        self.down
    }

    /// Whether this participant is up-honest (honest and not down).
    #[must_use]
    pub fn is_up_honest(&self) -> bool {
        self.class == ParticipantClass::Honest && !self.down
    }

    pub(crate) fn mark_down(&mut self) {
        self.down = true;
    }

    pub(crate) fn state_mut(&mut self) -> &mut NodeState {
        &mut self.state
    }

    pub(crate) fn key_pair(&self) -> &KeyPair {
        &self.key_pair
    }

    /// The node's subscription set.
    #[must_use]
    pub fn subscriptions(&self) -> Vec<TopicId> {
        self.state.subscriptions_snapshot()
    }

    /// The node's candidate peers for `topic`.
    #[must_use]
    pub fn candidates(&self, topic: &TopicId) -> Vec<PeerId> {
        self.state.candidates_snapshot(topic)
    }

    /// Whether the **stored** membership set for `topic` contains `peer` —
    /// raw, no self-exclusion; lets the registration-mode equivalence test
    /// pin the full-membership invariant the filtered snapshot cannot see.
    #[cfg(test)]
    pub(crate) fn candidate_set_contains(&self, topic: &TopicId, peer: &PeerId) -> bool {
        self.state.candidate_set_contains(topic, peer)
    }

    /// Whether the node has folded/been given readiness.
    #[must_use]
    pub fn is_synced(&self) -> bool {
        self.state.is_synced()
    }

    /// The node's **relay** upstream connections (peer, topic, state), sorted
    /// by (peer, topic). Experiments populations are relay-only (the M2
    /// baseline shape), so the relay-filtered snapshot is the whole picture.
    #[must_use]
    pub fn upstream(&self) -> Vec<(PeerId, TopicId, LinkState)> {
        let mut entries = self.state.upstream_relays();
        entries.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
        entries
    }

    /// The node's **relay** downstream connections (peer, topic), sorted.
    /// Relay-filtered by design: publisher links are seed edges, never
    /// propagation edges, so the M2 digraph extraction must not see them.
    #[must_use]
    pub fn downstream(&self) -> Vec<(PeerId, TopicId)> {
        let mut entries = self.state.downstream_relays();
        entries.sort();
        entries
    }

    /// Whether the node has accepted a message with this content hash
    /// (published or received).
    #[must_use]
    pub fn has_seen(&self, hash: &MessageHash) -> bool {
        self.state.has_seen(hash)
    }

    /// Number of deliveries the node has recorded.
    #[must_use]
    pub fn received_count(&self) -> usize {
        self.state.received_len()
    }

    /// Where the node's recorded delivery of the given content came from —
    /// [`Origin::Local`](crate::Origin::Local) for its own publish, the
    /// delivering peer otherwise. `None` if the node never recorded it.
    #[must_use]
    pub fn delivery_origin(&self, hash: &MessageHash) -> Option<crate::received::Origin> {
        self.state
            .received()
            .iter()
            .find_map(|delivery| match &delivery.message {
                crate::message::Message::Dissemination(signed)
                    if MessageHash::of(&signed.plain) == *hash =>
                {
                    Some(delivery.origin.clone())
                }
                _ => None,
            })
    }

    /// The node's current epoch nonce.
    #[must_use]
    pub fn epoch_nonce(&self) -> u64 {
        self.state.epoch_nonce()
    }
}

/// The strategy triad one class of participants runs.
#[derive(Clone, Debug)]
pub struct StrategySpec {
    /// The dial (connection-selection) policy.
    pub connection: ConnectionSpec,
    /// The inbound-acceptance policy.
    pub acceptance: AcceptanceSpec,
    /// The fan-out policy.
    pub fanout: FanoutSpec,
}

impl StrategySpec {
    /// Validate the spec by constructing every strategy once with a
    /// placeholder identity: parameter errors surface at configuration
    /// validation, before any population is built.
    pub fn probe(&self, self_id: &PeerId) -> Result<(), StrategyConfigError> {
        self.connection.build(self_id, [0u8; 32])?;
        self.acceptance.build(self_id)?;
        let _ = self.fanout.build();
        Ok(())
    }
}

/// A dial policy specification: a protocol kind (005's CLI kinds) or the
/// experiments-only uniform sampler.
#[derive(Clone, Debug)]
pub enum ConnectionSpec {
    /// One of the protocol's own connection kinds, with its parameters.
    Protocol {
        /// The protocol strategy kind.
        kind: ConnectionStrategyKind,
        /// Target degree, where the kind requires one.
        target_degree: Option<usize>,
        /// Optional pinned bucket count.
        bucket_count: Option<usize>,
    },
    /// The experiments-only uniform sampler: exactly
    /// `min(target_degree, |candidates|)` uniform picks without replacement,
    /// seeded per participant from the master seed.
    UniformSampler {
        /// Number of upstreams to sample per topic.
        target_degree: usize,
    },
}

impl ConnectionSpec {
    /// The parameterless full-mesh protocol dial.
    #[must_use]
    pub fn connect_to_all() -> Self {
        Self::Protocol {
            kind: ConnectionStrategyKind::ConnectToAll,
            target_degree: None,
            bucket_count: None,
        }
    }

    fn build(
        &self,
        self_id: &PeerId,
        sampler_seed: [u8; 32],
    ) -> Result<Arc<dyn ConnectionStrategy>, StrategyConfigError> {
        match self {
            Self::Protocol {
                kind,
                target_degree,
                bucket_count,
            } => kind.build(&ConnectionParams {
                self_id: self_id.clone(),
                kind: LinkKind::Relay,
                target_degree: *target_degree,
                bucket_count: *bucket_count,
                symmetric: false,
            }),
            Self::UniformSampler { target_degree } => {
                Ok(Arc::new(UniformSampler::new(*target_degree, sampler_seed)))
            }
        }
    }
}

/// An acceptance policy specification (the protocol's own kinds; there is no
/// experiments-only acceptance strategy in v1).
#[derive(Clone, Debug)]
pub enum AcceptanceSpec {
    /// One of the protocol's acceptance kinds, with its parameters.
    Protocol {
        /// The protocol acceptance kind.
        kind: AcceptanceStrategyKind,
        /// Target degree, where the kind requires one.
        target_degree: Option<usize>,
        /// Optional pinned bucket count.
        bucket_count: Option<usize>,
        /// Accept-cap buffer `c` (protocol default 3).
        cap_buffer: usize,
    },
}

impl AcceptanceSpec {
    /// The parameterless membership-only protocol acceptance.
    #[must_use]
    pub fn accept_from_all() -> Self {
        Self::Protocol {
            kind: AcceptanceStrategyKind::AcceptFromAll,
            target_degree: None,
            bucket_count: None,
            cap_buffer: 3,
        }
    }

    fn build(
        &self,
        self_id: &PeerId,
    ) -> Result<Arc<dyn ConnectionAcceptanceStrategy>, StrategyConfigError> {
        match self {
            Self::Protocol {
                kind,
                target_degree,
                bucket_count,
                cap_buffer,
            } => kind.build(&AcceptanceParams {
                self_id: self_id.clone(),
                kind: LinkKind::Relay,
                target_degree: *target_degree,
                bucket_count: *bucket_count,
                cap_buffer: *cap_buffer,
                symmetric: false,
            }),
        }
    }
}

/// A fan-out policy specification.
#[derive(Clone, Copy, Debug)]
pub enum FanoutSpec {
    /// Forward held messages to every relay downstream on the topic — the
    /// protocol's default policy. (Experiment populations are relay-only, so
    /// this is the whole fan-out; the M5 all-links `forward-to-all` kind
    /// waits for publisher-link experiment support.)
    ForwardToRelays,
    /// Forward to no one (the experiments-only silent adversary).
    SilentRelay,
}

impl FanoutSpec {
    fn build(self) -> Arc<dyn FanoutStrategy> {
        match self {
            Self::ForwardToRelays => Arc::new(ForwardToRelays),
            Self::SilentRelay => Arc::new(SilentRelay),
        }
    }
}

/// The build inputs of a seeded population.
#[derive(Clone, Debug)]
pub struct PopulationConfig {
    /// The single topic the whole population subscribes to.
    pub topic: TopicId,
    /// Total participant count N.
    pub size: usize,
    /// How many participants the class draw marks adversarial.
    pub adversarial: usize,
    /// The strategy triad honest participants run.
    pub honest_strategies: StrategySpec,
    /// The strategy triad adversarial participants run.
    pub adversarial_strategies: StrategySpec,
}

/// The seeds a population build consumes. Derived from the master seed by the
/// sweep layer; explicit here so a run stays a pure function of
/// its inputs.
#[derive(Clone, Copy, Debug)]
pub struct PopulationSeeds {
    /// Identity-key generation (the seeded mock crypto scheme).
    pub keys: [u8; 32],
    /// The class draw (which participants are adversarial).
    pub classes: [u8; 32],
    /// Root of the per-participant sampler seeds.
    pub sampler: [u8; 32],
}

/// Rejected population-build inputs.
#[derive(Debug, thiserror::Error)]
pub enum PopulationBuildError {
    /// Fewer than two honest participants: no publisher/receiver pair exists.
    #[error("population must contain at least two honest participants (a publisher and a receiver); {size} total minus {adversarial} adversarial leaves {honest}")]
    TooFewHonest {
        /// Total population size.
        size: usize,
        /// Requested adversarial count.
        adversarial: usize,
        /// Resulting honest count.
        honest: usize,
    },
    /// A strategy specification rejected its parameters.
    #[error(transparent)]
    Strategy(#[from] StrategyConfigError),
}

/// A driver-owned population: one real node core per participant, keyed by
/// peer id (deterministic iteration order).
pub struct Population {
    topic: TopicId,
    participants: BTreeMap<PeerId, Participant>,
}

impl Population {
    /// Build a population from already-parsed inputs and derived seeds.
    ///
    /// Keys come from the seeded mock crypto scheme; the class draw marks
    /// `adversarial` of the `size` participants (uniform, seeded); each
    /// participant's sampler seed is derived from the sampler root and the
    /// participant's build index. The population is **not** registered yet —
    /// registration is a driver phase with two setup modes.
    pub fn build(
        config: &PopulationConfig,
        seeds: &PopulationSeeds,
    ) -> Result<Self, PopulationBuildError> {
        let honest = config.size.saturating_sub(config.adversarial);
        if honest < 2 || config.adversarial > config.size {
            return Err(PopulationBuildError::TooFewHonest {
                size: config.size,
                adversarial: config.adversarial,
                honest: if config.adversarial > config.size {
                    0
                } else {
                    honest
                },
            });
        }

        let mut scheme = crate::crypto::mock::MockCryptoScheme::with_seed(seeds.keys);
        let key_pairs: Vec<KeyPair> = (0..config.size)
            .map(|_| scheme.generate_keypair())
            .collect();

        let mut class_rng = ChaCha20Rng::from_seed(seeds.classes);
        let adversarial_indices: HashSet<usize> =
            rand::seq::index::sample(&mut class_rng, config.size, config.adversarial)
                .into_iter()
                .collect();

        let verifier: Arc<TestVerifier> = Arc::new(TestVerifier);
        let mut participants = BTreeMap::new();
        for (index, key_pair) in key_pairs.into_iter().enumerate() {
            let class = if adversarial_indices.contains(&index) {
                ParticipantClass::Adversarial
            } else {
                ParticipantClass::Honest
            };
            let spec = match class {
                ParticipantClass::Honest => &config.honest_strategies,
                ParticipantClass::Adversarial => &config.adversarial_strategies,
            };
            let sampler_seed = derive_seed(&seeds.sampler, "participant-sampler", index as u64);
            let peer = PeerId::new(key_pair.public.clone());
            participants.insert(
                peer,
                Participant::from_spec(class, key_pair, spec, sampler_seed, &verifier)?,
            );
        }

        Ok(Self {
            topic: config.topic.clone(),
            participants,
        })
    }

    /// Assemble a population from already-built participants (the scripted-
    /// topology path).
    pub(crate) fn from_parts(topic: TopicId, participants: BTreeMap<PeerId, Participant>) -> Self {
        Self {
            topic,
            participants,
        }
    }

    /// The population's single topic.
    #[must_use]
    pub fn topic(&self) -> &TopicId {
        &self.topic
    }

    /// Number of participants.
    #[must_use]
    pub fn len(&self) -> usize {
        self.participants.len()
    }

    /// Whether the population is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.participants.is_empty()
    }

    /// Iterate participants in canonical (peer-id) order.
    pub fn participants(&self) -> impl Iterator<Item = (&PeerId, &Participant)> {
        self.participants.iter()
    }

    /// Look up one participant.
    #[must_use]
    pub fn participant(&self, id: &PeerId) -> Option<&Participant> {
        self.participants.get(id)
    }

    pub(crate) fn participant_mut(&mut self, id: &PeerId) -> Option<&mut Participant> {
        self.participants.get_mut(id)
    }

    /// All peer ids in canonical order.
    #[must_use]
    pub fn peer_ids(&self) -> Vec<PeerId> {
        self.participants.keys().cloned().collect()
    }

    /// The up-honest participants (honest and not down), canonical order.
    #[must_use]
    pub fn up_honest(&self) -> Vec<PeerId> {
        self.participants
            .iter()
            .filter(|(_, p)| p.is_up_honest())
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Direct state pre-population — the fast-path registration mode:
    /// every participant gets the topic registered open, its own
    /// subscription, the topic's full membership set (self included — the
    /// strategy view excludes the node's own id at read time, ADR 0038), and
    /// readiness, with no folds and no readiness dial.
    ///
    /// Every core receives the **same** `Arc`-shared membership set — one
    /// (N)-element set per run instead of N of them, the O(N²) → O(N)
    /// collapse of N-033. Nothing mutates the sets after this point (churn
    /// marks participants down without touching membership), so the sharing
    /// survives the whole run.
    pub fn prepopulate_registration(&mut self) {
        let members: Arc<BTreeSet<PeerId>> = Arc::new(self.participants.keys().cloned().collect());
        let topic = self.topic.clone();
        for participant in self.participants.values_mut() {
            let state = participant.state_mut();
            state.prepopulate_registered_topic(topic.clone(), BTreeSet::new());
            state.prepopulate_subscription(topic.clone());
            state.prepopulate_candidates(topic.clone(), Arc::clone(&members));
            state.prepopulate_synced();
        }
    }

    /// The faithful-mode registration script for one target node: the topic
    /// registration fold, then one membership fold per member — every entry
    /// folds into the per-topic membership set (the node's own entry also
    /// sets its subscriptions), reproducing the fast path's full-membership
    /// content (ADR 0038). The
    /// readiness event is deliberately absent — the driver injects all
    /// `Synced` events as one wave, after every node has folded these
    /// (the registration barrier).
    pub(crate) fn faithful_registration_events(&self) -> Vec<Event> {
        let mut events = vec![Event::TopicRegistryUpdate(TopicRegistryEvent::Registered {
            topic: self.topic.clone(),
            publishers: BTreeSet::new(),
        })];
        for member in self.participants.keys() {
            events.push(Event::MembershipUpdate(MembershipEvent::Joined {
                node: member.clone(),
                topics: [self.topic.clone()].into_iter().collect(),
            }));
        }
        events
    }
}

/// Derive a labelled sub-seed: `SHA-256(label ‖ parent ‖ index)`. The domain
/// label keeps independently-purposed draws uncorrelated.
pub(crate) fn derive_seed(parent: &[u8; 32], label: &str, index: u64) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(label.as_bytes());
    hasher.update(parent);
    hasher.update(index.to_be_bytes());
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::{
        derive_seed, AcceptanceSpec, ConnectionSpec, FanoutSpec, ParticipantClass, Population,
        PopulationBuildError, PopulationConfig, PopulationSeeds, StrategySpec,
    };
    use crate::topic::TopicId;

    fn spec(fanout: FanoutSpec) -> StrategySpec {
        StrategySpec {
            connection: ConnectionSpec::connect_to_all(),
            acceptance: AcceptanceSpec::accept_from_all(),
            fanout,
        }
    }

    fn config(size: usize, adversarial: usize) -> PopulationConfig {
        PopulationConfig {
            topic: TopicId::from_str("t0").expect("valid topic"),
            size,
            adversarial,
            honest_strategies: spec(FanoutSpec::ForwardToRelays),
            adversarial_strategies: spec(FanoutSpec::SilentRelay),
        }
    }

    fn seeds() -> PopulationSeeds {
        PopulationSeeds {
            keys: [1u8; 32],
            classes: [2u8; 32],
            sampler: [3u8; 32],
        }
    }

    // 016-FR-004: the population is keyed by peer id with build-time classes;
    // the seeded class draw marks exactly the configured adversarial count.
    #[test]
    fn class_draw_marks_exactly_the_configured_count() {
        let population = Population::build(&config(10, 3), &seeds()).expect("valid build");
        assert_eq!(population.len(), 10);
        let adversarial = population
            .participants()
            .filter(|(_, p)| p.class() == ParticipantClass::Adversarial)
            .count();
        assert_eq!(adversarial, 3);
        assert_eq!(population.up_honest().len(), 7);
    }

    // 016-FR-024: the build is deterministic in its seeds — same seeds, same
    // identities and classes.
    #[test]
    fn build_is_deterministic_in_the_seeds() {
        let a = Population::build(&config(10, 3), &seeds()).expect("valid build");
        let b = Population::build(&config(10, 3), &seeds()).expect("valid build");
        assert_eq!(a.peer_ids(), b.peer_ids());
        for (id, participant) in a.participants() {
            let other = b.participant(id).expect("same identity set");
            assert_eq!(participant.class(), other.class());
        }
        // A different class seed redraws the classes over the same identities.
        let mut other_seeds = seeds();
        other_seeds.classes = [9u8; 32];
        let c = Population::build(&config(10, 3), &other_seeds).expect("valid build");
        assert_eq!(a.peer_ids(), c.peer_ids());
    }

    // 016-FR-031: a population without a publisher/receiver pair is rejected.
    #[test]
    fn too_few_honest_is_rejected() {
        assert!(matches!(
            Population::build(&config(3, 2), &seeds()),
            Err(PopulationBuildError::TooFewHonest { honest: 1, .. }),
        ));
        assert!(matches!(
            Population::build(&config(2, 3), &seeds()),
            Err(PopulationBuildError::TooFewHonest { .. }),
        ));
        assert!(Population::build(&config(2, 0), &seeds()).is_ok());
    }

    // 016-FR-008: the fast path pre-populates registration — topic registered,
    // subscribed, the full membership set (self stored, excluded at read —
    // ADR 0038), synced — with no folds.
    #[test]
    fn prepopulation_installs_the_registered_view() {
        let mut population = Population::build(&config(4, 0), &seeds()).expect("valid build");
        population.prepopulate_registration();
        let topic = population.topic().clone();
        let ids = population.peer_ids();
        for (id, participant) in population.participants() {
            assert!(participant.is_synced());
            assert_eq!(participant.subscriptions(), vec![topic.clone()]);
            let mut expected: Vec<_> = ids.iter().filter(|i| *i != id).cloned().collect();
            expected.sort();
            let mut candidates = participant.candidates(&topic);
            candidates.sort();
            assert_eq!(candidates, expected);
        }
    }

    // Labelled sub-seed derivation separates domains and indices.
    #[test]
    fn derived_seeds_separate_domain_and_index() {
        let parent = [5u8; 32];
        assert_eq!(
            derive_seed(&parent, "participant-sampler", 0),
            derive_seed(&parent, "participant-sampler", 0),
        );
        assert_ne!(
            derive_seed(&parent, "participant-sampler", 0),
            derive_seed(&parent, "participant-sampler", 1),
        );
        assert_ne!(
            derive_seed(&parent, "participant-sampler", 0),
            derive_seed(&parent, "churn", 0),
        );
    }
}
