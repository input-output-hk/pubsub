//! Publish-drain measurement: coverage, depth, miss-cause classification,
//! send/suppression accounting, and run-record assembly.
//!
//! Every quantity is computed from driver-owned state (the drain outcome,
//! the participants' node cores, the extracted digraph) — never from log
//! output. The accounting identity
//! `sends = first receipts + suppressed + sent-to-down` is asserted on
//! every assembled record: a drop the instrument cannot explain is a bug,
//! not a statistic.
// 016-FR-015…FR-018; data-model §5.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::connection_state::LinkState;
use crate::peer::PeerId;

use super::driver::{DrainOutcome, KindTally, RunObservation, SendTally};
use super::graph::GraphAnalysis;
use super::population::{Participant, Population};

/// Why an eligible receiver missed the message, classified from driver-owned
/// state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MissCause {
    /// Every upstream source the node holds is adversarial or down.
    AllUpstreamsAdversarialOrDown,
    /// The node holds no upstream source at all.
    NoUpstream,
    /// The node has an up-honest upstream, but no up-honest path connects it
    /// to the publisher.
    NoUpHonestPath,
}

/// Miss counts per cause (the `miss_causes` record field).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct MissCauseCounts {
    /// Misses where every upstream source is adversarial or down.
    pub all_upstreams_adversarial_or_down: u64,
    /// Misses with no upstream source at all.
    pub no_upstream: u64,
    /// Misses with an up-honest upstream but no up-honest path.
    pub no_up_honest_path: u64,
}

impl MissCauseCounts {
    fn record(&mut self, cause: MissCause) {
        match cause {
            MissCause::AllUpstreamsAdversarialOrDown => {
                self.all_upstreams_adversarial_or_down += 1;
            }
            MissCause::NoUpstream => self.no_upstream += 1,
            MissCause::NoUpHonestPath => self.no_up_honest_path += 1,
        }
    }

    /// Total misses across all causes.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.all_upstreams_adversarial_or_down + self.no_upstream + self.no_up_honest_path
    }
}

/// One publish phase's measured slice of a run record.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PublishRecord {
    /// Fraction of eligible receivers (up-honest, publisher excluded) whose
    /// received set contains the published message by content hash.
    pub coverage: f64,
    /// Eligible receivers that received the message.
    pub received: u64,
    /// Eligible receivers that missed it.
    pub missed: u64,
    /// Longest first-receipt hop path over up-honest nodes.
    pub max_depth: u64,
    /// Up-honest first receipts per wave (index = wave; wave 0 = the
    /// publisher's own record).
    pub depth_hist: Vec<u64>,
    /// Misses per classified cause.
    pub miss_causes: MissCauseCounts,
    /// Dissemination sends split by recipient class.
    pub sends: SendTally,
    /// Dissemination sends split by carrying link kind (relay wins the
    /// both-kinds case); degenerate columns are zero, never absent.
    pub sends_by_kind: KindTally,
    /// Deliveries suppressed by content-hash dedup at the recipient.
    pub suppressed: u64,
    /// Signature-failure severances consumed during the drain.
    pub severed: u64,
}

/// One run's JSONL row: scalars and
/// degree/depth-bounded vectors only — nothing sized by the population.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RunRecord {
    /// Canonical run index within the sweep.
    pub run: u64,
    /// Index into the manifest's expanded experiment list.
    pub experiment: u64,
    /// The run seed (hex), from which the whole run replays.
    pub seed: String,
    /// Honest participants as drawn.
    pub honest: u64,
    /// Adversarial participants as drawn.
    pub adversarial: u64,
    /// Honest participants the churn draw marked down.
    pub down: u64,
    /// Up-honest participants (honest − down).
    pub up_honest: u64,
    /// The drawn publisher.
    pub publisher: PeerId,
    /// Handshake waves processed by the dial drain.
    pub dial_waves: u64,
    /// Total sends during the dial drain.
    pub dial_sends: u64,
    /// Over-capacity `Rejected` replies routed during the dial drain.
    pub rejected_over_capacity: u64,
    /// Post-churn good-topology verdict (one SCC).
    pub good: bool,
    /// Post-churn worst-case publisher coverage.
    pub min_publisher_coverage: f64,
    /// Post-churn honest sinks (out-degree 0).
    pub sinks: u64,
    /// Post-churn strongly-connected-component count.
    pub sccs: u64,
    /// Post-churn largest component size.
    pub largest_scc: u64,
    /// Post-churn deaf vertices — up-honest nodes the largest component
    /// cannot reach (the in-defect direction; a vertex disconnected in
    /// both directions counts in both classes, unlike the formal
    /// classifier's disjoint third class — subtract the overlap
    /// `deaf + mute − stranded` before joining onto its tables). Zero on
    /// every good graph; with `mute` it classifies the stranded set whose
    /// size is `up_honest − largest_scc`.
    pub deaf: u64,
    /// Post-churn mute vertices — up-honest nodes that cannot reach the
    /// largest component (the out-defect class; the muted publisher is the
    /// canonical case). Raw-digraph classification under every model:
    /// M3's seed rescue shows in `good`, never here.
    pub mute: u64,
    /// Post-churn in-degree histogram (index = degree).
    pub in_degree_hist: Vec<u64>,
    /// Post-churn out-degree histogram (index = degree).
    pub out_degree_hist: Vec<u64>,
    /// Post-churn standing links held per up-honest node (index = count).
    /// Counts connections rather than propagation edges, so unlike the
    /// degree histograms above it includes links that carry no dissemination
    /// traffic — M3's initiation links in particular.
    pub standing_degree_hist: Vec<u64>,
    /// Pre-churn goodness — present iff the run drew churn (absent ≠ false).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub good_pre_churn: Option<bool>,
    /// Pre-churn min publisher coverage — present iff the run drew churn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_publisher_coverage_pre_churn: Option<f64>,
    /// Pre-churn sink count — present iff the run drew churn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sinks_pre_churn: Option<u64>,
    /// One measured slice per publish phase, in publish order.
    pub publishes: Vec<PublishRecord>,
}

