//! Statistics conventions and the per-experiment aggregates fold: sparse
//! integer histograms, fixed-width fraction bins, closed-form Wilson 95%
//! intervals, and means/percentiles — folded over run records in canonical
//! run-index order (float summation is not reorder-stable, so the fold
//! order is load-bearing for byte-identical aggregates).
//!
//! Probability estimates always carry their raw counts: `(count, runs, p,
//! wilson95)`. The Wilson score interval has well-defined nonzero width at
//! p̂ ∈ {0, 1} — the all-good sample is the common case — where a plain ±1σ
//! standard error degenerates to zero width; any other convention stays
//! derivable from the counts.
// 016-FR-023; 016-SC-007; research R3/R7; data-model §6; ADR 0033.

use std::collections::BTreeMap;

use serde::Serialize;

use super::metrics::RunRecord;

/// The 97.5% standard-normal quantile: the z of a 95% interval. The level is
/// fixed — a configurable level is a knob without a consumer.
const WILSON_Z: f64 = 1.959_963_984_540_054;

/// Fraction-valued metrics (coverage, min publisher-coverage) are binned at
/// this fixed width — a statistics-module constant, not configuration.
const FRACTION_BIN_WIDTH: f64 = 0.05;

/// A probability estimate as raw counts plus the Wilson 95% interval.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CountEstimate {
    /// Successes.
    pub count: u64,
    /// Trials (runs).
    pub runs: u64,
    /// The point estimate count/runs.
    pub p: f64,
    /// The Wilson score interval at the fixed 95% level: `[low, high]`.
    pub wilson95: [f64; 2],
}

/// Build a count estimate with its Wilson 95% interval (closed form).
///
/// # Panics
///
/// Panics if `runs` is zero or `count > runs` — configurations guarantee at
/// least one run.
#[must_use]
pub fn count_estimate(count: u64, runs: u64) -> CountEstimate {
    assert!(runs > 0, "an estimate needs at least one run");
    assert!(count <= runs, "successes cannot exceed runs");
    #[allow(clippy::cast_precision_loss)] // run counts ≪ 2^52
    let (successes, n) = (count as f64, runs as f64);
    let p = successes / n;
    let z2 = WILSON_Z * WILSON_Z;
    let denominator = 1.0 + z2 / n;
    let centre = (p + z2 / (2.0 * n)) / denominator;
    let half_width = (WILSON_Z / denominator) * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt();
    CountEstimate {
        count,
        runs,
        p,
        wilson95: [
            (centre - half_width).max(0.0),
            (centre + half_width).min(1.0),
        ],
    }
}

/// A sparse integer histogram: value → occurrence count. Only realised
/// values appear, so the map stays degree/depth-bounded.
pub type SparseHistogram = BTreeMap<u64, u64>;

/// Add one observation to a sparse histogram.
pub fn observe(histogram: &mut SparseHistogram, value: u64) {
    *histogram.entry(value).or_insert(0) += 1;
}

/// The fixed-width bin index of a fraction in `[0, 1]`: `floor(f / width)`,
/// with 1.0 landing in the final full bin's successor (index 20 at width
/// 0.05) so exact-full stays distinguishable from almost-full.
#[must_use]
pub fn fraction_bin(fraction: f64) -> u64 {
    debug_assert!((0.0..=1.0).contains(&fraction));
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    // non-negative by the guard; bin count ≤ 21
    {
        (fraction / FRACTION_BIN_WIDTH).floor() as u64
    }
}

/// Nearest-rank percentiles of a series (p50/p90/p99), computed over a
/// sorted copy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Percentiles {
    /// 50th percentile (nearest rank).
    pub p50: f64,
    /// 90th percentile (nearest rank).
    pub p90: f64,
    /// 99th percentile (nearest rank).
    pub p99: f64,
}

