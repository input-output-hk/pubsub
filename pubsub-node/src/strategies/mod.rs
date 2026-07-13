//! The strategy seams and their construction.
//!
//! Everything policy-related lives here: the three seam *families* — link
//! selection ([`selection`], one instance per role slot), inbound acceptance
//! ([`acceptance`], one instance per role slot), and origin-aware fan-out
//! ([`fanout`], the dissemination-model knob — ADR 0034) — each a trait in
//! its submodule's `mod.rs` with one file per concrete strategy and a
//! config-facing kind selector, plus [`config`]: the two-phase construction
//! of the whole set (ADR 0028).
//!
//! Link *vocabulary and lifecycle state* (`LinkRole`/`LinkDirection`/
//! `LinkState`) is core domain state, not a strategy — it lives in
//! [`crate::connection_state`].

pub mod acceptance;
pub mod config;
pub mod edge;
pub mod fanout;
pub mod selection;
#[cfg(test)]
pub(crate) mod test_support;
pub mod view;