/// One node's opt-in dissection row: regenerable exactly from
/// the run's recorded seed, never part of the three default artifacts.
///
/// Degrees are the node's post-churn propagation-digraph degrees, so
/// summing rows reproduces the run record's degree histograms; adversarial
/// and down nodes are not digraph vertices and carry no degrees (absent ≠
/// zero, like every opt-in field here). The connection-accounting columns
/// (serving slots by linked-peer class, refused-dial counts) are defined
/// for every class and always present — their zeros are real zeros.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PerNodeDetail {
    /// Which publish phase the row describes.
    pub publish: u64,
    /// The node.
    pub node: PeerId,
    /// The node's class.
    pub class: super::population::ParticipantClass,
    /// Whether the churn draw marked the node down.
    pub down: bool,
    /// Post-churn digraph in-degree (up-honest vertices only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_degree: Option<u64>,
    /// Post-churn digraph out-degree (up-honest vertices only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub out_degree: Option<u64>,
    /// Relay-kind downstream entries held to honest peers at measure time.
    /// On directional configurations these are the node's granted serving
    /// slots. Under the symmetric handshake reciprocity writes both ends of
    /// every edge into `downstream`, so both roles are counted (≈ 2× the
    /// pick count) — the route columns below split that total by
    /// drain-observed initiation (N-040, ADR 0042). Relay seam only; the
    /// publisher seam has its own pair below, so the kind-agnostic refusal
    /// columns reconcile per seam (N-041, completed).
    pub downstream_honest: u64,
    /// Relay-kind downstream entries held to adversarial peers — on a
    /// capped directional acceptor, capacity the adversary consumed. Same
    /// symmetric-handshake and relay-only caveats as `downstream_honest`
    /// (N-040/N-041).
    pub downstream_adversarial: u64,
    /// Of `downstream_honest`, entries the node alone dialed (a
    /// drain-observed symmetric dial with no crossing dial back): edges
    /// from the node's own picks only. The route columns partition the
    /// both-role totals — own-only + mutual + admitted = downstream, per
    /// class. Zero on directional configurations, where no symmetric dials
    /// exist and every downstream entry is peer-initiated by placement
    /// (N-040, ADR 0042).
    pub edges_own_only_honest: u64,
    /// Of `downstream_adversarial`, entries the node alone dialed — its
    /// own picks that landed on adversarial peers, the admission-free
    /// occupancy route no acceptance policy sees (ADR 0042).
    pub edges_own_only_adversarial: u64,
    /// Of `downstream_honest`, entries both ends dialed (crossings —
    /// budget-exempt under ADR 0042's admissions semantics).
    pub edges_mutual_honest: u64,
    /// Of `downstream_adversarial`, entries both ends dialed.
    pub edges_mutual_adversarial: u64,
    /// Of `downstream_honest`, entries the peer alone dialed: admissions —
    /// what an acceptance cap governs (ADR 0042). On directional
    /// configurations this equals `downstream_honest`.
    pub edges_admitted_honest: u64,
    /// Of `downstream_adversarial`, entries the peer alone dialed. On a
    /// capped acceptor, the adversary's admission-route occupancy.
    pub edges_admitted_adversarial: u64,
    /// **Publisher-kind** downstream entries held `Active` to honest peers
    /// — the node's own accepted seeding links (the dialer is the sender
    /// on this seam, so downstream = the node's seed targets). Completes
    /// the N-041 accounting: publisher-seam refusals in the kind-agnostic
    /// refusal columns now have slot columns to reconcile against.
    pub downstream_publisher_honest: u64,
    /// Publisher-kind downstream entries held `Active` to adversarial
    /// peers (N-041).
    pub downstream_publisher_adversarial: u64,
    /// Routed over-capacity `Rejected` replies this node received for its
    /// own dials in the connection drain (v1 has no retry: each refused
    /// dial is a lost link).
    pub dials_refused: u64,
    /// Over-capacity refusals this node issued to honest dialers.
    pub refusals_issued_honest: u64,
    /// Over-capacity refusals this node issued to adversarial dialers.
    pub refusals_issued_adversarial: u64,
    /// Of `refusals_issued_honest`, refusals of a **crossing** — the node
    /// had itself dialed the refused honest peer, so the refusal killed an
    /// edge its own selection wanted (ADR 0042's veto channel; identically
    /// zero under the admissions-budget semantics and on directional
    /// configurations).
    pub refusals_issued_crossing_honest: u64,
    /// Of `refusals_issued_adversarial`, refusals of a crossing.
    pub refusals_issued_crossing_adversarial: u64,
    /// The wave of the node's first receipt (0 = the publisher's record).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_receipt_wave: Option<u64>,
    /// Who delivered the node's recorded copy: `local` for the publisher's
    /// own record, the delivering peer otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_delivery_origin: Option<String>,
    /// Why the node missed — present only on up-honest non-publishers that
    /// missed the message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub miss_cause: Option<MissCause>,
}

