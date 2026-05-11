use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Source of millisecond wall-clock timestamps.
///
/// Abstracted so integration tests can drive a deterministic clock and so
/// production code can later swap in NTP-corrected time without touching
/// the protocol code.
pub trait Clock: Send + Sync {
    fn now_ms(&self) -> u64;
}

/// Manually advanced clock for tests. Cheap to clone — the internal counter
/// is shared.
#[derive(Default, Clone)]
pub struct ManualClock {
    inner: Arc<AtomicU64>,
}

impl ManualClock {
    pub fn new(initial_ms: u64) -> Self {
        Self {
            inner: Arc::new(AtomicU64::new(initial_ms)),
        }
    }

    pub fn advance(&self, delta_ms: u64) {
        self.inner.fetch_add(delta_ms, Ordering::Relaxed);
    }
}

impl Clock for ManualClock {
    fn now_ms(&self) -> u64 {
        self.inner.load(Ordering::Relaxed)
    }
}
