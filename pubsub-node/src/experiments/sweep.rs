//! The sweep layer: manifest construction, seed derivation, run-as-pure-
//! function orchestration, canonical-order JSONL streaming, and aggregates
//! emission — the only layer that performs I/O.
//!
//! A run is a pure function (parameters, run seed) → run record, sharing no
//! state with other runs; run seeds are pre-derived from the master seed
//! and the canonical run index, independent of execution order. Records
//! stream to `runs.jsonl` in run-index order and aggregates fold in that
//! same order, so the three artifacts are byte-identical for the same
//! (description, master seed, tool commit) — an interrupted sweep leaves a
//! valid canonical-order prefix with no completion claim (no resume; re-run).
// 016-FR-024…FR-026, 016-FR-028, 016-FR-029; research R6; ADR 0033;
// contracts/output-artifacts.md.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::topic::TopicId;

use super::config::{StrategyTable, SweepDescription};
use super::driver::{Driver, RunObservation, SetupMode};
use super::graph::{ChurnPhase, DisseminationModel};
use super::metrics::{
    assemble_per_node_detail, assemble_run_record, PerNodeDetail, RunIdentity, RunRecord,
};
use super::population::{derive_seed, Population, PopulationConfig, PopulationSeeds};
use super::statistics::{fold_aggregates, ExperimentAggregates};

/// The recorded seed-derivation rule: embedded in every
/// manifest so a sweep is self-describing.
pub const SEED_DERIVATION_RULE: &str = "run_seed = SHA-256('experiments/run-seed/v1' || \
     master_seed_be8 || run_index_be8); sub_seed(label) = SHA-256(label || run_seed || 0_be8) \
     for labels keys/classes/sampler/churn/publisher; participant sampler seed i = \
     SHA-256('participant-sampler' || sub_seed('sampler') || i_be8)";

/// One resolved experiment: the complete result-affecting parameter set,
/// referenced by index from every run record.
#[derive(Clone, Debug, Serialize)]
pub struct ExperimentParameters {
    /// The dissemination model (serialized by its configuration name).
    #[serde(serialize_with = "serialize_model")]
    pub model: DisseminationModel,
    /// Population size N.
    pub size: usize,
    /// Adversarial count.
    pub adversarial: usize,
    /// Honest nodes marked down per run.
    pub churn_count: usize,
    /// The single topic.
    pub topic: String,
    /// Honest-class strategy configuration.
    pub honest_strategies: StrategyTable,
    /// Adversarial-class strategy configuration.
    pub adversarial_strategies: StrategyTable,
    /// Publish phases per run.
    pub publishes_per_run: u64,
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's serialize_with contract
fn serialize_model<S: serde::Serializer>(
    model: &DisseminationModel,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(model.name())
}

/// The sweep manifest: tool commit, master seed and derivation
/// rule, and the expanded experiment list run records reference by index.
#[derive(Clone, Debug, Serialize)]
pub struct SweepManifest {
    /// The tool commit the artifacts were produced by (the schema pin).
    pub tool_commit: String,
    /// The sweep's master seed.
    pub master_seed: u64,
    /// The seed-derivation rule, verbatim.
    pub seed_derivation: String,
    /// Runs per experiment R.
    pub runs_per_experiment: u64,
    /// The expanded experiment list (index-referenced).
    pub experiments: Vec<ExperimentParameters>,
}

/// Expand a description into its experiment list: the axes' cross-product
/// in declaration order (first-declared axis varying slowest); a
/// description without axes is a single experiment.
///
/// # Panics
///
/// Panics on a description whose grid points do not all validate —
/// descriptions from [`super::config::parse_sweep_description`] are
/// pre-validated.
#[must_use]
pub fn expand_experiments(description: &SweepDescription) -> Vec<ExperimentParameters> {
    description
        .resolved_experiments()
        .expect("descriptions from the parser are pre-validated")
        .into_iter()
        .map(|resolved| ExperimentParameters {
            model: description.model,
            size: resolved.size,
            adversarial: resolved.adversarial,
            churn_count: resolved.churn_count,
            topic: description.topic.as_str().to_string(),
            honest_strategies: resolved.honest_strategies,
            adversarial_strategies: resolved.adversarial_strategies,
            publishes_per_run: resolved.publishes_per_run,
        })
        .collect()
}

/// Build the manifest for a description.
#[must_use]
pub fn build_manifest(description: &SweepDescription, tool_commit: &str) -> SweepManifest {
    SweepManifest {
        tool_commit: tool_commit.to_string(),
        master_seed: description.master_seed,
        seed_derivation: SEED_DERIVATION_RULE.to_string(),
        runs_per_experiment: description.runs_per_experiment,
        experiments: expand_experiments(description),
    }
}

/// Derive the pre-derived run seed of canonical run index `run_index`.
#[must_use]
pub fn run_seed(master_seed: u64, run_index: u64) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"experiments/run-seed/v1");
    hasher.update(master_seed.to_be_bytes());
    hasher.update(run_index.to_be_bytes());
    hasher.finalize().into()
}