/// One node's relay-kind downstream entries partitioned by drain-observed
/// initiation route; each pair counts (honest, adversarial) linked peers.
#[derive(Clone, Copy, Default)]
struct RouteSlots {
    own_only: (u64, u64),
    mutual: (u64, u64),
    admitted: (u64, u64),
    /// Publisher-kind `Active` downstream, (honest, adversarial) —
    /// N-041's seam-completing pair, outside the relay route partition.
    publisher: (u64, u64),
}

/// A node's `Active` publisher-kind downstream split by the linked peer's
/// class — N-041's seam-completing pair. Active only: a pending dial is
/// not yet a link, and only `Active` entries carry traffic.
fn publisher_slots(participant: &Participant, population: &Population) -> (u64, u64) {
    let mut split = (0u64, 0u64);
    for (peer, _, state) in participant.publisher_downstream() {
        if state != LinkState::Active {
            continue;
        }
        match population
            .participant(&peer)
            .expect("linked peer is a population member")
            .class()
        {
            super::population::ParticipantClass::Honest => split.0 += 1,
            super::population::ParticipantClass::Adversarial => split.1 += 1,
        }
    }
    split
}

/// Assemble the opt-in per-node dissection table for a run: one row per
/// (publish, node), in publish then peer-id order. Pure over
/// the same inputs as the run record — the detail never alters the record.
#[must_use]
pub fn assemble_per_node_detail(
    population: &Population,
    observation: &RunObservation,
    post_churn: &GraphAnalysis,
) -> Vec<PerNodeDetail> {
    let reachable = post_churn.digraph.reachable_from(&observation.publisher);
    // One O(V+E) degree pass shared by every row — a per-node in-degree
    // query would scan the whole adjacency structure each time.
    let (in_degrees, out_degrees) = post_churn.digraph.degree_vectors();
    let degree_of: BTreeMap<&PeerId, (usize, usize)> = post_churn
        .digraph
        .vertices()
        .iter()
        .enumerate()
        .map(|(index, id)| (id, (in_degrees[index], out_degrees[index])))
        .collect();
    // Serving slots split by the linked peer's class and by drain-observed
    // initiation route, one end-state pass — per-run data, repeated
    // verbatim on every publish slice like degrees. Route attribution reads
    // the dial drain's symmetric-dial record: an entry the node alone
    // dialed is own-only, both ends mutual, and everything else admitted —
    // so with no symmetric dials (directional configurations, scripted
    // populations) every entry is admitted, matching directional placement
    // semantics. The routes partition the class totals by construction.
    let dials = &observation.dial.symmetric_dials;
    let dialed = |dialer: &PeerId, target: &PeerId| -> bool {
        dials
            .get(dialer)
            .is_some_and(|targets| targets.contains(target))
    };
    let slots_of: BTreeMap<&PeerId, RouteSlots> = population
        .participants()
        .map(|(id, participant)| {
            let mut slots = RouteSlots::default();
            for (peer, _) in participant.downstream() {
                let class = population
                    .participant(&peer)
                    .expect("linked peer is a population member")
                    .class();
                let honest = class == super::population::ParticipantClass::Honest;
                let route = match (dialed(id, &peer), dialed(&peer, id)) {
                    (true, true) => &mut slots.mutual,
                    (true, false) => &mut slots.own_only,
                    (false, _) => &mut slots.admitted,
                };
                if honest {
                    route.0 += 1;
                } else {
                    route.1 += 1;
                }
            }
            slots.publisher = publisher_slots(participant, population);
            (id, slots)
        })
        .collect();
    let mut rows = Vec::new();
    for (publish_index, publish) in observation.publishes.iter().enumerate() {
        for (id, participant) in population.participants() {
            let degrees = degree_of.get(id);
            let received = participant.has_seen(&publish.message);
            let first_delivery_origin = if received {
                participant
                    .delivery_origin(&publish.message)
                    .map(|origin| match origin {
                        crate::received::Origin::Local => "local".to_string(),
                        crate::received::Origin::Peer(peer) => peer.to_string(),
                    })
            } else {
                None
            };
            let miss_cause =
                (participant.is_up_honest() && id != &observation.publisher && !received)
                    .then(|| classify_miss(id, participant, population, &reachable));
            let slots = slots_of.get(id).copied().unwrap_or_default();
            let refusals = observation.dial.refusals_issued.get(id).copied();
            rows.push(PerNodeDetail {
                publish: publish_index as u64,
                node: id.clone(),
                class: participant.class(),
                down: participant.is_down(),
                in_degree: degrees.map(|&(in_degree, _)| in_degree as u64),
                out_degree: degrees.map(|&(_, out_degree)| out_degree as u64),
                downstream_honest: slots.own_only.0 + slots.mutual.0 + slots.admitted.0,
                downstream_adversarial: slots.own_only.1 + slots.mutual.1 + slots.admitted.1,
                edges_own_only_honest: slots.own_only.0,
                edges_own_only_adversarial: slots.own_only.1,
                edges_mutual_honest: slots.mutual.0,
                edges_mutual_adversarial: slots.mutual.1,
                edges_admitted_honest: slots.admitted.0,
                edges_admitted_adversarial: slots.admitted.1,
                downstream_publisher_honest: slots.publisher.0,
                downstream_publisher_adversarial: slots.publisher.1,
                dials_refused: observation.dial.dials_refused.get(id).copied().unwrap_or(0),
                refusals_issued_honest: refusals.map_or(0, |tally| tally.honest),
                refusals_issued_adversarial: refusals.map_or(0, |tally| tally.adversarial),
                refusals_issued_crossing_honest: refusals.map_or(0, |tally| tally.crossing_honest),
                refusals_issued_crossing_adversarial: refusals
                    .map_or(0, |tally| tally.crossing_adversarial),
                first_receipt_wave: publish.drain.first_receipt.get(id).copied(),
                first_delivery_origin,
                miss_cause,
            });
        }
    }
    rows
}