fn percentiles(series: &[f64]) -> Percentiles {
    if series.is_empty() {
        return Percentiles::default();
    }
    let mut sorted = series.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("finite metric values"));
    let rank = |percentile: f64| -> f64 {
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        let index = ((percentile / 100.0 * sorted.len() as f64).ceil() as usize).max(1) - 1;
        sorted[index.min(sorted.len() - 1)]
    };
    Percentiles {
        p50: rank(50.0),
        p90: rank(90.0),
        p99: rank(99.0),
    }
}

/// Per-experiment aggregates: a pure fold of the experiment's run records in
/// run-index order.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ExperimentAggregates {
    /// Index into the manifest's experiment list.
    pub experiment: u64,
    /// Runs folded.
    pub runs: u64,
    /// Publish drains folded (runs × publishes per run).
    pub publishes: u64,
    /// P(good topology), post-churn.
    pub good: CountEstimate,
    /// P(every publish of a run reached full coverage).
    pub full_coverage: CountEstimate,
    /// P(good) pre-churn — present iff the experiment drew churn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub good_pre_churn: Option<CountEstimate>,
    /// Coverage per publish, in fixed-width fraction bins.
    pub coverage_hist: SparseHistogram,
    /// Missed-count distribution per publish.
    pub missed_hist: SparseHistogram,
    /// Max-depth distribution per publish.
    pub max_depth_hist: SparseHistogram,
    /// Post-churn sink-count distribution per run.
    pub sinks_hist: SparseHistogram,
    /// Min publisher-coverage per run, in fixed-width fraction bins.
    pub min_publisher_coverage_hist: SparseHistogram,
    /// Element-wise sum of the runs' per-publish depth histograms.
    pub depth_hist_pooled: Vec<u64>,
    /// Mean sends to honest recipients per publish.
    pub sends_honest_mean: f64,
    /// Mean sends to adversarial recipients per publish.
    pub sends_adversarial_mean: f64,
    /// Mean sends into down nodes per publish.
    pub sends_down_mean: f64,
    /// Total-sends percentiles per publish.
    pub sends_total_percentiles: Percentiles,
    /// Mean fraction of arrivals that were redundant:
    /// suppressed / (first receipts via sends + suppressed), per publish
    /// (publishes with no arrivals contribute 0).
    pub duplication_ratio_mean: f64,
}

