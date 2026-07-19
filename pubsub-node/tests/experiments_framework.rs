//! Integration suite for the experiments framework (feature-gated): exercises
//! the public `pubsub_node::experiments` surface end to end — scripted-
//! topology exactness, driver determinism, and (in later phases) the
//! two-instrument cross-check, artifact byte-diffs, and the smoke variant.
#![cfg(feature = "experiments")]

use std::str::FromStr;

use pubsub_node::experiments::config::parse_sweep_description;
use pubsub_node::experiments::driver::{Driver, RunPlan, RunSeeds, SetupMode};
use pubsub_node::experiments::population::{
    AcceptanceSpec, ConnectionSpec, FanoutSpec, ParticipantClass, Population, PopulationConfig,
    PopulationSeeds, StrategySpec,
};
use pubsub_node::experiments::scripted;
use pubsub_node::experiments::sweep::{
    execute_run_from_seed, execute_run_record, expand_experiments, run_sweep, seed_from_hex,
};
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

fn sweep_toml(size: usize, master_seed: u64, runs: u64) -> String {
    format!(
        r#"
            model = "m2"
            master_seed = {master_seed}

            [population]
            size = {size}
            adversarial = 3
            churn = 0.1
            topic = "t0"

            [strategies.honest]
            connection = "uniform-sampler"
            target_degree = 4
            acceptance = "accept-from-all"
            fanout = "forward-to-all"

            [strategies.adversarial]
            connection = "uniform-sampler"
            target_degree = 4
            acceptance = "accept-from-all"
            fanout = "silent-relay"

            [execution]
            runs_per_experiment = {runs}
        "#
    )
}

// 016-SC-001 (value level, the workhorse): repeated in-memory executions of
// the same (parameters, master seed) yield value-identical records.
#[test]
fn records_are_value_identical_across_executions() {
    let description = parse_sweep_description(&sweep_toml(30, 42, 1)).expect("valid description");
    let params = &expand_experiments(&description)[0];
    let first: Vec<_> = (0..4)
        .map(|run| execute_run_record(params, 0, run, description.master_seed))
        .collect();
    let second: Vec<_> = (0..4)
        .map(|run| execute_run_record(params, 0, run, description.master_seed))
        .collect();
    assert_eq!(first, second);
    // Distinct runs draw distinct seeds (and thus, generically, populations).
    assert_ne!(first[0].seed, first[1].seed);
}

// 016-SC-001 (the artifact-level anchor — the ONE file-level byte diff): a
// tiny sweep written twice produces byte-identical manifest, runs, and
// aggregates files.
#[test]
fn twice_written_sweep_is_byte_identical() {
    let description = parse_sweep_description(&sweep_toml(30, 7, 5)).expect("valid description");
    let dirs = tempfile::tempdir().expect("temp dir");
    let (a, b) = (dirs.path().join("a"), dirs.path().join("b"));
    run_sweep(&description, &a, "test-commit").expect("sweep runs");
    run_sweep(&description, &b, "test-commit").expect("sweep runs");
    for artifact in ["manifest.json", "runs.jsonl", "aggregates.json"] {
        let left = std::fs::read(a.join(artifact)).expect("artifact written");
        let right = std::fs::read(b.join(artifact)).expect("artifact written");
        assert_eq!(left, right, "{artifact} must be byte-identical");
        assert!(!left.is_empty());
    }
}

// 016-SC-004: any run replays exactly from its recorded seed — the record's
// hex seed alone reproduces the record.
#[test]
fn runs_replay_from_their_recorded_seed() {
    let description = parse_sweep_description(&sweep_toml(30, 99, 1)).expect("valid description");
    let params = &expand_experiments(&description)[0];
    let original = execute_run_record(params, 0, 3, description.master_seed);
    let seed = seed_from_hex(&original.seed).expect("recorded seed parses");
    let replayed = execute_run_from_seed(params, 0, 3, seed);
    assert_eq!(original, replayed);
}