/// A run's identity within its sweep.
#[derive(Clone, Debug)]
pub struct RunIdentity {
    /// Canonical run index.
    pub run: u64,
    /// Index into the manifest's experiment list.
    pub experiment: u64,
    /// The run seed, hex-encoded.
    pub seed: String,
}

/// Assemble one run record from the run's driver observation and graph
/// passes.
///
/// `pre_churn` carries the formed-topology diagnostic pass — `Some` exactly
/// when the run drew churn; its fields are then present in the record and
/// absent otherwise (absent ≠ zero).
///
/// # Panics
///
/// Panics if a publish drain violates the accounting identity
/// `sends = first receipts + suppressed + sent-to-down`, or if a
/// graph-reachable node missed the drain (the two instruments must agree
/// under all-or-nothing relays) — either is an unexplained inconsistency
/// inside the instrument, never a measurement.
#[must_use]
pub fn assemble_run_record(
    identity: &RunIdentity,
    population: &Population,
    observation: &RunObservation,
    post_churn: &GraphAnalysis,
    pre_churn: Option<&GraphAnalysis>,
) -> RunRecord {
    let mut honest = 0u64;
    let mut adversarial = 0u64;
    let mut down = 0u64;
    for (_, participant) in population.participants() {
        match participant.class() {
            super::population::ParticipantClass::Honest => {
                honest += 1;
                if participant.is_down() {
                    down += 1;
                }
            }
            super::population::ParticipantClass::Adversarial => adversarial += 1,
        }
    }

    let reachable = post_churn.digraph.reachable_from(&observation.publisher);
    let publishes = observation
        .publishes
        .iter()
        .map(|publish| {
            assemble_publish_record(
                population,
                &observation.publisher,
                &publish.message,
                &publish.drain,
                &reachable,
            )
        })
        .collect();

    RunRecord {
        run: identity.run,
        experiment: identity.experiment,
        seed: identity.seed.clone(),
        honest,
        adversarial,
        down,
        up_honest: honest - down,
        publisher: observation.publisher.clone(),
        dial_waves: observation.dial.waves,
        dial_sends: observation.dial.sends.total(),
        rejected_over_capacity: observation.dial.rejected_over_capacity,
        good: post_churn.verdict.good,
        min_publisher_coverage: post_churn.verdict.min_publisher_coverage,
        sinks: post_churn.shape.sinks,
        sccs: post_churn.verdict.sccs,
        largest_scc: post_churn.verdict.largest_scc,
        deaf: post_churn.verdict.deaf,
        mute: post_churn.verdict.mute,
        in_degree_hist: post_churn.shape.in_degree_hist.clone(),
        out_degree_hist: post_churn.shape.out_degree_hist.clone(),
        standing_degree_hist: super::graph::degree_histogram(&super::graph::standing_degrees(
            population,
            super::graph::ChurnPhase::PostChurn,
        )),
        good_pre_churn: pre_churn.map(|pre| pre.verdict.good),
        min_publisher_coverage_pre_churn: pre_churn.map(|pre| pre.verdict.min_publisher_coverage),
        sinks_pre_churn: pre_churn.map(|pre| pre.shape.sinks),
        publishes,
    }
}

