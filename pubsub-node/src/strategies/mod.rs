//! The strategy seams and their construction.
//!
//! Everything policy-related lives here: the three injected seams — connection
//! (dial/upstream), acceptance (inbound/downstream), and fan-out (relay) — each
//! a trait in its submodule's `mod.rs` with one file per concrete strategy and a
//! config-facing `*StrategyKind` selector, plus [`config`]: the two-phase
//! construction of the whole set (ADR 0028).
//!
//! Connection *lifecycle state* (`UpstreamState`) is core domain state, not a
//! strategy — it lives in [`crate::connection_state`].

pub mod acceptance;
pub mod config;
pub mod connection;
pub mod edge;
pub mod fanout;
pub mod view;
