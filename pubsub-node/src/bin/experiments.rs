//! The experiments front end: parse the invocation, load and validate the
//! sweep description, run the sweep, and report progress to stderr.
//!
//! The TOML sweep description is result-affecting and lands in the
//! manifest; everything on this command line (paths) is invocation surface
//! and never reaches the artifacts.

use std::path::PathBuf;
use std::process::Command;

use clap::Parser;

use pubsub_node::experiments::config::parse_sweep_description;
use pubsub_node::experiments::sweep::run_sweep;

/// Run a deterministic dissemination sweep and write its three artifacts.
#[derive(Parser)]
#[command(name = "experiments", version)]
struct Invocation {
    /// Path to the sweep-description TOML file.
    #[arg(long)]
    config: PathBuf,
    /// Output directory for manifest.json, runs.jsonl, and aggregates.json.
    #[arg(long)]
    out: PathBuf,
    /// Worker-pool size: the maximum number of in-flight runs. Each
    /// in-flight run holds a full population in memory, so this is also the
    /// memory knob at large population sizes.
    #[arg(long, default_value_t = default_workers())]
    workers: usize,
}

fn default_workers() -> usize {
    std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get)
}

/// The commit the artifacts should cite: the working tree's HEAD when
/// available, else the crate version (a build outside a checkout).
fn tool_commit() -> String {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|hash| hash.trim().to_string())
        .filter(|hash| !hash.is_empty())
        .unwrap_or_else(|| format!("pubsub-node v{}", env!("CARGO_PKG_VERSION")))
}

fn main() {
    let invocation = Invocation::parse();

    let text = match std::fs::read_to_string(&invocation.config) {
        Ok(text) => text,
        Err(error) => {
            eprintln!(
                "error: cannot read config {}: {error}",
                invocation.config.display(),
            );
            std::process::exit(2);
        }
    };
    let description = match parse_sweep_description(&text) {
        Ok(description) => description,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(2);
        }
    };

    eprintln!(
        "sweep: {} run(s) per experiment, master seed {}",
        description.runs_per_experiment, description.master_seed,
    );
    match run_sweep(
        &description,
        &invocation.out,
        &tool_commit(),
        invocation.workers,
    ) {
        Ok(summary) => {
            eprintln!(
                "done: {} experiment(s), {} run(s) → {}",
                summary.experiments,
                summary.runs,
                invocation.out.display(),
            );
        }
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    }
}