/// Fold one experiment's run records — **in canonical run-index order** —
/// into its aggregates.
///
/// # Panics
///
/// Panics if `records` is empty, or if the structural invariant
/// `full_coverage.count ≥ good.count` fails — under v1 relays a good
/// topology delivers everything, so a good run that
/// missed coverage means the instruments disagree.
#[must_use]
pub fn fold_aggregates(experiment: u64, records: &[RunRecord]) -> ExperimentAggregates {
    assert!(!records.is_empty(), "an experiment folds at least one run");
    debug_assert!(records.windows(2).all(|pair| pair[0].run < pair[1].run));

    let runs = records.len() as u64;
    let mut good_count = 0u64;
    let mut full_coverage_count = 0u64;
    let mut good_pre_churn_count: Option<u64> = None;
    let mut coverage_hist = SparseHistogram::new();
    let mut missed_hist = SparseHistogram::new();
    let mut max_depth_hist = SparseHistogram::new();
    let mut sinks_hist = SparseHistogram::new();
    let mut min_publisher_coverage_hist = SparseHistogram::new();
    let mut depth_hist_pooled: Vec<u64> = Vec::new();
    let mut publishes = 0u64;
    let mut sends_honest_sum = 0.0f64;
    let mut sends_adversarial_sum = 0.0f64;
    let mut sends_down_sum = 0.0f64;
    let mut sends_totals: Vec<f64> = Vec::new();
    let mut duplication_sum = 0.0f64;

    #[allow(clippy::cast_precision_loss)] // counts ≪ 2^52
    for record in records {
        if record.good {
            good_count += 1;
        }
        if let Some(good_pre) = record.good_pre_churn {
            let counter = good_pre_churn_count.get_or_insert(0);
            if good_pre {
                *counter += 1;
            }
        }
        observe(&mut sinks_hist, record.sinks);
        observe(
            &mut min_publisher_coverage_hist,
            fraction_bin(record.min_publisher_coverage),
        );

        let mut all_full = true;
        for publish in &record.publishes {
            publishes += 1;
            if publish.missed > 0 {
                all_full = false;
            }
            observe(&mut coverage_hist, fraction_bin(publish.coverage));
            observe(&mut missed_hist, publish.missed);
            observe(&mut max_depth_hist, publish.max_depth);
            if depth_hist_pooled.len() < publish.depth_hist.len() {
                depth_hist_pooled.resize(publish.depth_hist.len(), 0);
            }
            for (pooled, &count) in depth_hist_pooled.iter_mut().zip(&publish.depth_hist) {
                *pooled += count;
            }
            sends_honest_sum += publish.sends.honest as f64;
            sends_adversarial_sum += publish.sends.adversarial as f64;
            sends_down_sum += publish.sends.down as f64;
            sends_totals.push(publish.sends.total() as f64);
            let receipts_via_sends =
                publish.sends.total() - publish.suppressed - publish.sends.down;
            let arrivals = receipts_via_sends + publish.suppressed;
            if arrivals > 0 {
                duplication_sum += publish.suppressed as f64 / arrivals as f64;
            }
        }
        if all_full {
            full_coverage_count += 1;
        }
    }

    assert!(
        full_coverage_count >= good_count,
        "structural invariant violated: {good_count} good runs but only \
         {full_coverage_count} fully-covered — a good topology must deliver everything",
    );

    #[allow(clippy::cast_precision_loss)] // counts ≪ 2^52
    let publish_count = publishes as f64;
    ExperimentAggregates {
        experiment,
        runs,
        publishes,
        good: count_estimate(good_count, runs),
        full_coverage: count_estimate(full_coverage_count, runs),
        good_pre_churn: good_pre_churn_count.map(|count| count_estimate(count, runs)),
        coverage_hist,
        missed_hist,
        max_depth_hist,
        sinks_hist,
        min_publisher_coverage_hist,
        depth_hist_pooled,
        sends_honest_mean: sends_honest_sum / publish_count,
        sends_adversarial_mean: sends_adversarial_sum / publish_count,
        sends_down_mean: sends_down_sum / publish_count,
        sends_total_percentiles: percentiles(&sends_totals),
        duplication_ratio_mean: duplication_sum / publish_count,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        count_estimate, fold_aggregates, fraction_bin, observe, percentiles, SparseHistogram,
    };
    use crate::experiments::driver::SendTally;
    use crate::experiments::metrics::{MissCauseCounts, PublishRecord, RunRecord};
    use crate::experiments::scripted::peer;

    fn publish(coverage: f64, missed: u64, sends: SendTally, suppressed: u64) -> PublishRecord {
        PublishRecord {
            coverage,
            received: 0,
            missed,
            max_depth: 1,
            depth_hist: vec![1, 2],
            miss_causes: MissCauseCounts {
                all_upstreams_adversarial_or_down: missed,
                no_upstream: 0,
                no_up_honest_path: 0,
            },
            sends,
            suppressed,
            severed: 0,
        }
    }

    fn record(run: u64, good: bool, missed: u64) -> RunRecord {
        RunRecord {
            run,
            experiment: 0,
            seed: format!("seed-{run}"),
            honest: 4,
            adversarial: 0,
            down: 0,
            up_honest: 4,
            publisher: peer(0),
            dial_waves: 2,
            dial_sends: 12,
            rejected_over_capacity: 0,
            good,
            min_publisher_coverage: if good { 1.0 } else { 0.5 },
            sinks: u64::from(!good),
            sccs: if good { 1 } else { 2 },
            largest_scc: 3,
            in_degree_hist: vec![0, 3],
            out_degree_hist: vec![0, 3],
            good_pre_churn: None,
            min_publisher_coverage_pre_churn: None,
            sinks_pre_churn: None,
            publishes: vec![publish(
                if missed == 0 { 1.0 } else { 0.5 },
                missed,
                SendTally {
                    honest: 6,
                    adversarial: 0,
                    down: 0,
                },
                3,
            )],
        }
    }

    // 016-FR-023 / research R7: Wilson 95% has nonzero width at the all-good
    // sample, where ±1σ degenerates; the interval stays within [0, 1].
    #[test]
    fn wilson_interval_is_nonzero_width_at_all_good() {
        let estimate = count_estimate(20, 20);
        assert!((estimate.p - 1.0).abs() < f64::EPSILON);
        assert!((estimate.wilson95[1] - 1.0).abs() < 1e-12);
        assert!(estimate.wilson95[0] < 1.0);
        assert!(estimate.wilson95[0] > 0.8, "20/20 lower bound ≈ 0.839");

        let none = count_estimate(0, 20);
        assert!(none.wilson95[1] > 0.0);
        assert!(none.wilson95[0].abs() < 1e-12);
    }

    // A known Wilson value: 15/20 at 95% ⇒ [0.5313, 0.8879] (4 dp).
    #[test]
    fn wilson_matches_a_published_value() {
        let estimate = count_estimate(15, 20);
        assert!((estimate.wilson95[0] - 0.5313).abs() < 5e-4);
        assert!((estimate.wilson95[1] - 0.8879).abs() < 5e-4);
    }

    // Sparse histograms hold realised values only.
    #[test]
    fn sparse_histogram_stays_sparse() {
        let mut histogram = SparseHistogram::new();
        observe(&mut histogram, 3);
        observe(&mut histogram, 3);
        observe(&mut histogram, 1_000_000);
        assert_eq!(histogram.len(), 2);
        assert_eq!(histogram[&3], 2);
        assert_eq!(histogram[&1_000_000], 1);
    }

    // Fraction bins are fixed-width with exact-full in its own bin.
    #[test]
    fn fraction_bins_are_fixed_width() {
        assert_eq!(fraction_bin(0.0), 0);
        assert_eq!(fraction_bin(0.04), 0);
        assert_eq!(fraction_bin(0.05), 1);
        assert_eq!(fraction_bin(0.97), 19);
        assert_eq!(fraction_bin(1.0), 20);
    }

    // Nearest-rank percentiles on a known series.
    #[test]
    fn nearest_rank_percentiles() {
        let series: Vec<f64> = (1..=100).map(f64::from).collect();
        let p = percentiles(&series);
        assert!((p.p50 - 50.0).abs() < f64::EPSILON);
        assert!((p.p90 - 90.0).abs() < f64::EPSILON);
        assert!((p.p99 - 99.0).abs() < f64::EPSILON);
    }

    // 016-FR-029: the fold is a pure function of the records — same input,
    // same output — and pools depth histograms element-wise.
    #[test]
    fn fold_is_pure_and_pools_depths() {
        let records = vec![record(0, true, 0), record(1, false, 2), record(2, true, 0)];
        let a = fold_aggregates(0, &records);
        let b = fold_aggregates(0, &records);
        assert_eq!(a, b);
        assert_eq!(a.runs, 3);
        assert_eq!(a.publishes, 3);
        assert_eq!(a.good.count, 2);
        assert_eq!(a.full_coverage.count, 2);
        assert!(a.good_pre_churn.is_none());
        assert_eq!(a.depth_hist_pooled, vec![3, 6]);
        assert!((a.sends_honest_mean - 6.0).abs() < f64::EPSILON);
        // Each publish: 6 sends, 3 suppressed, 0 down ⇒ 3 receipts;
        // duplication = 3/6.
        assert!((a.duplication_ratio_mean - 0.5).abs() < f64::EPSILON);
        assert_eq!(a.coverage_hist[&fraction_bin(1.0)], 2);
        assert_eq!(a.missed_hist[&2], 1);
    }

    // 016-SC-007: the structural invariant full_coverage ≥ good is asserted —
    // a good run that missed someone refuses to fold.
    #[test]
    #[should_panic(expected = "structural invariant")]
    fn good_run_without_full_coverage_refuses_to_fold() {
        let records = vec![record(0, true, 1)];
        let _ = fold_aggregates(0, &records);
    }
}
