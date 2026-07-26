//! Deterministic experiments framework.
//!
//! Drives the pure state-transition core under a round-based wavefront
//! scheduler to measure message dissemination over configurable node
//! populations. Available only with the `experiments` cargo feature; the
//! default build is unaffected.
//!
//! - [`config`] — sweep-description parsing and validation (the loader
//!   layer the front-end binary calls).
//! - [`driver`] — the wavefront scheduler: canonicalised waves, per-phase
//!   drains, run-phase orchestration.
//! - [`graph`] — realised-graph analytics: extraction dispatch, iterative
//!   Kosaraju, condensation, goodness, topology shape.
//! - [`metrics`] — publish-drain measurement and run-record assembly.
//! - [`population`] — participants, classes, the seeded population build,
//!   and the two registration setup modes.
//! - [`statistics`] — histograms, Wilson 95% intervals, and the
//!   per-experiment aggregates fold.
//! - [`sweep`] — manifest, seed derivation, run orchestration, and the
//!   three output artifacts (the only I/O layer).
//! - [`strategies`] — the experiments-only strategy instances (silent
//!   relay, uniform sampler).
//! - [`scripted`] — declarative scripted-topology builders with
//!   hand-computable metrics (validation support).

pub mod config;
pub mod driver;
pub mod graph;
pub mod metrics;
pub mod population;
pub mod scripted;
pub mod statistics;
pub mod strategies;
pub mod sweep;