/// Measure one publish drain into its record slice, asserting the
/// accounting identity.
fn assemble_publish_record(
    population: &Population,
    publisher: &PeerId,
    message: &crate::crypto::MessageHash,
    drain: &DrainOutcome,
    reachable_from_publisher: &BTreeSet<PeerId>,
) -> PublishRecord {
    let receipts = receipts_via_sends(drain);
    assert!(
        drain.sends.total() == receipts + drain.suppressed + drain.sends.down,
        "accounting identity violated: {} sends ≠ {} first receipts + {} suppressed + {} sent-to-down",
        drain.sends.total(),
        receipts,
        drain.suppressed,
        drain.sends.down,
    );
    // ADR 0041: every dissemination send is attributed to exactly one
    // carrying link kind — a publish drain's sends are all dissemination.
    assert!(
        drain.sends_by_kind.total() == drain.sends.total(),
        "kind attribution violated: {} relay + {} publisher ≠ {} sends",
        drain.sends_by_kind.relay,
        drain.sends_by_kind.publisher,
        drain.sends.total(),
    );

    let mut received = 0u64;
    let mut miss_causes = MissCauseCounts::default();
    let mut depth_hist: Vec<u64> = Vec::new();
    let mut max_depth = 0u64;
    for (id, participant) in population.participants() {
        if !participant.is_up_honest() {
            continue;
        }
        let is_publisher = id == publisher;
        if participant.has_seen(message) {
            if !is_publisher {
                received += 1;
            }
            if let Some(&wave) = drain.first_receipt.get(id) {
                let index = usize::try_from(wave).expect("wave index fits usize");
                if depth_hist.len() <= index {
                    depth_hist.resize(index + 1, 0);
                }
                depth_hist[index] += 1;
                max_depth = max_depth.max(wave);
            }
        } else if !is_publisher {
            miss_causes.record(classify_miss(
                id,
                participant,
                population,
                reachable_from_publisher,
            ));
        }
    }

    let missed = miss_causes.total();
    let eligible = received + missed;
    #[allow(clippy::cast_precision_loss)] // population sizes ≪ 2^52
    let coverage = if eligible == 0 {
        1.0
    } else {
        received as f64 / eligible as f64
    };

    PublishRecord {
        coverage,
        received,
        missed,
        max_depth,
        depth_hist,
        miss_causes,
        sends: drain.sends,
        sends_by_kind: drain.sends_by_kind,
        suppressed: drain.suppressed,
        severed: drain.severed,
    }
}

/// Classify why the up-honest non-publisher `participant` missed the
/// message, from its connection state and the post-churn
/// digraph's publisher reachability.
fn classify_miss(
    id: &PeerId,
    participant: &Participant,
    population: &Population,
    reachable_from_publisher: &BTreeSet<PeerId>,
) -> MissCause {
    // The two-instrument cross-check, always on (release sweeps included):
    // under the all-or-nothing v1 relays, drain coverage must equal graph
    // reachability, so a graph-reachable node that missed the drain is an
    // instrument bug, not a measurement. The reachable set is already
    // materialized once per record, so this costs one set lookup per miss.
    assert!(
        !reachable_from_publisher.contains(id),
        "a graph-reachable node missed the drain — the two instruments disagree",
    );
    let sources: Vec<PeerId> = participant
        .upstream()
        .into_iter()
        .filter(|(_, topic, state)| *state == LinkState::Active && topic == population.topic())
        .map(|(peer, _, _)| peer)
        .collect();
    if sources.is_empty() {
        return MissCause::NoUpstream;
    }
    let all_hostile_or_down = sources.iter().all(|source| {
        population
            .participant(source)
            .map_or(true, |p| !p.is_up_honest())
    });
    if all_hostile_or_down {
        MissCause::AllUpstreamsAdversarialOrDown
    } else {
        MissCause::NoUpHonestPath
    }
}

/// The identity's receipt term: first receipts that arrived via a send
/// (wave ≥ 1 — the publisher's wave-0 record is not a send).
fn receipts_via_sends(outcome: &DrainOutcome) -> u64 {
    outcome
        .first_receipt
        .values()
        .filter(|&&wave| wave >= 1)
        .count() as u64
}

#[cfg(test)]
mod tests {
    use super::{assemble_run_record, MissCauseCounts, RunIdentity};
    use crate::connection_state::LinkState;
    use crate::experiments::driver::{DrainOutcome, Driver, RunObservation, SetupMode};
    use crate::experiments::graph::{ChurnPhase, DisseminationModel};
    use crate::experiments::population::{
        FanoutSpec, ParticipantClass, Population, PopulationConfig, PopulationSeeds, PublisherSpec,
        StrategySpec,
    };
    use crate::experiments::scripted::{self, peer};
    use crate::peer::PeerId;
    use crate::topic::TopicId;

    fn identity() -> RunIdentity {
        RunIdentity {
            run: 0,
            experiment: 0,
            seed: "test-seed".into(),
        }
    }

    /// Drive a publish at `publisher` over an already-linked scripted
    /// population and assemble the record (no dial, no churn draw beyond
    /// what the caller marked).
    fn record_for(
        mut population: Population,
        publisher: &PeerId,
        churned: bool,
    ) -> (super::RunRecord, Population) {
        let mut driver = Driver::new(population, [0; 32]);
        let publish = driver.publish_drain(publisher, 0);
        let observation = RunObservation {
            publisher: publisher.clone(),
            down: driver
                .population()
                .participants()
                .filter(|(_, p)| p.is_down())
                .map(|(id, _)| id.clone())
                .collect(),
            dial: DrainOutcome::default(),
            publishes: vec![publish],
        };
        population = driver.into_population();
        let post = DisseminationModel::M2.analyze(&population, ChurnPhase::PostChurn);
        let pre =
            churned.then(|| DisseminationModel::M2.analyze(&population, ChurnPhase::PreChurn));
        let record =
            assemble_run_record(&identity(), &population, &observation, &post, pre.as_ref());
        (record, population)
    }

