use async_trait::async_trait;

use crate::descriptor::Descriptor;
use crate::error::Result;

/// Source of initial peer descriptors for a fresh node.
///
/// A node cannot gossip without at least one known peer; this trait abstracts
/// over the mechanism that supplies those first descriptors so the same
/// algorithm runs against in-memory tests, on-chain registries, mDNS, etc.
#[async_trait]
pub trait BootstrapSource: Send + Sync {
    async fn seeds(&self) -> Result<Vec<Descriptor>>;
}

/// Hard-coded seed list. Useful for integration tests and local testnets.
#[derive(Clone, Default)]
pub struct StaticSeeds {
    seeds: Vec<Descriptor>,
}

impl StaticSeeds {
    pub fn new(seeds: Vec<Descriptor>) -> Self {
        Self { seeds }
    }
}

#[async_trait]
impl BootstrapSource for StaticSeeds {
    async fn seeds(&self) -> Result<Vec<Descriptor>> {
        Ok(self.seeds.clone())
    }
}
