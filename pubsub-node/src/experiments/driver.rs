//! The deterministic wavefront driver: steps a [`Population`] of real node
//! cores through a run's phases and observes the outcome from driver-owned
//! state.
//!
//! Round r is the set of in-flight deliveries; applying them yields the
//! sends forming round r+1; a round producing no new sends is quiescence —
//! detected exactly, with no polling, sleeps, or timeouts. Before routing,
//! every wave is stably sorted by a canonical key — (addressee index,
//! seeded arrival key, sender index, message identity), where the arrival
//! key is a pure function of (run seed, addressee, sender) and the identity
//! is the content hash computed once at collection for dissemination and
//! the signed bytes for connection control — so a whole run is a
//! deterministic function of (configuration, seeds) regardless of the
//! core's hash-based collection iteration order, while each recipient
//! processes its intra-wave arrivals in its own decorrelated order
//! (ADR 0044). All message kinds route identically — connection control
//! and dissemination — and severance effects are consumed and tallied by
//! the driver.
// 016-FR-003…FR-010, 016-FR-014, 016-FR-027; research R1/R2; ADR 0035/0044.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use sha2::{Digest, Sha256};

use crate::connection_state::LinkKind;
use crate::crypto::mock::MockCryptoScheme;
use crate::crypto::{MessageHash, Signer, Timestamp};
use crate::event::Event;
use crate::message::{
    push_len_prefixed, ConnectionAction, HandshakeKind, Message, MessagePayload, PlainMessage,
    PublisherId, SignedMessage,
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
    /// The message's content hash, computed once when the send is collected
    /// and reused for the wave sort and the suppressed-check. `None` exactly
    /// for connection-control messages — `MessageHash` is content-only and
    /// undefined for them (they keep a signed-bytes sort key).
    pub dissemination_hash: Option<MessageHash>,
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

/// Dissemination sends split by the carrying link's kind, attributed at
/// emission: a send is `relay` iff the sender holds an `Active` relay
/// downstream link for `(topic, recipient)`, and `publisher` otherwise —
/// a recipient reachable over both kinds is attributed to the relay mesh
/// (the deduped single send would have happened over it regardless of the
/// publisher link). Connection-control sends carry no kind; a dial drain
/// leaves this tally zero. Degenerate columns are constant at zero, never
/// absent: relay-only models show `publisher` ≡ 0, and M5's `k_in` = 0
/// boundary row (M1) shows `relay` ≡ 0.
// ADR 0041 (amending ADR 0036's row schema).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub struct KindTally {
    /// Sends carried by relay links.
    pub relay: u64,
    /// Sends carried by publisher links.
    pub publisher: u64,
}

impl KindTally {
    /// Total sends across both link kinds.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.relay + self.publisher
    }
}

/// A node's issued over-capacity refusals, split by the refused
/// dialer's class, with the crossing subset attributed alongside.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RefusalTally {
    /// Refusals issued to honest dialers (each a lost honest link — v1
    /// has no retry).
    pub honest: u64,
    /// Refusals issued to adversarial dialers.
    pub adversarial: u64,
    /// The subset of `honest` refusals that were **crossings** — the
    /// refuser had itself emitted a symmetric dial toward the refused
    /// dialer, so the refusal killed an edge the refuser's own selection
    /// wanted (ADR 0042's veto channel; zero on directional
    /// configurations, where no symmetric dials exist).
    pub crossing_honest: u64,
    /// The subset of `adversarial` refusals that were crossings.
    pub crossing_adversarial: u64,
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
    /// Dissemination sends tallied at emission, split by carrying link kind
    /// (zero for control-only drains).
    pub sends_by_kind: KindTally,
    /// Deliveries whose content hash the recipient had already seen.
    pub suppressed: u64,
    /// `Misbehaved` effects consumed (signature-failure severances).
    pub severed: u64,
    /// Over-capacity `Rejected` control replies routed.
    pub rejected_over_capacity: u64,
    /// Per-node refused dials: refused dialer → routed `Rejected` replies
    /// it received. Populated by connection drains only; the counts sum to
    /// `rejected_over_capacity`.
    pub dials_refused: BTreeMap<PeerId, u64>,
    /// Per-node issued refusals: refusing acceptor → counts split by the
    /// refused dialer's class. Populated by connection drains only.
    pub refusals_issued: BTreeMap<PeerId, RefusalTally>,
    /// Symmetric-handshake dials observed at emission: dialer → the peers
    /// it sent symmetric `Request`s to (down targets included — the dial
    /// was emitted). The drain-time initiation record the direction-erased
    /// end-state cannot provide (N-040): the per-node detail's route
    /// attribution and the refusal crossing split read it. Empty on
    /// directional configurations and for non-connection drains.
    pub symmetric_dials: BTreeMap<PeerId, BTreeSet<PeerId>>,
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
    /// Each peer's rank in the population's canonical (sorted) order — the
    /// wave sort compares these integers instead of cloning `PeerId`s; rank
    /// order and `PeerId` order are the same order by construction.
    index: HashMap<PeerId, usize>,
    /// The run seed the per-victim arrival keys derive from (ADR 0044): each
    /// recipient processes its intra-wave arrivals in an order that is a
    /// pure function of (this seed, recipient, sender), so admission races
    /// at different victims are decorrelated like a real network's
    /// independent delivery orders.
    arrival_seed: [u8; 32],
}

