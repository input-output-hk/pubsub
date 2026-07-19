//! Integration suite for the experiments framework (feature-gated): exercises
//! the public `pubsub_node::experiments` surface end to end — scripted-
//! topology exactness, driver determinism, and (in later phases) the
//! two-instrument cross-check, artifact byte-diffs, and the smoke variant.
#![cfg(feature = "experiments")]

use std::str::FromStr;

use pubsub_node::experiments::driver::{Driver, RunPlan, RunSeeds, SetupMode};
use pubsub_node::experiments::population::{
    AcceptanceSpec, ConnectionSpec, FanoutSpec, ParticipantClass, Population, PopulationConfig,
    PopulationSeeds, StrategySpec,
};
use pubsub_node::experiments::scripted;
use pubsub_node::TopicId;

fn population(size: usize, adversarial: usize) -> Population {
    let honest = StrategySpec {
        connection: ConnectionSpec::UniformSampler { target_degree: 3 },
        acceptance: AcceptanceSpec::accept_from_all(),
        fanout: FanoutSpec::ForwardToAll,
    };
    let config = PopulationConfig {
        topic: TopicId::from_str("t0").expect("valid topic"),
        size,
        adversarial,
        adversarial_strategies: StrategySpec {
            fanout: FanoutSpec::SilentRelay,
            ..honest.clone()
        },
        honest_strategies: honest,
    };
    let seeds = PopulationSeeds {
        keys: [1u8; 32],
        classes: [2u8; 32],
        sampler: [3u8; 32],
    };
    Population::build(&config, &seeds).expect("valid build")
}

// 016-FR-003/FR-005: a full run over real node cores — uniform-sampler dial,
// silent adversaries, churn — executes to exact quiescence through the public
// surface, and every up-honest receiver the topology reaches is recorded.
#[test]
fn full_run_executes_on_real_cores() {
    let mut driver = Driver::new(population(12, 2));
    let plan = RunPlan {
        setup: SetupMode::Prepopulated,
        churn_count: 2,
        publishes_per_run: 1,
    };
    let seeds = RunSeeds {
        churn: [4u8; 32],
        publisher: [5u8; 32],
    };
    let observation = driver.execute_run(&plan, &seeds);

    assert_eq!(observation.down.len(), 2);
    let publisher = driver
        .population()
        .participant(&observation.publisher)
        .expect("publisher in population");
    assert_eq!(publisher.class(), ParticipantClass::Honest);
    assert!(!publisher.is_down());

    let publish = &observation.publishes[0];
    assert_eq!(
        publish.drain.first_receipt.get(&observation.publisher),
        Some(&0)
    );
    // Everyone the drain recorded actually holds the content (driver-owned
    // state, never logs).
    for id in publish.drain.first_receipt.keys() {
        assert!(driver
            .population()
            .participant(id)
            .expect("recorded receiver exists")
            .has_seen(&publish.message));
    }
    // Nothing was sent anywhere after quiescence: the identity's terms are
    // all accounted (full assertion lands with the metrics module).
    assert!(publish.drain.sends.total() >= publish.drain.first_receipt.len() as u64 - 1);
}

// 016-FR-007/FR-024: the same configuration and seeds reproduce the same
// observation, value-for-value, through the public surface.
#[test]
fn runs_replay_exactly_from_their_seeds() {
    let plan = RunPlan {
        setup: SetupMode::Prepopulated,
        churn_count: 1,
        publishes_per_run: 2,
    };
    let seeds = RunSeeds {
        churn: [6u8; 32],
        publisher: [7u8; 32],
    };
    let first = Driver::new(population(10, 2)).execute_run(&plan, &seeds);
    let second = Driver::new(population(10, 2)).execute_run(&plan, &seeds);
    assert_eq!(first, second);
}

// 016-FR-032: scripted-topology exactness is available through the public
// surface (the hand-computable star: hub at wave 1, far leaves at wave 2).
#[test]
fn scripted_star_has_hand_computable_depths() {
    let mut driver = Driver::new(scripted::star(5).build());
    let publisher = scripted::peer(1);
    let outcome = driver.publish_drain(&publisher, 0);
    assert_eq!(
        outcome.drain.first_receipt.get(&scripted::peer(1)),
        Some(&0)
    );
    assert_eq!(
        outcome.drain.first_receipt.get(&scripted::peer(0)),
        Some(&1)
    );
    for leaf in [2, 3, 4] {
        assert_eq!(
            outcome.drain.first_receipt.get(&scripted::peer(leaf)),
            Some(&2),
            "leaf {leaf} receives via the hub",
        );
    }
    assert_eq!(outcome.drain.waves, 2);
}
