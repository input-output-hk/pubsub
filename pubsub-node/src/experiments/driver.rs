//! The deterministic wavefront driver: steps a [`Population`] of real node
//! cores through a run's phases and observes the outcome from driver-owned
//! state.
//!
//! Round r is the set of in-flight deliveries; applying them yields the
//! sends forming round r+1; a round producing no new sends is quiescence —
//! detected exactly, with no polling, sleeps, or timeouts. Before routing,
//! every wave is stably sorted by a canonical content-derived key
//! (sender, addressee, message identity), so a whole run is a deterministic
//! function of (configuration, seeds) regardless of the core's hash-based
//! collection iteration order. All message kinds route identically —
//! connection control and dissemination — and severance effects are
//! consumed and tallied by the driver.
// 016-FR-003…FR-010, 016-FR-014, 016-FR-027; research R1/R2; ADR 0035.

use std::collections::BTreeMap;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;

use crate::crypto::mock::MockCryptoScheme;
use crate::crypto::{MessageHash, Signer, Timestamp};
use crate::event::Event;
use crate::message::{
    ConnectionAction, Message, MessagePayload, PlainMessage, PublisherId, SignedMessage,
};
use crate::peer::PeerId;
use crate::state::{apply, Effect};

use super::population::{ParticipantClass, Population};

/// One in-flight delivery: a message sent by `from`, addressed to `to`,
/// not yet applied.
#[derive(Debug)]
pub struct Delivery {
    /// The sending peer (the transport frame's sender).
    pub from: PeerId,
    /// The addressed peer.
    pub to: PeerId,
    /// The message in flight.
    pub message: Message,
}

/// A wave: the deliveries of one round, canonicalised before routing.
pub type Wave = Vec<Delivery>;

/// Dissemination sends split by recipient class. `down` is the
/// sent-to-down term of the accounting identity.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub struct SendTally {
    /// Sends addressed to up honest recipients.
    pub honest: u64,
    /// Sends addressed to adversarial recipients.
    pub adversarial: u64,
    /// Sends addressed to down recipients (delivered into the void).
    pub down: u64,
}

impl SendTally {
    /// Total sends across all recipient classes.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.honest + self.adversarial + self.down
    }
}

/// The driver's per-phase observation of one drain.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DrainOutcome {
    /// Number of non-empty waves processed.
    pub waves: u64,
    /// Per-node first-receipt wave for the drained dissemination content
    /// (wave 0 = the publisher's own record). Empty for control-only drains.
    pub first_receipt: BTreeMap<PeerId, u64>,
    /// Sends tallied at emission, split by recipient class.
    pub sends: SendTally,
    /// Deliveries whose content hash the recipient had already seen.
    pub suppressed: u64,
    /// `Misbehaved` effects consumed (signature-failure severances).
    pub severed: u64,
    /// Over-capacity `Rejected` control replies routed.
    pub rejected_over_capacity: u64,
}

/// One publish phase's observation: the published content hash and the
/// dissemination drain it produced.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishOutcome {
    /// Content hash of the published message (coverage is checked against it).
    pub message: MessageHash,
    /// The dissemination drain's observation.
    pub drain: DrainOutcome,
}

/// How the registration phase installs the registered view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetupMode {
    /// Feed membership, topic-registry, and readiness events through the real
    /// fold logic: all registry folds land before any readiness event, and
    /// all readiness events are injected as one wave.
    Faithful,
    /// Write the registered view directly onto state (the fast path).
    Prepopulated,
}

/// A run's phase plan.
#[derive(Clone, Copy, Debug)]
pub struct RunPlan {
    /// Registration setup mode.
    pub setup: SetupMode,
    /// How many honest nodes the churn draw marks down (resolved count).
    pub churn_count: usize,
    /// How many publish phases to run (fresh message each; default 1).
    pub publishes_per_run: u64,
}

/// The seeds a run's driver-owned draws consume (derived from the master
/// seed by the sweep layer).
#[derive(Clone, Copy, Debug)]
pub struct RunSeeds {
    /// The churn draw.
    pub churn: [u8; 32],
    /// The publisher choice.
    pub publisher: [u8; 32],
}

/// Everything a run's phases observed, before metric assembly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunObservation {
    /// The drawn up-honest publisher.
    pub publisher: PeerId,
    /// The peers the churn draw marked down, sorted.
    pub down: Vec<PeerId>,
    /// The registration/dial drain's observation.
    pub dial: DrainOutcome,
    /// One entry per publish phase, in publish order.
    pub publishes: Vec<PublishOutcome>,
}

