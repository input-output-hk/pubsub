//! Vanilla Cyclon peer-sampling protocol (Voulgaris–Gavidia–van Steen 2005,
//! revisited in Antonov & Voulgaris 2023 §II.B + Fig. 1).
//!
//! Each node maintains a bounded view of peer descriptors and periodically
//! initiates a push-pull gossip exchange with the oldest peer in its view,
//! swapping a small slice of descriptors with the partner. The resulting
//! overlay closely approximates a random graph and self-heals under churn.

pub mod bootstrap;
pub mod clock;
pub mod config;
pub mod cyclon;
pub mod descriptor;
pub mod error;
pub mod protocol;
pub mod transport;
pub mod view;

pub use bootstrap::{BootstrapSource, StaticSeeds};
pub use clock::{Clock, ManualClock};
pub use config::CyclonConfig;
pub use cyclon::Cyclon;
pub use descriptor::{Descriptor, NodeId};
pub use error::{Result, SecureCyclonError};
pub use protocol::{GossipRequest, GossipResponse};
pub use transport::Transport;
pub use view::View;
