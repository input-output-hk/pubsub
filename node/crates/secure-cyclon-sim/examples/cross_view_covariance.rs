//! Run vanilla Cyclon across several network sizes and measure the
//! cross-view covariance after the overlay mixes.
//!
//! Usage:
//!
//! ```text
//! cargo run --release --example cross_view_covariance -- \
//!     --ns 50,100,200,500 --view-len 10 --swap-len 3 \
//!     --warmup 60 --snapshots 20 --gap-cycles 100 \
//!     --seed 42 --csv out.csv
//! ```
//!
//! All flags are optional; defaults match the recommended evaluation
//! configuration. The summary table is printed to stdout in markdown; the
//! optional CSV is suitable for plotting in any external tool.

use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::io::Write;
use std::process::ExitCode;
use std::time::Instant;

use secure_cyclon::NodeId;
use secure_cyclon_sim::stats::{cross_view_covariance, CovarianceStats};
use secure_cyclon_sim::SimBuilder;

#[derive(Debug, Clone)]
struct Args {
    ns: Vec<usize>,
    view_len: usize,
    swap_len: usize,
    warmup: usize,
    snapshots: usize,
    gap_cycles: usize,
    seed: u64,
    csv: Option<String>,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            ns: vec![50, 100, 200, 500],
            view_len: 10,
            swap_len: 3,
            warmup: 60,
            snapshots: 20,
            gap_cycles: 0, // auto = 3 * n / swap_len when zero
            seed: 42,
            csv: None,
        }
    }
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args::default();
    let mut argv = env::args().skip(1);
    while let Some(flag) = argv.next() {
        let value = match argv.next() {
            Some(v) => v,
            None => return Err(format!("missing value for {flag}")),
        };
        match flag.as_str() {
            "--ns" => {
                args.ns = value
                    .split(',')
                    .map(|s| s.trim().parse::<usize>())
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| format!("invalid --ns: {e}"))?;
            }
            "--view-len" => {
                args.view_len = value.parse().map_err(|e| format!("--view-len: {e}"))?
            }
            "--swap-len" => {
                args.swap_len = value.parse().map_err(|e| format!("--swap-len: {e}"))?
            }
            "--warmup" => args.warmup = value.parse().map_err(|e| format!("--warmup: {e}"))?,
            "--snapshots" => {
                args.snapshots = value.parse().map_err(|e| format!("--snapshots: {e}"))?
            }
            "--gap-cycles" => {
                args.gap_cycles = value.parse().map_err(|e| format!("--gap-cycles: {e}"))?
            }
            "--seed" => args.seed = value.parse().map_err(|e| format!("--seed: {e}"))?,
            "--csv" => args.csv = Some(value),
            other => return Err(format!("unknown flag: {other}")),
        }
    }
    if args.ns.is_empty() {
        return Err("--ns must be a non-empty comma-separated list".into());
    }
    Ok(args)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            eprintln!("\nrun with --help for usage (see file header)");
            return ExitCode::from(2);
        }
    };

    let mut rows: Vec<(CovarianceStats, f64)> = Vec::new(); // (stats, elapsed_sec)

    for &n in &args.ns {
        let gap = if args.gap_cycles == 0 {
            (3 * n / args.swap_len.max(1)).max(1)
        } else {
            args.gap_cycles
        };

        let start = Instant::now();
        let sim = SimBuilder::new(n)
            .view_len(args.view_len)
            .swap_len(args.swap_len)
            .seed(args.seed)
            .seeds_per_node(args.view_len.min(n.saturating_sub(1)))
            .build()
            .await;
        sim.bootstrap_all().await.expect("bootstrap should succeed");
        sim.ticks(args.warmup).await;

        let node_ids: Vec<NodeId> = sim.node_ids.clone();
        let mut snapshots: Vec<Vec<HashSet<NodeId>>> = Vec::with_capacity(args.snapshots);
        for _ in 0..args.snapshots {
            snapshots.push(sim.snapshot_views().await);
            sim.ticks(gap).await;
        }

        let stats = cross_view_covariance(&snapshots, &node_ids, args.view_len);
        let elapsed = start.elapsed().as_secs_f64();
        rows.push((stats, elapsed));
    }

    println!(
        "| N | c | snapshots | P_single | c/(N-1) | P_both | Cov | Cov·N² | bound·N² | wall (s) |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|");
    for (s, elapsed) in &rows {
        let marginal = s.view_len as f64 / (s.n - 1) as f64;
        println!(
            "| {} | {} | {} | {:.6} | {:.6} | {:.6} | {:+.3e} | {:+.4} | {:+.4} | {:.2} |",
            s.n,
            s.view_len,
            s.snapshots,
            s.p_single,
            marginal,
            s.p_both,
            s.cov,
            s.cov_scaled,
            s.conservation_bound * (s.n as f64).powi(2),
            elapsed
        );
    }

    if let Some(path) = &args.csv {
        match write_csv(path, &rows) {
            Ok(()) => eprintln!("wrote {path}"),
            Err(e) => {
                eprintln!("failed to write {path}: {e}");
                return ExitCode::from(3);
            }
        }
    }

    ExitCode::SUCCESS
}

fn write_csv(path: &str, rows: &[(CovarianceStats, f64)]) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    writeln!(
        file,
        "n,view_len,snapshots,p_single,marginal_pred,p_both,cov,cov_scaled,conservation_bound,wall_sec"
    )?;
    for (s, elapsed) in rows {
        let marginal = s.view_len as f64 / (s.n - 1) as f64;
        writeln!(
            file,
            "{},{},{},{},{},{},{},{},{},{}",
            s.n,
            s.view_len,
            s.snapshots,
            s.p_single,
            marginal,
            s.p_both,
            s.cov,
            s.cov_scaled,
            s.conservation_bound,
            elapsed
        )?;
    }
    Ok(())
}