impl Driver {
    /// Take ownership of the population to drive. `arrival_seed` is the run
    /// seed; it feeds only the per-victim arrival keys of the wave sort.
    #[must_use]
    pub fn new(population: Population, arrival_seed: [u8; 32]) -> Self {
        let index = population
            .peer_ids()
            .into_iter()
            .enumerate()
            .map(|(rank, id)| (id, rank))
            .collect();
        Self {
            population,
            index,
            arrival_seed,
        }
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
                    // Drain-time initiation record: a symmetric Request is
                    // the one moment "who dialed" exists — the constructed
                    // link erases it (N-040). Recorded before the down
                    // check: an emitted dial is a dial.
                    if let Some((HandshakeKind::Symmetric, control)) = message.connection_parts() {
                        if matches!(control.plain.action, ConnectionAction::Request { .. }) {
                            outcome
                                .symmetric_dials
                                .entry(from.clone())
                                .or_default()
                                .insert(to.clone());
                        }
                    }
                    let recipient = self
                        .population
                        .participant(&to)
                        .expect("send addressed to a population member");
                    if recipient.is_down() {
                        outcome.sends.down += 1;
                        if matches!(&message, Message::Dissemination(_)) {
                            self.attribute_send_kind(from, &to, outcome);
                        }
                        continue;
                    }
                    match recipient.class() {
                        ParticipantClass::Honest => outcome.sends.honest += 1,
                        ParticipantClass::Adversarial => outcome.sends.adversarial += 1,
                    }
                    // The one hash per delivery: computed here, reused by the
                    // wave sort and the suppressed-check in `drain`.
                    let dissemination_hash = match &message {
                        Message::Dissemination(signed) => Some(MessageHash::of(&signed.plain)),
                        _ => None,
                    };
                    if dissemination_hash.is_some() {
                        self.attribute_send_kind(from, &to, outcome);
                    }
                    next.push(Delivery {
                        from: from.clone(),
                        to,
                        message,
                        dissemination_hash,
                    });
                }
                Effect::Misbehaved { .. } => outcome.severed += 1,
            }
        }
    }

    /// Attribute one dissemination send to its carrying link kind — relay
    /// when the sender holds an `Active` relay downstream link for the
    /// recipient (the both-kinds case included), publisher otherwise.
    ///
    /// Every dissemination send has a carrying link by construction: the
    /// fan-out policies target `Active` downstream links only.
    fn attribute_send_kind(&self, from: &PeerId, to: &PeerId, outcome: &mut DrainOutcome) {
        let sender = self
            .population
            .participant(from)
            .expect("send emitted by a population member");
        let topic = self.population.topic();
        if sender.holds_active_downstream(topic, to, LinkKind::Relay) {
            outcome.sends_by_kind.relay += 1;
        } else {
            debug_assert!(
                sender.holds_active_downstream(topic, to, LinkKind::Publisher),
                "a dissemination send with no carrying downstream link",
            );
            outcome.sends_by_kind.publisher += 1;
        }
    }

    /// Drain waves to quiescence, starting at `wave_index` for the given
    /// initial wave. Each wave is canonicalised before routing; a wave
    /// producing no new sends ends the drain exactly.
    fn drain(&mut self, mut wave: Wave, mut wave_index: u64, outcome: &mut DrainOutcome) {
        while !wave.is_empty() {
            canonicalise(&mut wave, &self.index, &self.arrival_seed);
            let mut next = Wave::new();
            for Delivery {
                from,
                to,
                message,
                dissemination_hash,
            } in wave
            {
                debug_assert!(
                    !self
                        .population
                        .participant(&to)
                        .expect("delivery to a population member")
                        .is_down(),
                    "down recipients are never enqueued",
                );
                if dissemination_hash.is_none() {
                    let (_, control) = message
                        .connection_parts()
                        .expect("non-dissemination messages are connection control");
                    if matches!(control.plain.action, ConnectionAction::Rejected { .. }) {
                        outcome.rejected_over_capacity += 1;
                        // A `Rejected` is routed back to its dialer: `to` is
                        // the refused dialer, `from` the refusing acceptor.
                        *outcome.dials_refused.entry(to.clone()).or_default() += 1;
                        // A crossing: the refuser had itself dialed the
                        // refused dialer. Sound to read here — every
                        // symmetric Request is emitted into the initial
                        // wave, before any Rejected routes.
                        let crossing = outcome
                            .symmetric_dials
                            .get(&from)
                            .is_some_and(|targets| targets.contains(&to));
                        let refused = outcome.refusals_issued.entry(from.clone()).or_default();
                        match self
                            .population
                            .participant(&to)
                            .expect("delivery to a population member")
                            .class()
                        {
                            ParticipantClass::Honest => {
                                refused.honest += 1;
                                if crossing {
                                    refused.crossing_honest += 1;
                                }
                            }
                            ParticipantClass::Adversarial => {
                                refused.adversarial += 1;
                                if crossing {
                                    refused.crossing_adversarial += 1;
                                }
                            }
                        }
                    }
                }
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

/// A message's canonical within-wave identity: the carried content hash for
/// dissemination (variant order keeps dissemination ahead of control within a
/// (sender, addressee) pair, as the old byte tags did), the signed bytes plus
/// signature for connection control.
///
/// The dissemination key is content-only — safe because relays forward
/// verbatim clones, so equal content means an identical signature too, and
/// colliding keys are byte-identical deliveries where order cannot matter.
/// Distinct contents order by hash rather than by raw signed bytes — a
/// legitimate reordering of within-wave ties; the artifacts' values are
/// interleaving-invariant.
#[derive(Eq, Ord, PartialEq, PartialOrd)]
enum MessageIdentity {
    Dissemination([u8; 32]),
    Control(Vec<u8>),
}

/// The hash domain of the per-victim arrival keys (ADR 0044). Instrument
/// randomness, not protocol randomness: nothing a node computes reads it.
const ARRIVAL_ORDER_DOMAIN: &[u8] = b"experiments/arrival-order/v1";

/// The seeded arrival key of one (recipient, sender) pair:
/// `SHA-256(lp(domain) ‖ run_seed ‖ lp(recipient) ‖ lp(sender))` — a pure
/// function of the run seed and the pair, so a recipient's intra-wave order
/// over its senders is fixed for the whole run, independent of every other
/// recipient's order and of the worker count (ADR 0044).
fn arrival_key(seed: &[u8; 32], recipient: &PeerId, sender: &PeerId) -> [u8; 32] {
    let recipient = recipient.as_public_key().as_bytes();
    let sender = sender.as_public_key().as_bytes();
    let mut preimage = Vec::with_capacity(
        ARRIVAL_ORDER_DOMAIN.len() + seed.len() + recipient.len() + sender.len() + 12,
    );
    push_len_prefixed(&mut preimage, ARRIVAL_ORDER_DOMAIN);
    preimage.extend_from_slice(seed);
    push_len_prefixed(&mut preimage, recipient);
    push_len_prefixed(&mut preimage, sender);
    Sha256::digest(&preimage).into()
}

/// Stable-sort a wave by the canonical key (addressee index, seeded arrival
/// key, sender index, message identity): the within-wave order that makes
/// routing a function of wave *content* and the run seed, independent of
/// the core's hash-based collection iteration order. Deliveries group by
/// recipient, and each recipient's arrivals follow its own seeded order
/// over senders — the per-victim decorrelation of ADR 0044 (the retired
/// global (sender, addressee) order coupled every victim's budget race to
/// one rank order, amplifying per-node tails under saturated budgets —
/// N-042). Ties after the sender index are same-pair deliveries, ordered
/// by message identity as before. Index order equals `PeerId` order (the
/// ranks are drawn from the same sorted sequence), so the integer compare
/// is the `PeerId` compare without the clones. This sort is permanent and
/// load-bearing for byte-determinism.
fn canonicalise(wave: &mut Wave, index: &HashMap<PeerId, usize>, arrival_seed: &[u8; 32]) {
    wave.sort_by_cached_key(|delivery| {
        let rank = |id: &PeerId| *index.get(id).expect("delivery endpoints are members");
        (
            rank(&delivery.to),
            arrival_key(arrival_seed, &delivery.to, &delivery.from),
            rank(&delivery.from),
            match &delivery.dissemination_hash {
                Some(hash) => MessageIdentity::Dissemination(*hash.as_bytes()),
                None => MessageIdentity::Control(control_key(&delivery.message)),
            },
        )
    });
}

/// The canonical identity bytes of a connection-control message: the
/// signed-over bytes and the signature. All connection variants share one
/// namespace — the handshake kind is inside the signed bytes (its preimage
/// tag), so distinct handshakes never collide; any remaining ties are
/// byte-identical deliveries, where order cannot matter.
fn control_key(message: &Message) -> Vec<u8> {
    let (kind, control) = message
        .connection_parts()
        .expect("non-dissemination messages are connection control");
    let mut key = control.plain.signed_bytes(kind);
    key.extend_from_slice(control.signature.as_bytes());
    key
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::{arrival_key, canonicalise, Delivery, Driver, RunPlan, RunSeeds, SetupMode, Wave};
    use crate::connection_state::LinkState;
    use crate::crypto::MessageHash;
    use crate::experiments::population::{
        FanoutSpec, ParticipantClass, Population, PopulationConfig, PopulationSeeds, StrategySpec,
    };
    use crate::experiments::scripted;
    use crate::message::Message;
    use crate::peer::PeerId;
    use crate::topic::TopicId;

    fn full_relay_spec() -> StrategySpec {
        StrategySpec::open(FanoutSpec::ForwardToRelays)
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
        let mut driver = Driver::new(scripted::line(4).build(), [0; 32]);
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
        let mut driver = Driver::new(scripted::full_mesh(4).build(), [0; 32]);
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

    // ADR 0044: the wave sort groups deliveries by recipient and orders
    // each recipient's arrivals by its own seeded arrival key — a pure
    // function of (run seed, recipient, sender), decorrelated between
    // recipients.
    #[test]
    fn canonicalise_orders_each_recipient_by_its_seeded_arrival_key() {
        let seed = [7u8; 32];
        let driver = Driver::new(scripted::full_mesh(7).build(), seed);
        let message = driver.published_message(&scripted::peer(0), 0);
        let hash = MessageHash::of(&message.plain);
        let mut wave: Wave = Vec::new();
        for sender in 0..5 {
            for recipient in [5usize, 6] {
                wave.push(Delivery {
                    from: scripted::peer(sender),
                    to: scripted::peer(recipient),
                    message: Message::Dissemination(message.clone()),
                    dissemination_hash: Some(hash.clone()),
                });
            }
        }
        canonicalise(&mut wave, &driver.index, &seed);

        // Recipient blocks are contiguous, in rank order.
        let recipients: Vec<PeerId> = wave.iter().map(|delivery| delivery.to.clone()).collect();
        let mut expected = vec![scripted::peer(5); 5];
        expected.extend(vec![scripted::peer(6); 5]);
        assert_eq!(recipients, expected);

        // Within each block, senders follow that recipient's seeded key
        // order exactly.
        let mut orders: Vec<Vec<PeerId>> = Vec::new();
        for block in wave.chunks(5) {
            let recipient = &block[0].to;
            let mut keyed: Vec<PeerId> = (0..5).map(scripted::peer).collect();
            keyed.sort_by_key(|sender| arrival_key(&seed, recipient, sender));
            let observed: Vec<PeerId> =
                block.iter().map(|delivery| delivery.from.clone()).collect();
            assert_eq!(observed, keyed);
            orders.push(observed);
        }
        // This fixture's two recipients realise different sender orders — a
        // fixed property of the seed, asserted so a regression to any
        // recipient-independent (global) order fails loudly.
        assert_ne!(orders[0], orders[1]);
        // And a different run seed realises a different order for the same
        // recipient (same fixture-pinned rationale).
        let mut reseeded: Vec<PeerId> = (0..5).map(scripted::peer).collect();
        reseeded.sort_by_key(|sender| arrival_key(&[8u8; 32], &scripted::peer(5), sender));
        assert_ne!(orders[0], reseeded);
    }

    // ADR 0044: the arrival key is worker-independent state — rebuilding
    // the same wave in any collection order canonicalises identically.
    #[test]
    fn canonicalise_is_collection_order_independent() {
        let seed = [9u8; 32];
        let driver = Driver::new(scripted::full_mesh(5).build(), seed);
        let message = driver.published_message(&scripted::peer(0), 0);
        let hash = MessageHash::of(&message.plain);
        let build = |pairs: &[(usize, usize)]| -> Wave {
            pairs
                .iter()
                .map(|&(from, to)| Delivery {
                    from: scripted::peer(from),
                    to: scripted::peer(to),
                    message: Message::Dissemination(message.clone()),
                    dissemination_hash: Some(hash.clone()),
                })
                .collect()
        };
        let pairs: Vec<(usize, usize)> = (0..4).flat_map(|s| [(s, 3), (s, 4)]).collect();
        let mut forward = build(&pairs);
        let reversed_pairs: Vec<(usize, usize)> = pairs.iter().rev().copied().collect();
        let mut reversed = build(&reversed_pairs);
        canonicalise(&mut forward, &driver.index, &seed);
        canonicalise(&mut reversed, &driver.index, &seed);
        let order = |wave: &Wave| -> Vec<(PeerId, PeerId)> {
            wave.iter()
                .map(|delivery| (delivery.from.clone(), delivery.to.clone()))
                .collect()
        };
        assert_eq!(order(&forward), order(&reversed));
    }

    // 016-FR-007/FR-027: a whole run is a deterministic function of
    // (configuration, seeds) — two identically-built populations produce
    // value-identical observations.
    #[test]
    fn execute_run_is_deterministic() {
        let run = || {
            let mut driver = Driver::new(population(8, 2), [0; 32]);
            driver.execute_run(&plan(1, 2), &seeds())
        };
        assert_eq!(run(), run());
    }

    // 016-FR-008: the faithful mode (folds before readiness, readiness as one
    // wave) and the prepopulated fast path produce the same observable
    // population and the same established topology.
    #[test]
    fn faithful_and_prepopulated_agree() {
        let mut faithful = Driver::new(population(5, 0), [0; 32]);
        let mut fast = Driver::new(population(5, 0), [0; 32]);
        let faithful_outcome = faithful.establish(SetupMode::Faithful);
        let fast_outcome = fast.establish(SetupMode::Prepopulated);
        assert_eq!(faithful_outcome, fast_outcome);

        let topic = faithful.population().topic().clone();
        for (id, a) in faithful.population().participants() {
            let b = fast.population().participant(id).expect("same identities");
            assert!(a.is_synced() && b.is_synced());
            assert_eq!(a.subscriptions(), b.subscriptions());
            assert_eq!(a.candidates(&topic), b.candidates(&topic));
            // ADR 0038: the modes must agree on the STORED sets too — both
            // hold the node's own id. The self-filtered snapshot above cannot
            // see a divergence on exactly this point.
            assert!(a.candidate_set_contains(&topic, id));
            assert!(b.candidate_set_contains(&topic, id));
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
        let mut driver = Driver::new(population(6, 1), [0; 32]);
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
        let mut driver = Driver::new(population(8, 2), [0; 32]);
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
        let mut driver = Driver::new(population(4, 0), [0; 32]);
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
        let mut driver = Driver::new(population(4, 0), [0; 32]);
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

    // ADR 0041: a population with the publisher pair establishes standing
    // initiation links in the dial drain, and forward-to-all pushes every
    // held message over them. At the k_in = 0 boundary (no relay mesh —
    // the M1 shape) every dissemination send is publisher-attributed: the
    // kind columns invert relative to the relay-only baseline.
    #[test]
    fn publisher_pair_establishes_and_inverts_the_kind_split() {
        let spec = StrategySpec {
            pick_count: Some(0), // k_in = 0: no relay dials
            publisher: Some(crate::experiments::population::PublisherSpec {
                pick_count: Some(2), // k_out = 2
                bucket_count: None,
                accept_cap: None,
                accept_unverified: false,
            }),
            ..StrategySpec::open(FanoutSpec::ForwardToAll)
        };
        let config = PopulationConfig {
            topic: TopicId::from_str("t0").expect("valid topic"),
            size: 5,
            adversarial: 0,
            honest_strategies: spec.clone(),
            adversarial_strategies: spec,
        };
        let seeds = PopulationSeeds {
            keys: [1u8; 32],
            classes: [2u8; 32],
            sampler: [3u8; 32],
        };
        let mut driver = Driver::new(
            Population::build(&config, &seeds).expect("valid build"),
            [0; 32],
        );
        driver.establish(SetupMode::Prepopulated);
        for (_, participant) in driver.population().participants() {
            let publisher_links = participant.publisher_downstream();
            assert_eq!(publisher_links.len(), 2, "exactly k_out dials");
            assert!(publisher_links
                .iter()
                .all(|(_, _, state)| *state == LinkState::Active));
            assert!(
                participant.downstream().is_empty(),
                "no relay links at k_in = 0",
            );
        }

        let publisher = driver.draw_publisher([5u8; 32]);
        let outcome = driver.publish_drain(&publisher, 0);
        assert_eq!(outcome.drain.sends_by_kind.relay, 0);
        assert_eq!(
            outcome.drain.sends_by_kind.publisher,
            outcome.drain.sends.total(),
            "every send carried by a publisher link",
        );
        assert!(
            outcome.drain.sends.total() >= 2,
            "the publisher's own k_out"
        );
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
                accept_cap: Some(1),
                ..StrategySpec::open(FanoutSpec::ForwardToRelays)
            },
            adversarial_strategies: full_relay_spec(),
        };
        let seeds = PopulationSeeds {
            keys: [1u8; 32],
            classes: [2u8; 32],
            sampler: [3u8; 32],
        };
        let mut driver = Driver::new(
            Population::build(&config, &seeds).expect("valid build"),
            [0; 32],
        );
        let outcome = driver.establish(SetupMode::Prepopulated);
        // Each node dials 3 peers; each acceptor's fed cap is 1:
        // 4 accepts land in total, 8 dials are refused with a routed Rejected.
        assert_eq!(outcome.rejected_over_capacity, 8);
        // Per-node attribution: the refused-dial counts sum to the total,
        // and every acceptor refused exactly its two over-cap dials — all
        // honest in this population.
        assert_eq!(outcome.dials_refused.values().sum::<u64>(), 8);
        assert_eq!(outcome.refusals_issued.len(), 4);
        for tally in outcome.refusals_issued.values() {
            assert_eq!(tally.honest, 2, "3 dials received, cap 1, 2 refused");
            assert_eq!(tally.adversarial, 0, "no adversarial dialers exist");
        }
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

    // The refused dialer's class attributes each refusal: with adversarial
    // dialers competing for capped honest slots, the acceptor-side split
    // separates honest starvation from the cap refusing the adversary, and
    // the dialer-side counts agree with the acceptor-side split per class.
    #[test]
    fn refusals_split_by_the_refused_dialers_class() {
        let config = PopulationConfig {
            topic: TopicId::from_str("t0").expect("valid topic"),
            size: 6,
            adversarial: 2,
            honest_strategies: StrategySpec {
                accept_cap: Some(2),
                ..StrategySpec::open(FanoutSpec::ForwardToRelays)
            },
            adversarial_strategies: full_relay_spec(),
        };
        let seeds = PopulationSeeds {
            keys: [4u8; 32],
            classes: [5u8; 32],
            sampler: [6u8; 32],
        };
        let mut driver = Driver::new(
            Population::build(&config, &seeds).expect("valid build"),
            [0; 32],
        );
        let outcome = driver.establish(SetupMode::Prepopulated);
        // Every node dials the other 5. The 4 honest acceptors each receive
        // 5 dials against a cap of 2 → 3 refusals each; the 2 adversarial
        // acceptors are uncapped and refuse nothing.
        assert_eq!(outcome.rejected_over_capacity, 12);
        assert_eq!(outcome.dials_refused.values().sum::<u64>(), 12);
        let mut issued_to_honest = 0;
        let mut issued_to_adversarial = 0;
        for (acceptor, tally) in &outcome.refusals_issued {
            assert_eq!(
                driver
                    .population()
                    .participant(acceptor)
                    .expect("acceptor in population")
                    .class(),
                ParticipantClass::Honest,
                "only capped (honest) acceptors refuse",
            );
            assert_eq!(tally.honest + tally.adversarial, 3, "5 dials, cap 2");
            issued_to_honest += tally.honest;
            issued_to_adversarial += tally.adversarial;
        }
        assert_eq!(issued_to_honest + issued_to_adversarial, 12);
        // Dialer-side counts agree with the acceptor-side class split.
        let refused_of_class = |class: ParticipantClass| -> u64 {
            outcome
                .dials_refused
                .iter()
                .filter(|(id, _)| {
                    driver
                        .population()
                        .participant(id)
                        .expect("dialer in population")
                        .class()
                        == class
                })
                .map(|(_, count)| count)
                .sum()
        };
        assert_eq!(refused_of_class(ParticipantClass::Honest), issued_to_honest);
        assert_eq!(
            refused_of_class(ParticipantClass::Adversarial),
            issued_to_adversarial,
        );
        // Every dialer's accepted + refused dials account for all 5 targets.
        for (id, participant) in driver.population().participants() {
            let refused = outcome.dials_refused.get(id).copied().unwrap_or(0);
            assert_eq!(
                participant.upstream().len() as u64 + refused,
                5,
                "accepted + refused = dialed",
            );
        }
    }

    // ADR 0042 (N-040): the drain records symmetric dials at emission, and
    // crossings are exempt from the admissions budget. Three symmetric
    // ungated no-pick nodes at cap 1: everyone dials everyone, so every
    // inbound request crosses the acceptor's own dial — nothing spends
    // budget, nothing is refused, and the full triangle forms. (At the
    // pre-ADR both-role scan this same fleet lost the rank-1–rank-2 edge
    // to the crossing veto — the contrast the A cell measured.)
    #[test]
    fn symmetric_crossings_are_exempt_and_dials_recorded() {
        let config = PopulationConfig {
            topic: TopicId::from_str("t0").expect("valid topic"),
            size: 3,
            adversarial: 0,
            honest_strategies: StrategySpec {
                accept_cap: Some(1),
                symmetric: true,
                ..StrategySpec::open(FanoutSpec::ForwardToRelays)
            },
            adversarial_strategies: full_relay_spec(),
        };
        let seeds = PopulationSeeds {
            keys: [7u8; 32],
            classes: [8u8; 32],
            sampler: [9u8; 32],
        };
        let mut driver = Driver::new(
            Population::build(&config, &seeds).expect("valid build"),
            [0; 32],
        );
        let outcome = driver.establish(SetupMode::Prepopulated);

        // The initiation record is complete: every node dialed both others.
        assert_eq!(outcome.symmetric_dials.len(), 3);
        for (dialer, targets) in &outcome.symmetric_dials {
            assert_eq!(targets.len(), 2);
            assert!(!targets.contains(dialer));
        }

        // Every request was a crossing: no refusals, the triangle complete.
        assert_eq!(outcome.rejected_over_capacity, 0);
        assert!(outcome.refusals_issued.is_empty());
        for (_, participant) in driver.population().participants() {
            assert_eq!(participant.downstream().len(), 2);
        }
    }

    // ADR 0043: under the ordered comparison predicate the realised
    // symmetric edge set is the union of the two directions' draws — an
    // edge exists iff either end's ordered draw admits the pair (each end
    // dials its own survivors; the acceptor verifies the dialer's
    // direction; the handshake constructs reciprocity as always).
    #[test]
    fn ordered_symmetric_edges_are_the_directional_union() {
        let config = PopulationConfig {
            topic: TopicId::from_str("t0").expect("valid topic"),
            size: 12,
            adversarial: 0,
            honest_strategies: StrategySpec {
                bucket_count: Some(2),
                symmetric: true,
                symmetric_ordered: true,
                ..StrategySpec::open(FanoutSpec::ForwardToRelays)
            },
            adversarial_strategies: full_relay_spec(),
        };
        let seeds = PopulationSeeds {
            keys: [13u8; 32],
            classes: [14u8; 32],
            sampler: [15u8; 32],
        };
        let mut driver = Driver::new(
            Population::build(&config, &seeds).expect("valid build"),
            [0; 32],
        );
        driver.establish(SetupMode::Prepopulated);

        let topic = driver.population().topic().clone();
        let ids: Vec<_> = driver
            .population()
            .participants()
            .map(|(id, _)| id.clone())
            .collect();
        let mut checked = 0;
        for x in &ids {
            for y in &ids {
                if x >= y {
                    continue;
                }
                let expected =
                    crate::strategies::edge::is_valid_edge_sym_ordered(0, &topic, x, y, 2)
                        || crate::strategies::edge::is_valid_edge_sym_ordered(0, &topic, y, x, 2);
                let held = driver
                    .population()
                    .participant(x)
                    .expect("member")
                    .downstream()
                    .iter()
                    .any(|(peer, _)| peer == y);
                assert_eq!(held, expected, "edge {x}–{y}");
                checked += 1;
            }
        }
        assert_eq!(checked, 66, "all pairs checked");
    }

    // ADR 0042: fresh arrivals spend the admissions budget. Honest nodes
    // dial nobody (pick count 0) at budget 1; two adversarial flooders dial
    // every peer. Each honest node receives two FRESH adversarial requests:
    // the first admits, the second is refused — never as a crossing. The
    // flooders' own mutual edge is a crossing and forms freely.
    #[test]
    fn fresh_arrivals_spend_the_admissions_budget() {
        let config = PopulationConfig {
            topic: TopicId::from_str("t0").expect("valid topic"),
            size: 4,
            adversarial: 2,
            honest_strategies: StrategySpec {
                pick_count: Some(0),
                accept_cap: Some(1),
                symmetric: true,
                ..StrategySpec::open(FanoutSpec::ForwardToRelays)
            },
            adversarial_strategies: StrategySpec {
                symmetric: true,
                fanout: FanoutSpec::SilentRelay,
                ..StrategySpec::open(FanoutSpec::SilentRelay)
            },
        };
        let seeds = PopulationSeeds {
            keys: [10u8; 32],
            classes: [11u8; 32],
            sampler: [12u8; 32],
        };
        let mut driver = Driver::new(
            Population::build(&config, &seeds).expect("valid build"),
            [0; 32],
        );
        let outcome = driver.establish(SetupMode::Prepopulated);

        // One refusal per honest node, all fresh — no crossing is ever
        // refused under the budget.
        assert_eq!(outcome.rejected_over_capacity, 2);
        for (acceptor, tally) in &outcome.refusals_issued {
            assert_eq!(
                driver
                    .population()
                    .participant(acceptor)
                    .expect("member")
                    .class(),
                ParticipantClass::Honest,
            );
            assert_eq!(tally.adversarial, 1);
            assert_eq!(tally.honest, 0);
            assert_eq!(tally.crossing_honest + tally.crossing_adversarial, 0);
        }
        for (_, participant) in driver.population().participants() {
            match participant.class() {
                // Budget 1, no own picks: exactly one admitted edge each.
                ParticipantClass::Honest => assert_eq!(participant.downstream().len(), 1),
                // The flooders hold their mutual edge plus whatever the
                // honest budgets admitted (2 slots between them).
                ParticipantClass::Adversarial => assert!(!participant.downstream().is_empty()),
            }
        }
        let adversarial_total: usize = driver
            .population()
            .participants()
            .filter(|(_, p)| p.class() == ParticipantClass::Adversarial)
            .map(|(_, p)| p.downstream().len())
            .sum();
        // 2 (the mutual flooder edge, both ends) + 2 (one admitted slot per
        // honest node).
        assert_eq!(adversarial_total, 4);
    }
}