/// The wavefront driver over one population.
pub struct Driver {
    population: Population,
}

impl Driver {
    /// Take ownership of the population to drive.
    #[must_use]
    pub fn new(population: Population) -> Self {
        Self { population }
    }

    /// Read the driven population.
    #[must_use]
    pub fn population(&self) -> &Population {
        &self.population
    }

    /// Give the population back (post-run inspection).
    #[must_use]
    pub fn into_population(self) -> Population {
        self.population
    }

    /// The registration + dial phase: install the registered view per the
    /// setup mode, then establish the topology by draining the handshake
    /// waves to quiescence against the epoch nonce already on state
    /// (the genesis value).
    ///
    /// Faithful mode applies every registry fold to every node first, then
    /// injects all `Synced` events as one wave — their readiness dials are
    /// collected, not routed, until every node is synced. Prepopulated mode
    /// writes the registered view directly and fires one `Heartbeat` per
    /// node as the dial tick. Either way, the collected dials form wave 1 of
    /// the handshake drain, so every request lands on a synced acceptor.
    pub fn establish(&mut self, mode: SetupMode) -> DrainOutcome {
        let ids = self.population.peer_ids();
        match mode {
            SetupMode::Faithful => {
                for id in &ids {
                    for event in self.population.faithful_registration_events() {
                        let effects = self.apply_to(id, event);
                        debug_assert!(effects.is_empty(), "registry folds emit no effects");
                    }
                }
            }
            SetupMode::Prepopulated => self.population.prepopulate_registration(),
        }

        let mut outcome = DrainOutcome::default();
        let mut wave = Wave::new();
        for id in &ids {
            let event = match mode {
                SetupMode::Faithful => Event::Synced,
                SetupMode::Prepopulated => Event::Heartbeat,
            };
            let effects = self.apply_to(id, event);
            self.collect_effects(id, effects, &mut wave, &mut outcome);
        }
        self.drain(wave, 1, &mut outcome);
        outcome
    }

    /// The churn draw: mark `count` up-honest nodes down,
    /// uniformly from the seed. Generates no events and drains nothing; down
    /// nodes stay registered and present in peers' connection state. Returns
    /// the drawn peers, sorted.
    ///
    /// # Panics
    ///
    /// Panics if the draw would leave fewer than two up-honest nodes (no
    /// publisher/receiver pair) — configurations are validated before a run.
    pub fn churn_draw(&mut self, seed: [u8; 32], count: usize) -> Vec<PeerId> {
        let honest = self.population.up_honest();
        assert!(
            honest.len() >= count + 2,
            "churn draw of {count} from {} up-honest nodes would leave no publisher/receiver pair",
            honest.len(),
        );
        let mut rng = ChaCha20Rng::from_seed(seed);
        let mut down: Vec<PeerId> = rand::seq::index::sample(&mut rng, honest.len(), count)
            .into_iter()
            .map(|index| honest[index].clone())
            .collect();
        down.sort();
        for id in &down {
            self.population
                .participant_mut(id)
                .expect("drawn from the population")
                .mark_down();
        }
        down
    }

    /// Draw the run's publisher uniformly from the up-honest nodes.
    ///
    /// # Panics
    ///
    /// Panics if no up-honest node exists — configurations are validated
    /// before a run.
    #[must_use]
    pub fn draw_publisher(&self, seed: [u8; 32]) -> PeerId {
        let honest = self.population.up_honest();
        assert!(!honest.is_empty(), "publisher draw needs an up-honest node");
        let mut rng = ChaCha20Rng::from_seed(seed);
        honest[rng.gen_range(0..honest.len())].clone()
    }

    /// One publish phase: inject a fresh signed message at the
    /// publisher and drain the dissemination waves to quiescence. Repeated
    /// phases pass distinct `publish_index` values, yielding distinct
    /// content hashes with no state reset.
    pub fn publish_drain(&mut self, publisher: &PeerId, publish_index: u64) -> PublishOutcome {
        let signed = self.published_message(publisher, publish_index);
        let hash = MessageHash::of(&signed.plain);

        let mut outcome = DrainOutcome::default();
        let mut wave = Wave::new();
        let effects = self.apply_to(publisher, Event::Publish(signed));
        if self
            .population
            .participant(publisher)
            .expect("publisher in population")
            .has_seen(&hash)
        {
            outcome.first_receipt.insert(publisher.clone(), 0);
        }
        self.collect_effects(publisher, effects, &mut wave, &mut outcome);
        self.drain(wave, 1, &mut outcome);
        PublishOutcome {
            message: hash,
            drain: outcome,
        }
    }