/// Hex-encode a run seed for the record's `seed` field.
#[must_use]
pub fn seed_to_hex(seed: &[u8; 32]) -> String {
    use std::fmt::Write;
    seed.iter()
        .fold(String::with_capacity(64), |mut hex, byte| {
            write!(hex, "{byte:02x}").expect("writing to a String cannot fail");
            hex
        })
}

/// Parse a record's hex `seed` field back into a run seed (replay).
#[must_use]
pub fn seed_from_hex(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 {
        return None;
    }
    let mut seed = [0u8; 32];
    for (index, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let text = std::str::from_utf8(chunk).ok()?;
        seed[index] = u8::from_str_radix(text, 16).ok()?;
    }
    Some(seed)
}

/// Execute one run as a pure function of (parameters, run seed) — no I/O,
/// no shared state. `run` and `experiment` are recorded
/// identity only; every random draw stems from `seed`.
#[must_use]
pub fn execute_run_from_seed(
    params: &ExperimentParameters,
    experiment: u64,
    run: u64,
    seed: [u8; 32],
) -> RunRecord {
    execute_run_inner(params, experiment, run, seed, false).0
}

/// Execute one run and its opt-in per-node dissection table:
/// the same pure function as [`execute_run_from_seed`] — the record is
/// identical whether or not detail is requested — plus one detail row per
/// (publish, node).
#[must_use]
pub fn execute_run_and_detail_from_seed(
    params: &ExperimentParameters,
    experiment: u64,
    run: u64,
    seed: [u8; 32],
) -> (RunRecord, Vec<PerNodeDetail>) {
    let (record, detail) = execute_run_inner(params, experiment, run, seed, true);
    (record, detail.expect("detail was requested"))
}

fn execute_run_inner(
    params: &ExperimentParameters,
    experiment: u64,
    run: u64,
    seed: [u8; 32],
    want_detail: bool,
) -> (RunRecord, Option<Vec<PerNodeDetail>>) {
    let population_seeds = PopulationSeeds {
        keys: derive_seed(&seed, "keys", 0),
        classes: derive_seed(&seed, "classes", 0),
        sampler: derive_seed(&seed, "sampler", 0),
    };
    let churn_seed = derive_seed(&seed, "churn", 0);
    let publisher_seed = derive_seed(&seed, "publisher", 0);

    let population_config = PopulationConfig {
        topic: TopicId::from_str(&params.topic).expect("validated topic"),
        size: params.size,
        adversarial: params.adversarial,
        honest_strategies: params
            .honest_strategies
            .to_spec()
            .expect("validated strategy configuration"),
        adversarial_strategies: params
            .adversarial_strategies
            .to_spec()
            .expect("validated strategy configuration"),
    };
    let population =
        Population::build(&population_config, &population_seeds).expect("validated population");

    let mut driver = Driver::new(population);
    let dial = driver.establish(SetupMode::Prepopulated);
    let down = driver.churn_draw(churn_seed, params.churn_count);
    // The pre-churn pass ignores the down marks, so both passes run after
    // the draw — pre-churn is the paired formation diagnostic, present iff
    // the run drew churn.
    let pre_churn = (params.churn_count > 0).then(|| {
        params
            .model
            .analyze(driver.population(), ChurnPhase::PreChurn)
    });
    let post_churn = params
        .model
        .analyze(driver.population(), ChurnPhase::PostChurn);

    let publisher = driver.draw_publisher(publisher_seed);
    let outcomes = (0..params.publishes_per_run)
        .map(|index| driver.publish_drain(&publisher, index))
        .collect();
    let observation = RunObservation {
        publisher,
        down,
        dial,
        publishes: outcomes,
    };

    let record = assemble_run_record(
        &RunIdentity {
            run,
            experiment,
            seed: seed_to_hex(&seed),
        },
        driver.population(),
        &observation,
        &post_churn,
        pre_churn.as_ref(),
    );
    let detail = want_detail
        .then(|| assemble_per_node_detail(driver.population(), &observation, &post_churn));
    (record, detail)
}

