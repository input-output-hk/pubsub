use std::time::{Duration, Instant};

use async_trait::async_trait;
use dashmap::DashMap;
use tracing::{debug, trace};

use pubsub_types::error::PubSubError;
use pubsub_types::message::{Message, MessageId, TopicId};
use pubsub_types::traits::MessageStore;

/// Default time-to-live for cached messages (1 hour).
const DEFAULT_TTL: Duration = Duration::from_secs(3600);

/// In-memory message cache keyed by (TopicId, sequence_nr).
///
/// Provides fast concurrent access via `DashMap` and supports
/// TTL-based eviction. Intended as the Phase 1 hot cache before
/// the D2 clique-DHT persistence layer is implemented.
pub struct HotCache {
    /// Storage: (TopicId, sequence_nr) -> (Message, insert_time)
    entries: DashMap<(TopicId, u64), (Message, Instant)>,
    /// Maximum number of entries before the cache is considered full.
    max_entries: usize,
}

impl HotCache {
    /// Create a new HotCache with the given maximum capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: DashMap::new(),
            max_entries,
        }
    }

    /// Create a HotCache with the default capacity (100,000 entries).
    pub fn with_defaults() -> Self {
        Self::new(100_000)
    }
}

#[async_trait]
impl MessageStore for HotCache {
    async fn store(&self, msg: Message) -> Result<(), PubSubError> {
        let key = (msg.topic_id.clone(), msg.sequence_nr);

        if self.entries.len() >= self.max_entries {
            // Evict the oldest entry (by insert time) before inserting.
            let oldest_key = self
                .entries
                .iter()
                .min_by_key(|e| e.value().1)
                .map(|e| e.key().clone());
            if let Some(k) = oldest_key {
                self.entries.remove(&k);
                debug!(max = self.max_entries, "HotCache evicted oldest entry at capacity");
            }
        }

        trace!(topic = %key.0, seq = key.1, "Storing message in hot cache");
        self.entries.insert(key, (msg, Instant::now()));
        Ok(())
    }

    async fn get(&self, id: &MessageId) -> Result<Option<Message>, PubSubError> {
        let key = (id.topic_id.clone(), id.sequence_nr);
        let result = self.entries.get(&key).map(|entry| entry.value().0.clone());
        trace!(topic = %id.topic_id, seq = id.sequence_nr, found = result.is_some(), "HotCache get");
        Ok(result)
    }

    async fn get_since(
        &self,
        topic: &TopicId,
        since_sequence_nr: u64,
        limit: usize,
    ) -> Result<Vec<Message>, PubSubError> {
        let mut messages: Vec<(u64, Message)> = self
            .entries
            .iter()
            .filter(|entry| {
                let (ref t, ref seq) = *entry.key();
                t == topic && *seq > since_sequence_nr
            })
            .map(|entry| {
                let (_, ref seq) = *entry.key();
                (*seq, entry.value().0.clone())
            })
            .collect();

        // Sort by sequence number for deterministic ordering
        messages.sort_by_key(|(seq, _)| *seq);

        // Apply limit
        let result: Vec<Message> = messages
            .into_iter()
            .take(limit)
            .map(|(_, msg)| msg)
            .collect();

        debug!(
            topic = %topic,
            since = since_sequence_nr,
            returned = result.len(),
            "HotCache get_since"
        );
        Ok(result)
    }

    async fn evict_expired(&self) -> Result<usize, PubSubError> {
        let now = Instant::now();
        let before = self.entries.len();

        self.entries.retain(|_key, (_, inserted_at)| {
            now.duration_since(*inserted_at) < DEFAULT_TTL
        });

        let evicted = before - self.entries.len();
        if evicted > 0 {
            debug!(evicted, remaining = self.entries.len(), "Evicted expired entries from hot cache");
        }
        Ok(evicted)
    }
}