// 016-SC-005: run records are population-size-bounded — two runs differing
// only in N at fixed target degree carry near-constant vector lengths, and
// no array field scales with N.
#[test]
fn record_size_is_bounded_by_degree_and_depth_not_population() {
    fn max_array_len(value: &serde_json::Value) -> usize {
        match value {
            serde_json::Value::Array(items) => items
                .iter()
                .map(max_array_len)
                .max()
                .unwrap_or(0)
                .max(items.len()),
            serde_json::Value::Object(fields) => {
                fields.values().map(max_array_len).max().unwrap_or(0)
            }
            _ => 0,
        }
    }

    let small = parse_sweep_description(&sweep_toml(30, 5, 1)).expect("valid description");
    let large = parse_sweep_description(&sweep_toml(90, 5, 1)).expect("valid description");
    let record_small = execute_run_record(&expand_experiments(&small)[0], 0, 0, small.master_seed);
    let record_large = execute_run_record(&expand_experiments(&large)[0], 0, 0, large.master_seed);

    // Histogram lengths are bounded by realised max degree/depth + 1 —
    // near-constant across a 3× population change, nowhere near N.
    let lengths = |record: &pubsub_node::experiments::metrics::RunRecord| {
        [
            record.in_degree_hist.len(),
            record.out_degree_hist.len(),
            record.publishes[0].depth_hist.len(),
        ]
    };
    for (small_len, large_len) in lengths(&record_small)
        .into_iter()
        .zip(lengths(&record_large))
    {
        assert!(small_len < 25 && large_len < 25, "degree/depth-bounded");
        assert!(
            large_len < 2 * small_len + 8,
            "lengths must not scale with N ({small_len} → {large_len})",
        );
    }

    // Structural sweep: NO array anywhere in the larger record reaches the
    // smaller population's size.
    let value: serde_json::Value = serde_json::to_value(&record_large).expect("record serializes");
    assert!(
        max_array_len(&value) < 30,
        "no record field may be sized by the population",
    );
}

// Research R9: the golden serialization test — pins the record encoding and
// the structural field inventory (spellings, order, optional-field
// behaviour). A schema change must consciously update this string.
#[test]
fn run_record_serialization_is_pinned() {
    let record = pubsub_node::experiments::metrics::RunRecord {
        run: 1,
        experiment: 0,
        seed: "abcd".into(),
        honest: 3,
        adversarial: 1,
        down: 1,
        up_honest: 2,
        publisher: scripted::peer(0),
        dial_waves: 2,
        dial_sends: 6,
        rejected_over_capacity: 0,
        good: true,
        min_publisher_coverage: 1.0,
        sinks: 0,
        sccs: 1,
        largest_scc: 2,
        in_degree_hist: vec![0, 2],
        out_degree_hist: vec![0, 2],
        good_pre_churn: Some(true),
        min_publisher_coverage_pre_churn: Some(1.0),
        sinks_pre_churn: Some(0),
        publishes: vec![pubsub_node::experiments::metrics::PublishRecord {
            coverage: 0.5,
            received: 1,
            missed: 1,
            max_depth: 1,
            depth_hist: vec![1, 1],
            miss_causes: pubsub_node::experiments::metrics::MissCauseCounts {
                all_upstreams_adversarial_or_down: 1,
                no_upstream: 0,
                no_up_honest_path: 0,
            },
            sends: pubsub_node::experiments::driver::SendTally {
                honest: 1,
                adversarial: 1,
                down: 1,
            },
            suppressed: 0,
            severed: 0,
        }],
    };
    let json = serde_json::to_string(&record).expect("record serializes");
    assert_eq!(
        json,
        concat!(
            r#"{"run":1,"experiment":0,"seed":"abcd","honest":3,"adversarial":1,"#,
            r#""down":1,"up_honest":2,"publisher":"n000000","dial_waves":2,"dial_sends":6,"#,
            r#""rejected_over_capacity":0,"good":true,"min_publisher_coverage":1.0,"#,
            r#""sinks":0,"sccs":1,"largest_scc":2,"in_degree_hist":[0,2],"#,
            r#""out_degree_hist":[0,2],"good_pre_churn":true,"#,
            r#""min_publisher_coverage_pre_churn":1.0,"sinks_pre_churn":0,"#,
            r#""publishes":[{"coverage":0.5,"received":1,"missed":1,"max_depth":1,"#,
            r#""depth_hist":[1,1],"miss_causes":{"all_upstreams_adversarial_or_down":1,"#,
            r#""no_upstream":0,"no_up_honest_path":0},"sends":{"honest":1,"adversarial":1,"#,
            r#""down":1},"suppressed":0,"severed":0}]}"#,
        ),
    );
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