/// Execute canonical run `run_index` of a sweep: derive its seed from the
/// master seed and run it.
#[must_use]
pub fn execute_run_record(
    params: &ExperimentParameters,
    experiment: u64,
    run_index: u64,
    master_seed: u64,
) -> RunRecord {
    execute_run_from_seed(
        params,
        experiment,
        run_index,
        run_seed(master_seed, run_index),
    )
}

/// Invocation options for a sweep execution — never result-affecting and
/// never in the manifest.
#[derive(Clone, Copy, Debug)]
pub struct SweepOptions {
    /// Worker-pool size: the maximum in-flight runs (the memory knob).
    pub workers: usize,
    /// Emit the opt-in per-node dissection table per run.
    /// Adds `run-NNNNNN-detail.jsonl` files; the three artifacts are
    /// byte-identical either way.
    pub per_node_detail: bool,
}

impl Default for SweepOptions {
    fn default() -> Self {
        Self {
            workers: 1,
            per_node_detail: false,
        }
    }
}

/// What a completed sweep produced (operator summary).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SweepSummary {
    /// Experiments executed.
    pub experiments: usize,
    /// Total runs executed.
    pub runs: u64,
}

/// A failed sweep execution.
#[derive(Debug, thiserror::Error)]
pub enum SweepError {
    /// An artifact could not be written.
    #[error("failed to write {path}: {source}")]
    Io {
        /// The artifact path.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
}

fn io_error(path: &Path) -> impl FnOnce(std::io::Error) -> SweepError + '_ {
    move |source| SweepError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// The aggregates artifact's top-level shape: one entry per experiment, in
/// experiment-index order — a pure function of the run records.
#[derive(Serialize)]
struct AggregatesArtifact {
    experiments: Vec<ExperimentAggregates>,
}

/// Write one pretty-printed JSON artifact (the manifest / aggregates shape).
fn write_pretty_json<T: Serialize>(path: &Path, value: &T) -> Result<(), SweepError> {
    let json = serde_json::to_string_pretty(value).expect("artifact serializes");
    std::fs::write(path, json + "\n").map_err(io_error(path))
}

/// Write one run's per-node dissection table: JSONL, one row per
/// (publish, node), in publish then peer-id order.
fn write_detail_file(path: &Path, rows: &[PerNodeDetail]) -> Result<(), std::io::Error> {
    let mut writer = BufWriter::new(File::create(path)?);
    for row in rows {
        let line = serde_json::to_string(row).expect("detail row serializes");
        writeln!(writer, "{line}")?;
    }
    writer.flush()
}

/// The worker-shared write-side state: the pre-sized results vector and the
/// in-order streaming cursor. Workers complete runs in any order; the drain
/// after each completion writes every consecutively-ready record, so
/// `runs.jsonl` is always a canonical-order prefix.
struct SweepProgress {
    records: Vec<Option<RunRecord>>,
    next_to_write: usize,
    writer: BufWriter<File>,
    failure: Option<SweepError>,
}

/// Execute a whole sweep and write the three artifacts into `out_dir`:
/// `manifest.json` first, `runs.jsonl` streamed in canonical
/// run-index order, then `aggregates.json` folded in that same order.
///
/// `options.workers` bounds the in-flight runs (each holds a full
/// population — the memory knob); a `std::thread::scope` pool pulls run
/// indices from a shared counter and writes into the pre-sized results
/// vector, so the artifacts are byte-identical at any worker count.
/// With `options.per_node_detail` on, each run's
/// dissection table is written as `run-NNNNNN-detail.jsonl` beside the
/// three artifacts — which stay byte-identical either way.
pub fn run_sweep(
    description: &SweepDescription,
    out_dir: &Path,
    tool_commit: &str,
    options: &SweepOptions,
) -> Result<SweepSummary, SweepError> {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Mutex;

    let workers = options.workers.max(1);
    std::fs::create_dir_all(out_dir).map_err(io_error(out_dir))?;

    let manifest = build_manifest(description, tool_commit);
    write_pretty_json(&out_dir.join("manifest.json"), &manifest)?;

    let runs_path = out_dir.join("runs.jsonl");
    let runs_file = BufWriter::new(File::create(&runs_path).map_err(io_error(&runs_path))?);

    let runs_per_experiment = description.runs_per_experiment;
    let experiments = manifest.experiments.len() as u64;
    let total_runs = experiments * runs_per_experiment;
    let progress = Mutex::new(SweepProgress {
        records: vec![None; usize::try_from(total_runs).expect("run count fits usize")],
        next_to_write: 0,
        writer: runs_file,
        failure: None,
    });
    let next_run = AtomicU64::new(0);
    // Progress cadence: roughly twenty lines per sweep, at least one per
    // experiment, so long single-experiment sweeps stay visibly alive.
    let progress_interval = (total_runs / 20).clamp(1, runs_per_experiment);

    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let run_index = next_run.fetch_add(1, Ordering::Relaxed);
                    if run_index >= total_runs {
                        return;
                    }
                    let experiment = run_index / runs_per_experiment;
                    let params = &manifest.experiments
                        [usize::try_from(experiment).expect("experiment count fits usize")];
                    let seed = run_seed(description.master_seed, run_index);
                    let (record, detail_failure) = if options.per_node_detail {
                        let (record, detail) =
                            execute_run_and_detail_from_seed(params, experiment, run_index, seed);
                        // Detail files are per-run and written by the run's
                        // own worker — independent files need no ordering.
                        let detail_path = out_dir.join(format!("run-{run_index:06}-detail.jsonl"));
                        let failure = write_detail_file(&detail_path, &detail)
                            .map_err(io_error(&detail_path))
                            .err();
                        (record, failure)
                    } else {
                        (
                            execute_run_from_seed(params, experiment, run_index, seed),
                            None,
                        )
                    };

                    let mut guard = progress
                        .lock()
                        .expect("a worker panicked while holding the progress lock");
                    let state = &mut *guard;
                    if let Some(source) = detail_failure {
                        state.failure.get_or_insert(source);
                    }
                    if state.failure.is_some() {
                        return;
                    }
                    state.records[usize::try_from(run_index).expect("run index fits usize")] =
                        Some(record);
                    // Drain everything consecutively ready, in canonical
                    // run-index order.
                    while let Some(Some(ready)) = state.records.get(state.next_to_write) {
                        let line = serde_json::to_string(ready).expect("record serializes");
                        if let Err(source) = writeln!(state.writer, "{line}") {
                            state.failure = Some(io_error(&runs_path)(source));
                            return;
                        }
                        state.next_to_write += 1;
                        if state.next_to_write as u64 % progress_interval == 0 {
                            eprintln!("runs written: {}/{total_runs}", state.next_to_write);
                        }
                    }
                }
            });
        }
    });

    let mut state = progress
        .into_inner()
        .expect("a worker panicked while holding the progress lock");
    if let Some(failure) = state.failure.take() {
        return Err(failure);
    }
    state.writer.flush().map_err(io_error(&runs_path))?;

    let records: Vec<RunRecord> = state
        .records
        .into_iter()
        .map(|record| record.expect("every run index was executed"))
        .collect();
    let aggregates: Vec<ExperimentAggregates> = records
        .chunks(usize::try_from(runs_per_experiment).expect("run count fits usize"))
        .enumerate()
        .map(|(experiment, chunk)| fold_aggregates(experiment as u64, chunk))
        .collect();

    write_pretty_json(
        &out_dir.join("aggregates.json"),
        &AggregatesArtifact {
            experiments: aggregates,
        },
    )?;

    Ok(SweepSummary {
        experiments: manifest.experiments.len(),
        runs: total_runs,
    })
}

#[cfg(test)]
mod tests {
    use super::{run_seed, seed_from_hex, seed_to_hex};

    // 016-FR-024: run seeds are pre-derived — a function of (master seed,
    // run index) alone, independent of execution order.
    #[test]
    fn run_seeds_are_pre_derived_and_distinct() {
        assert_eq!(run_seed(42, 0), run_seed(42, 0));
        assert_ne!(run_seed(42, 0), run_seed(42, 1));
        assert_ne!(run_seed(42, 0), run_seed(43, 0));
    }

    // The record's hex seed field round-trips back into a run seed.
    #[test]
    fn seed_hex_round_trips() {
        let seed = run_seed(7, 3);
        let hex = seed_to_hex(&seed);
        assert_eq!(hex.len(), 64);
        assert_eq!(seed_from_hex(&hex), Some(seed));
        assert_eq!(seed_from_hex("zz"), None);
        assert_eq!(seed_from_hex(&hex[..62]), None);
    }
}