    /// The full phase sequence of one run: registration/dial → churn draw →
    /// publisher draw → publish drain × `publishes_per_run`.
    pub fn execute_run(&mut self, plan: &RunPlan, seeds: &RunSeeds) -> RunObservation {
        let dial = self.establish(plan.setup);
        let down = self.churn_draw(seeds.churn, plan.churn_count);
        let publisher = self.draw_publisher(seeds.publisher);
        let outcomes = (0..plan.publishes_per_run)
            .map(|index| self.publish_drain(&publisher, index))
            .collect();
        RunObservation {
            publisher,
            down,
            dial,
            publishes: outcomes,
        }
    }

    /// The fresh signed message of publish `publish_index`: signed by the
    /// publisher's own key, content distinct per index (sequence and
    /// payload), timestamp fixed at zero (no wall clock anywhere).
    fn published_message(&self, publisher: &PeerId, publish_index: u64) -> SignedMessage {
        let participant = self
            .population
            .participant(publisher)
            .expect("publisher in population");
        let scheme = MockCryptoScheme::with_seed([0u8; 32]);
        let signer = scheme.signer(participant.key_pair().private.clone());
        let plain = PlainMessage {
            topic: self.population.topic().clone(),
            publisher_id: PublisherId::new(participant.key_pair().public.clone()),
            parent_hash: None,
            sequence: publish_index,
            timestamp: Timestamp::from_millis(0),
            payload: MessagePayload::Ping(publish_index),
        };
        let signature = signer.sign(&plain.signed_bytes());
        SignedMessage { plain, signature }
    }

    /// Apply one event to one participant's core and return its effects.
    fn apply_to(&mut self, id: &PeerId, event: Event) -> Vec<Effect> {
        let participant = self
            .population
            .participant_mut(id)
            .expect("event addressed to a population member");
        apply(participant.state_mut(), event)
    }

    /// Route the collected sends of one node into the next wave, tallying
    /// each send by recipient class at emission. A send to a
    /// down recipient is tallied `sent-to-down` and never enqueued — down
    /// nodes are not stepped. `Misbehaved` effects are consumed and tallied.
    fn collect_effects(
        &self,
        from: &PeerId,
        effects: Vec<Effect>,
        next: &mut Wave,
        outcome: &mut DrainOutcome,
    ) {
        for effect in effects {
            match effect {
                Effect::Send { to, message } => {
                    let recipient = self
                        .population
                        .participant(&to)
                        .expect("send addressed to a population member");
                    if recipient.is_down() {
                        outcome.sends.down += 1;
                        continue;
                    }
                    match recipient.class() {
                        ParticipantClass::Honest => outcome.sends.honest += 1,
                        ParticipantClass::Adversarial => outcome.sends.adversarial += 1,
                    }
                    next.push(Delivery {
                        from: from.clone(),
                        to,
                        message,
                    });
                }
                Effect::Misbehaved { .. } => outcome.severed += 1,
            }
        }
    }

    /// Drain waves to quiescence, starting at `wave_index` for the given
    /// initial wave. Each wave is canonicalised before routing; a wave
    /// producing no new sends ends the drain exactly.
    fn drain(&mut self, mut wave: Wave, mut wave_index: u64, outcome: &mut DrainOutcome) {
        while !wave.is_empty() {
            canonicalise(&mut wave);
            let mut next = Wave::new();
            for Delivery { from, to, message } in wave {
                debug_assert!(
                    !self
                        .population
                        .participant(&to)
                        .expect("delivery to a population member")
                        .is_down(),
                    "down recipients are never enqueued",
                );
                let dissemination_hash = match &message {
                    Message::Dissemination(signed) => Some(MessageHash::of(&signed.plain)),
                    connection => {
                        let (_, control) = connection
                            .connection_parts()
                            .expect("non-dissemination messages are connection control");
                        if matches!(control.plain.action, ConnectionAction::Rejected { .. }) {
                            outcome.rejected_over_capacity += 1;
                        }
                        None
                    }
                };
                let seen_before = dissemination_hash.as_ref().is_some_and(|hash| {
                    self.population
                        .participant(&to)
                        .expect("delivery to a population member")
                        .has_seen(hash)
                });

                let effects = self.apply_to(&to, Event::MessageReceived { from, message });

                if let Some(hash) = dissemination_hash {
                    if seen_before {
                        outcome.suppressed += 1;
                    } else if self
                        .population
                        .participant(&to)
                        .expect("delivery to a population member")
                        .has_seen(&hash)
                    {
                        outcome
                            .first_receipt
                            .entry(to.clone())
                            .or_insert(wave_index);
                    }
                }
                self.collect_effects(&to, effects, &mut next, outcome);
            }
            outcome.waves = wave_index;
            wave = next;
            wave_index += 1;
        }
    }
}