    // 016-FR-017: the hand-computed silent-adversary worked example — every
    // miss cause exercised, coverage over the excluded-publisher denominator,
    // sends split, and the accounting identity, all verified by hand:
    //   0 (publisher) → 1 honest        → receives, wave 1
    //   0 → 2 silent                    → receives (adversarial recipient)
    //   2 → 3 honest                    → miss: only upstream adversarial
    //   4 honest, no links              → miss: no upstream
    //   3 → 5 honest                    → miss: up-honest upstream, no path
    //   0 → 6 honest, marked down       → sent-to-down, excluded
    //   6 → 7 honest                    → miss: only upstream down
    #[test]
    fn miss_causes_on_the_silent_adversary_worked_example() {
        let mut population = scripted::nodes(8)
            .silent(2)
            .link(0, 1)
            .link(0, 2)
            .link(2, 3)
            .link(3, 5)
            .link(0, 6)
            .link(6, 7)
            .build();
        population
            .participant_mut(&peer(6))
            .expect("node exists")
            .mark_down();

        let (record, _) = record_for(population, &peer(0), true);

        assert_eq!(record.honest, 7);
        assert_eq!(record.adversarial, 1);
        assert_eq!(record.down, 1);
        assert_eq!(record.up_honest, 6);

        let publish = &record.publishes[0];
        // Eligible receivers: {1, 3, 4, 5, 7} — up-honest minus publisher.
        assert_eq!(publish.received, 1);
        assert_eq!(publish.missed, 4);
        assert!((publish.coverage - 0.2).abs() < 1e-12);
        assert_eq!(
            publish.miss_causes,
            MissCauseCounts {
                all_upstreams_adversarial_or_down: 2, // 3 (adversarial), 7 (down)
                no_upstream: 1,                       // 4
                no_up_honest_path: 1,                 // 5
            },
        );
        // Publisher fans to 1 (honest), 2 (adversarial), 6 (down); nobody
        // else forwards (1 has no downstream; 2 is silent; 6 is not stepped).
        assert_eq!(publish.sends.honest, 1);
        assert_eq!(publish.sends.adversarial, 1);
        assert_eq!(publish.sends.down, 1);
        // All-relay topology: every send relay-attributed, sent-to-down
        // included in the kind identity.
        assert_eq!(publish.sends_by_kind.relay, 3);
        assert_eq!(publish.sends_by_kind.publisher, 0);
        assert_eq!(publish.suppressed, 0);
        // Depth over up-honest: publisher at 0, node 1 at 1.
        assert_eq!(publish.depth_hist, vec![1, 1]);
        assert_eq!(publish.max_depth, 1);
        // Pre-churn fields present (the run drew churn).
        assert!(record.good_pre_churn.is_some());
        assert!(record.sinks_pre_churn.is_some());
    }

    // 016-FR-015: full coverage on a mesh — everyone receives at wave 1, the
    // echo wave is pure suppression, and the identity balances:
    // 9 sends = 3 first receipts (via sends) + 6 suppressed + 0 to down.
    #[test]
    fn full_coverage_mesh_balances_the_identity() {
        let (record, _) = record_for(scripted::full_mesh(4).build(), &peer(0), false);
        let publish = &record.publishes[0];
        assert!((publish.coverage - 1.0).abs() < f64::EPSILON);
        assert_eq!(publish.received, 3);
        assert_eq!(publish.missed, 0);
        assert_eq!(publish.miss_causes.total(), 0);
        assert_eq!(publish.sends.total(), 9);
        assert_eq!(publish.suppressed, 6);
        assert_eq!(publish.depth_hist, vec![1, 3]);
        assert_eq!(publish.max_depth, 1);
        assert!(record.good);
    }

    // ADR 0041: the sends-by-kind split. A publisher with a relay link to 1,
    // a publisher link to 2, and BOTH kinds to 3 publishes under
    // forward-to-relays: the seeding send to 2 is publisher-attributed, the
    // sends to 1 and 3 relay-attributed (relay wins the both-kinds dedup),
    // and the kind identity covers the total.
    #[test]
    fn sends_split_by_carrying_link_kind() {
        let population = scripted::nodes(4)
            .link(0, 1)
            .publisher_link(0, 2)
            .link(0, 3)
            .publisher_link(0, 3)
            .build();
        let (record, _) = record_for(population, &peer(0), false);
        let publish = &record.publishes[0];
        assert_eq!(publish.sends.total(), 3);
        assert_eq!(publish.sends_by_kind.relay, 2);
        assert_eq!(publish.sends_by_kind.publisher, 1);
        // All three targets received it (the kind-agnostic gate admits the
        // publisher-link delivery too).
        assert_eq!(publish.received, 3);
        assert!((publish.coverage - 1.0).abs() < f64::EPSILON);
    }

    // 016-FR-016: depth is the per-node first-receipt wave distribution — on
    // a line, one node per wave and max depth = the line length.
    #[test]
    fn line_depth_distribution() {
        let (record, _) = record_for(scripted::line(4).build(), &peer(0), false);
        let publish = &record.publishes[0];
        assert_eq!(publish.depth_hist, vec![1, 1, 1, 1]);
        assert_eq!(publish.max_depth, 3);
        assert!((publish.coverage - 1.0).abs() < f64::EPSILON);
    }

