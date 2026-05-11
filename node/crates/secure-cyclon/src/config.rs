/// Cyclon protocol configuration.
///
/// Defaults follow the paper's evaluation in §VI.A: `swap_len = 3`,
/// `view_len = 20`.
#[derive(Debug, Clone)]
pub struct CyclonConfig {
    /// View length ℓ — number of peer descriptors held.
    pub view_len: usize,
    /// Swap length s — descriptors exchanged per gossip cycle.
    pub swap_len: usize,
    /// Per-cycle gossip period.
    pub gossip_period_ms: u64,
    /// Timeout for a single gossip exchange before recovery kicks in.
    pub exchange_timeout_ms: u64,
}

impl Default for CyclonConfig {
    fn default() -> Self {
        Self {
            view_len: 20,
            swap_len: 3,
            gossip_period_ms: 10_000,
            exchange_timeout_ms: 5_000,
        }
    }
}