/// Stable-sort a wave by the canonical content key (sender, addressee,
/// message identity): the within-wave tie-break that makes routing order a
/// function of wave *content*, independent of the core's hash-based
/// collection iteration order. This sort is
/// permanent and load-bearing for byte-determinism.
fn canonicalise(wave: &mut Wave) {
    wave.sort_by_cached_key(|delivery| {
        (
            delivery.from.clone(),
            delivery.to.clone(),
            message_key(&delivery.message),
        )
    });
}

/// The canonical identity bytes of a message: a kind tag, the signed-over
/// content bytes, and the signature (distinct signed copies of identical
/// content stay distinguishable, so the order is total in practice — and any
/// remaining ties are byte-identical deliveries, where order cannot matter).
fn message_key(message: &Message) -> Vec<u8> {
    match message {
        Message::Dissemination(signed) => {
            let mut key = vec![0x00];
            key.extend_from_slice(&signed.plain.signed_bytes());
            key.extend_from_slice(signed.signature.as_bytes());
            key
        }
        connection => {
            // All connection variants share one canonical-key namespace: the
            // handshake kind is inside the signed bytes (its preimage tag), so
            // distinct handshakes never collide.
            let (kind, control) = connection
                .connection_parts()
                .expect("non-dissemination messages are connection control");
            let mut key = vec![0x01];
            key.extend_from_slice(&control.plain.signed_bytes(kind));
            key.extend_from_slice(control.signature.as_bytes());
            key
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::{Driver, RunPlan, RunSeeds, SetupMode};
    use crate::connection_state::LinkState;
    use crate::experiments::population::{
        AcceptanceSpec, ConnectionSpec, FanoutSpec, ParticipantClass, Population, PopulationConfig,
        PopulationSeeds, StrategySpec,
    };
    use crate::experiments::scripted;
    use crate::strategies::acceptance::AcceptanceStrategyKind;
    use crate::topic::TopicId;

    fn full_relay_spec() -> StrategySpec {
        StrategySpec {
            connection: ConnectionSpec::connect_to_all(),
            acceptance: AcceptanceSpec::accept_from_all(),
            fanout: FanoutSpec::ForwardToAll,
        }
    }

    fn population(size: usize, adversarial: usize) -> Population {
        let config = PopulationConfig {
            topic: TopicId::from_str("t0").expect("valid topic"),
            size,
            adversarial,
            honest_strategies: full_relay_spec(),
            adversarial_strategies: StrategySpec {
                fanout: FanoutSpec::SilentRelay,
                ..full_relay_spec()
            },
        };
        let seeds = PopulationSeeds {
            keys: [1u8; 32],
            classes: [2u8; 32],
            sampler: [3u8; 32],
        };
        Population::build(&config, &seeds).expect("valid build")
    }

    fn plan(churn_count: usize, publishes_per_run: u64) -> RunPlan {
        RunPlan {
            setup: SetupMode::Prepopulated,
            churn_count,
            publishes_per_run,
        }
    }

    fn seeds() -> RunSeeds {
        RunSeeds {
            churn: [4u8; 32],
            publisher: [5u8; 32],
        }
    }

    // 016-FR-005/FR-016: on a line, the first-receipt wave equals the hop
    // distance from the publisher, and quiescence is exact (no re-forwarding
    // past the ends).
    #[test]
    fn line_first_receipt_equals_distance() {
        let mut driver = Driver::new(scripted::line(4).build());
        let publisher = scripted::peer(0);
        let outcome = driver.publish_drain(&publisher, 0);
        for (index, wave) in [(0usize, 0u64), (1, 1), (2, 2), (3, 3)] {
            assert_eq!(
                outcome.drain.first_receipt.get(&scripted::peer(index)),
                Some(&wave),
                "node {index} first-receipt wave",
            );
        }
        assert_eq!(outcome.drain.waves, 3);
        assert_eq!(outcome.drain.suppressed, 0);
        assert_eq!(outcome.drain.sends.total(), 3);
        assert_eq!(outcome.drain.severed, 0);
    }

    // 016-FR-005/FR-015 (dedup, fire-once): a full mesh floods in one wave;
    // the echo wave is entirely suppressed by content-hash dedup and the
    // drain reaches quiescence exactly.
    #[test]
    fn full_mesh_floods_once_and_dedups_the_echo_wave() {
        let mut driver = Driver::new(scripted::full_mesh(4).build());
        let publisher = scripted::peer(0);
        let outcome = driver.publish_drain(&publisher, 0);
        assert_eq!(outcome.drain.waves, 2);
        assert_eq!(outcome.drain.first_receipt.len(), 4);
        assert!(outcome
            .drain
            .first_receipt
            .iter()
            .all(|(id, wave)| (*id == publisher) == (*wave == 0) && *wave <= 1));
        // Publisher sends 3; each receiver forwards to 2 (split-horizon), all
        // suppressed: 3 + 6 sends, 6 suppressed.
        assert_eq!(outcome.drain.sends.total(), 9);
        assert_eq!(outcome.drain.suppressed, 6);
    }

    // 016-FR-007/FR-027: a whole run is a deterministic function of
    // (configuration, seeds) — two identically-built populations produce
    // value-identical observations.
    #[test]
    fn execute_run_is_deterministic() {
        let run = || {
            let mut driver = Driver::new(population(8, 2));
            driver.execute_run(&plan(1, 2), &seeds())
        };
        assert_eq!(run(), run());
    }

    // 016-FR-008: the faithful mode (folds before readiness, readiness as one
    // wave) and the prepopulated fast path produce the same observable
    // population and the same established topology.
    #[test]
    fn faithful_and_prepopulated_agree() {
        let mut faithful = Driver::new(population(5, 0));
        let mut fast = Driver::new(population(5, 0));
        let faithful_outcome = faithful.establish(SetupMode::Faithful);
        let fast_outcome = fast.establish(SetupMode::Prepopulated);
        assert_eq!(faithful_outcome, fast_outcome);

        let topic = faithful.population().topic().clone();
        for (id, a) in faithful.population().participants() {
            let b = fast.population().participant(id).expect("same identities");
            assert!(a.is_synced() && b.is_synced());
            assert_eq!(a.subscriptions(), b.subscriptions());
            assert_eq!(a.candidates(&topic), b.candidates(&topic));
            assert_eq!(a.upstream(), b.upstream());
            assert_eq!(a.downstream(), b.downstream());
            // Connect-to-all over a synced barrier: every dial must land —
            // a dropped (pre-sync) request would leave a hole here.
            assert_eq!(a.upstream().len(), 4);
            assert_eq!(a.downstream().len(), 4);
        }
    }

    // 016-FR-009: v1 runs never advance the epoch nonce.
    #[test]
    fn epoch_nonce_stays_at_genesis_across_a_run() {
        let mut driver = Driver::new(population(6, 1));
        driver.execute_run(&plan(1, 1), &seeds());
        for (_, participant) in driver.population().participants() {
            assert_eq!(participant.epoch_nonce(), 0);
        }
    }

    // 016-FR-014: the churn draw marks exactly the requested number of honest
    // nodes down, never an adversary, generates no deliveries, and leaves the
    // down nodes registered and present in peers' connection state.
    #[test]
    fn churn_draw_marks_only_honest_nodes_and_generates_no_events() {
        let mut driver = Driver::new(population(8, 2));
        driver.establish(SetupMode::Prepopulated);
        let received_before: Vec<usize> = driver
            .population()
            .participants()
            .map(|(_, p)| p.received_count())
            .collect();

        let down = driver.churn_draw([4u8; 32], 3);
        assert_eq!(down.len(), 3);
        assert_eq!(driver.population().up_honest().len(), 3);
        for id in &down {
            let participant = driver
                .population()
                .participant(id)
                .expect("drawn from population");
            assert!(participant.is_down());
            assert_eq!(participant.class(), ParticipantClass::Honest);
        }
        // No events, no drains: nothing was received anywhere.
        let received_after: Vec<usize> = driver
            .population()
            .participants()
            .map(|(_, p)| p.received_count())
            .collect();
        assert_eq!(received_before, received_after);
        // Down ≠ unregistered: peers still hold the down nodes downstream.
        let topic = driver.population().topic().clone();
        let down_first = down.first().expect("nonempty draw");
        let holders = driver
            .population()
            .participants()
            .filter(|(id, p)| {
                *id != down_first
                    && p.downstream()
                        .contains(&(down_first.clone(), topic.clone()))
            })
            .count();
        assert_eq!(holders, 7);
    }

    // 016-FR-014: down nodes are not stepped and do not relay — sends into
    // them are tallied sent-to-down and their received set never grows.
    #[test]
    fn down_nodes_are_not_stepped_and_relay_nothing() {
        let mut driver = Driver::new(population(4, 0));
        driver.establish(SetupMode::Prepopulated);
        let down = driver.churn_draw([4u8; 32], 1);
        let down_node = down[0].clone();
        let publisher = driver.draw_publisher([5u8; 32]);
        assert_ne!(publisher, down_node);

        let outcome = driver.publish_drain(&publisher, 0);
        let down_participant = driver
            .population()
            .participant(&down_node)
            .expect("down node exists");
        assert_eq!(down_participant.received_count(), 0);
        assert!(!down_participant.has_seen(&outcome.message));
        assert!(!outcome.drain.first_receipt.contains_key(&down_node));
        // Full mesh of 4 with one down: P→{A,B,D} then A→{B,D}, B→{A,D}:
        // 4 honest sends (2 first receipts + 2 suppressed) + 3 into the void.
        assert_eq!(outcome.drain.sends.down, 3);
        assert_eq!(outcome.drain.sends.total(), 7);
        assert_eq!(outcome.drain.suppressed, 2);
    }

    // 016-FR-010: publishes-per-run repeats the phase with fresh messages —
    // distinct content hashes, no state reset between publishes.
    #[test]
    fn publish_repetition_uses_fresh_messages_without_reset() {
        let mut driver = Driver::new(population(4, 0));
        let observation = driver.execute_run(&plan(0, 3), &seeds());
        assert_eq!(observation.publishes.len(), 3);
        let hashes: Vec<_> = observation
            .publishes
            .iter()
            .map(|p| p.message.clone())
            .collect();
        assert!(hashes[0] != hashes[1] && hashes[1] != hashes[2] && hashes[0] != hashes[2]);
        // No reset: every node accumulated all three publishes.
        for (_, participant) in driver.population().participants() {
            assert_eq!(participant.received_count(), 3);
            for hash in &hashes {
                assert!(participant.has_seen(hash));
            }
        }
        // Each drain covered the full mesh.
        for publish in &observation.publishes {
            assert_eq!(publish.drain.first_receipt.len(), 4);
        }
    }

    // 016-FR-006: control messages route through the same machinery — an
    // over-capacity `Rejected` reply is routed back and tallied, and the
    // refused dialer holds no stranded pending upstream.
    #[test]
    fn rejected_replies_are_routed_and_tallied() {
        let config = PopulationConfig {
            topic: TopicId::from_str("t0").expect("valid topic"),
            size: 4,
            adversarial: 0,
            honest_strategies: StrategySpec {
                connection: ConnectionSpec::connect_to_all(),
                acceptance: AcceptanceSpec::Protocol {
                    kind: AcceptanceStrategyKind::Bounded,
                    target_degree: Some(1),
                    bucket_count: None,
                    cap_buffer: 0,
                },
                fanout: FanoutSpec::ForwardToAll,
            },
            adversarial_strategies: full_relay_spec(),
        };
        let seeds = PopulationSeeds {
            keys: [1u8; 32],
            classes: [2u8; 32],
            sampler: [3u8; 32],
        };
        let mut driver = Driver::new(Population::build(&config, &seeds).expect("valid build"));
        let outcome = driver.establish(SetupMode::Prepopulated);
        // Each node dials 3 peers; each acceptor caps at ⌈1 + 0·√1⌉ = 1:
        // 4 accepts land in total, 8 dials are refused with a routed Rejected.
        assert_eq!(outcome.rejected_over_capacity, 8);
        let mut accepted_upstreams = 0;
        for (_, participant) in driver.population().participants() {
            assert_eq!(participant.downstream().len(), 1, "the cap holds");
            let upstream = participant.upstream();
            // A routed Rejected removed every refused pending entry: whatever
            // upstream remains is Active, never a stranded AwaitingAccept.
            assert!(upstream
                .iter()
                .all(|(_, _, state)| *state == LinkState::Active));
            accepted_upstreams += upstream.len();
        }
        assert_eq!(accepted_upstreams, 4);
    }
}