    // Data-model §5: pre-churn fields are present iff the run drew churn —
    // a churn-free record serializes without the `_pre_churn` keys.
    #[test]
    fn pre_churn_fields_absent_without_churn() {
        let (record, _) = record_for(scripted::full_mesh(3).build(), &peer(0), false);
        assert!(record.good_pre_churn.is_none());
        assert!(record.min_publisher_coverage_pre_churn.is_none());
        assert!(record.sinks_pre_churn.is_none());
        let json = serde_json::to_string(&record).expect("record serializes");
        assert!(!json.contains("pre_churn"));

        let mut churned = scripted::full_mesh(4).build();
        churned
            .participant_mut(&peer(3))
            .expect("node exists")
            .mark_down();
        let (record, _) = record_for(churned, &peer(0), true);
        let json = serde_json::to_string(&record).expect("record serializes");
        assert!(json.contains("good_pre_churn"));
        assert_eq!(record.down, 1);
    }

    // 016-FR-018: an unexplained drop breaks the accounting identity and the
    // assembly refuses to emit the record.
    #[test]
    #[should_panic(expected = "accounting identity")]
    fn identity_violation_panics() {
        let population = scripted::full_mesh(3).build();
        let mut driver = Driver::new(population, [0; 32]);
        let mut publish = driver.publish_drain(&peer(0), 0);
        publish.drain.suppressed += 1; // cook the books
        let observation = RunObservation {
            publisher: peer(0),
            down: Vec::new(),
            dial: DrainOutcome::default(),
            publishes: vec![publish],
        };
        let population = driver.into_population();
        let post = DisseminationModel::M2.analyze(&population, ChurnPhase::PostChurn);
        let _ = assemble_run_record(&identity(), &population, &observation, &post, None);
    }

    // The two-instrument cross-check (016-SC-003): drain coverage ≡ graph
    // reachability — the up-honest nodes holding the content are exactly the
    // publisher's reachable set in the extracted post-churn digraph.
    #[test]
    fn drain_coverage_equals_graph_reachability() {
        let mut population = scripted::nodes(7)
            .silent(2)
            .link(0, 1)
            .link(1, 0)
            .link(0, 2)
            .link(2, 3)
            .link(1, 4)
            .link(4, 5)
            .link(5, 6)
            .build();
        population
            .participant_mut(&peer(5))
            .expect("node exists")
            .mark_down();

        let mut driver = Driver::new(population, [0; 32]);
        let publish = driver.publish_drain(&peer(0), 0);
        let population = driver.into_population();

        let digraph = DisseminationModel::M2.extract(
            &population,
            crate::experiments::graph::ChurnPhase::PostChurn,
        );
        let reachable = digraph.reachable_from(&peer(0));

        let holding: std::collections::BTreeSet<_> = population
            .participants()
            .filter(|(_, p)| p.is_up_honest() && p.has_seen(&publish.message))
            .map(|(id, _)| id.clone())
            .collect();
        assert_eq!(holding, reachable);
        assert!(reachable.contains(&peer(4)), "reached via honest relay");
        assert!(!reachable.contains(&peer(3)), "behind the silent relay");
        assert!(!reachable.contains(&peer(6)), "behind the down relay");
    }

    // ADR 0042 / N-040: the detail's route columns partition each node's
    // relay downstream by drain-observed initiation — own-only (the node
    // alone dialed), mutual (both dialed), admitted (everything else, the
    // scripted/directional fallback) — and the refusal crossing subset
    // lands beside the class totals.
    #[test]
    fn detail_route_columns_partition_downstream_by_observed_dials() {
        let population = scripted::nodes(4)
            .silent(2)
            .link(0, 1)
            .link(0, 2)
            .link(0, 3)
            .build();
        let mut driver = Driver::new(population, [0; 32]);
        let publish = driver.publish_drain(&peer(0), 0);
        let mut dial = DrainOutcome::default();
        dial.symmetric_dials
            .entry(peer(0))
            .or_default()
            .extend([peer(1), peer(2)]);
        dial.symmetric_dials
            .entry(peer(1))
            .or_default()
            .insert(peer(0));
        dial.refusals_issued.insert(
            peer(0),
            crate::experiments::driver::RefusalTally {
                honest: 2,
                adversarial: 1,
                crossing_honest: 1,
                crossing_adversarial: 0,
            },
        );
        let observation = RunObservation {
            publisher: peer(0),
            down: Vec::new(),
            dial,
            publishes: vec![publish],
        };
        let population = driver.into_population();
        let post = DisseminationModel::M2.analyze(&population, ChurnPhase::PostChurn);
        let rows = super::assemble_per_node_detail(&population, &observation, &post);

        let row = |node: &PeerId| rows.iter().find(|r| &r.node == node).expect("row exists");
        let zero = row(&peer(0));
        // Node 0 holds 1 (honest, both dialed → mutual), 2 (adversarial,
        // own dial only), 3 (honest, no observed dial → admitted).
        assert_eq!(zero.edges_mutual_honest, 1);
        assert_eq!(zero.edges_own_only_adversarial, 1);
        assert_eq!(zero.edges_admitted_honest, 1);
        assert_eq!(zero.edges_own_only_honest, 0);
        assert_eq!(zero.edges_mutual_adversarial, 0);
        assert_eq!(zero.edges_admitted_adversarial, 0);
        // The routes partition the class totals.
        assert_eq!(zero.downstream_honest, 2);
        assert_eq!(zero.downstream_adversarial, 1);
        // The refusal crossing subsets ride beside the class split.
        assert_eq!(zero.refusals_issued_honest, 2);
        assert_eq!(zero.refusals_issued_crossing_honest, 1);
        assert_eq!(zero.refusals_issued_adversarial, 1);
        assert_eq!(zero.refusals_issued_crossing_adversarial, 0);
        // With no observed dials, every held entry reads admitted — the
        // directional/scripted fallback — and the partition identity holds
        // on every row.
        for node in [peer(1), peer(2), peer(3)] {
            let r = row(&node);
            assert_eq!(r.edges_own_only_honest + r.edges_own_only_adversarial, 0);
            assert_eq!(r.edges_mutual_honest + r.edges_mutual_adversarial, 0);
            assert_eq!(
                r.edges_admitted_honest + r.edges_admitted_adversarial,
                r.downstream_honest + r.downstream_adversarial,
            );
        }
    }

