//! Deterministic experiments framework.
//!
//! Drives the pure state-transition core under a round-based wavefront
//! scheduler to measure message dissemination over configurable node
//! populations. Available only with the `experiments` cargo feature; the
//! default build is unaffected.
//!
//! - [`driver`] — the wavefront scheduler: canonicalised waves, per-phase
//!   drains, run-phase orchestration.
//! - [`population`] — participants, classes, the seeded population build,
//!   and the two registration setup modes.
//! - [`strategies`] — the experiments-only strategy instances (silent
//!   relay, uniform sampler).
//! - [`scripted`] — declarative scripted-topology builders with
//!   hand-computable metrics (validation support).

pub mod driver;
pub mod population;
pub mod scripted;
pub mod strategies;
