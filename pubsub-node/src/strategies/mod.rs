//! The strategy seams and their construction.
//!
//! Everything policy-related lives here: the four injected seams — connection
//! (relay dial/upstream), acceptance (inbound, one slot per link role),
//! publish (publishing-link targets, ADR 0033), and fan-out (origin-aware
//! forwarding) — each a trait in its submodule's `mod.rs` with one file per
//! concrete strategy and a config-facing `*StrategyKind` selector, plus
//! [`config`]: the two-phase construction of the whole set (ADR 0028).
//!
//! Link *vocabulary and lifecycle state* (`LinkRole`/`LinkDirection`/
//! `LinkState`) is core domain state, not a strategy — it lives in
//! [`crate::connection_state`].

pub mod acceptance;
pub mod config;
pub mod connection;
pub mod edge;
pub mod fanout;
pub mod publish;
#[cfg(test)]
pub(crate) mod test_support;
pub mod view;