    // N-041 (completed): the detail's publisher pair counts each node's
    // Active publisher-kind downstream entries split by the linked peer's
    // class — the seam-completing slot columns the kind-agnostic refusal
    // columns reconcile against. Verified against a recount of the
    // participants' own publisher downstream.
    #[test]
    fn detail_publisher_columns_count_active_seed_targets_by_class() {
        use std::str::FromStr;
        let publisher_pair = Some(PublisherSpec {
            pick_count: Some(3),
            bucket_count: None,
            accept_cap: None,
            accept_unverified: false,
        });
        let config = PopulationConfig {
            topic: TopicId::from_str("t0").expect("valid topic"),
            size: 6,
            adversarial: 2,
            honest_strategies: StrategySpec {
                pick_count: Some(2),
                publisher: publisher_pair.clone(),
                ..StrategySpec::open(FanoutSpec::ForwardToRelays)
            },
            adversarial_strategies: StrategySpec {
                pick_count: Some(2),
                publisher: publisher_pair,
                ..StrategySpec::open(FanoutSpec::SilentRelay)
            },
        };
        let seeds = PopulationSeeds {
            keys: [21u8; 32],
            classes: [22u8; 32],
            sampler: [23u8; 32],
        };
        let mut driver = Driver::new(
            Population::build(&config, &seeds).expect("valid build"),
            [0; 32],
        );
        let dial = driver.establish(SetupMode::Prepopulated);
        let publisher = driver
            .population()
            .participants()
            .find(|(_, p)| p.class() == ParticipantClass::Honest)
            .map(|(id, _)| id.clone())
            .expect("an honest node");
        let publish = driver.publish_drain(&publisher, 0);
        let observation = RunObservation {
            publisher,
            down: Vec::new(),
            dial,
            publishes: vec![publish],
        };
        let population = driver.into_population();
        let post = DisseminationModel::M3.analyze(&population, ChurnPhase::PostChurn);
        let rows = super::assemble_per_node_detail(&population, &observation, &post);

        let mut nonzero = 0;
        for row in &rows {
            let participant = population.participant(&row.node).expect("member");
            let (mut honest, mut adversarial) = (0u64, 0u64);
            for (peer, _, state) in participant.publisher_downstream() {
                if state != LinkState::Active {
                    continue;
                }
                match population.participant(&peer).expect("member").class() {
                    ParticipantClass::Honest => honest += 1,
                    ParticipantClass::Adversarial => adversarial += 1,
                }
            }
            assert_eq!(row.downstream_publisher_honest, honest, "node {}", row.node);
            assert_eq!(
                row.downstream_publisher_adversarial, adversarial,
                "node {}",
                row.node
            );
            nonzero += u64::from(honest + adversarial > 0);
        }
        assert!(nonzero > 0, "the publisher seam established links");
    }

    // A publish record embeds the dial outcome's numbers verbatim.
    #[test]
    fn dial_summary_lands_in_the_record() {
        let population = scripted::full_mesh(3).build();
        let mut driver = Driver::new(population, [0; 32]);
        let publish = driver.publish_drain(&peer(0), 0);
        let observation = RunObservation {
            publisher: peer(0),
            down: Vec::new(),
            dial: DrainOutcome {
                waves: 2,
                sends: crate::experiments::driver::SendTally {
                    honest: 12,
                    adversarial: 0,
                    down: 0,
                },
                rejected_over_capacity: 3,
                ..DrainOutcome::default()
            },
            publishes: vec![publish],
        };
        let population = driver.into_population();
        let post = DisseminationModel::M2.analyze(&population, ChurnPhase::PostChurn);
        let record = assemble_run_record(&identity(), &population, &observation, &post, None);
        assert_eq!(record.dial_waves, 2);
        assert_eq!(record.dial_sends, 12);
        assert_eq!(record.rejected_over_capacity, 3);
        assert_eq!(record.run, 0);
        assert_eq!(record.seed, "test-seed");
        assert_eq!(record.publisher, peer(0));
    }
}
