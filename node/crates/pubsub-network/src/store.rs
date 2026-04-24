use std::time::{Duration, Instant};

use async_trait::async_trait;
use dashmap::DashMap;
use tracing::{debug, trace};

use pubsub_types::error::PubSubError;
use pubsub_types::message::{Message, MessageId, PublisherId, TopicId};
use pubsub_types::traits::MessageStore;

/// Default time-to-live for cached messages (1 hour).
const DEFAULT_TTL: Duration = Duration::from_secs(3600);

/// In-memory message cache keyed by (TopicId, PublisherId, sequence_nr).
///
/// Including `publisher_id` in the key prevents two publishers on the same
/// topic from colliding at the same sequence number — a real scenario in
/// multi-publisher topics.
pub struct HotCache {
    entries: DashMap<(TopicId, PublisherId, u64), (Message, Instant)>,
    max_entries: usize,
}

impl HotCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: DashMap::new(),
            max_entries,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(100_000)
    }
}

#[async_trait]
impl MessageStore for HotCache {
    async fn store(&self, msg: Message) -> Result<(), PubSubError> {
        let key = (msg.topic_id.clone(), msg.publisher_id.clone(), msg.sequence_nr);

        if self.entries.len() >= self.max_entries {
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

        trace!(topic = %msg.topic_id, seq = msg.sequence_nr, "Storing message in hot cache");
        self.entries.insert(key, (msg, Instant::now()));
        Ok(())
    }

    async fn get(&self, id: &MessageId) -> Result<Option<Message>, PubSubError> {
        let key = (id.topic_id.clone(), id.publisher_id.clone(), id.sequence_nr);
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
                let (ref t, _, ref seq) = *entry.key();
                t == topic && *seq > since_sequence_nr
            })
            .map(|entry| {
                let (_, _, ref seq) = *entry.key();
                (*seq, entry.value().0.clone())
            })
            .collect();

        messages.sort_by_key(|(seq, _)| *seq);

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

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use bytes::Bytes;

    use pubsub_types::message::{Message, MessageId, PublisherCredential, PublisherId, TopicId};
    use pubsub_types::traits::MessageStore;

    use super::HotCache;

    fn topic(seed: u8) -> TopicId {
        TopicId([seed; 32])
    }

    fn publisher(seed: u8) -> PublisherId {
        PublisherId(PublisherCredential::ed25519(Bytes::from(vec![seed; 32])))
    }

    fn msg(topic_seed: u8, pub_seed: u8, seq: u64) -> Message {
        Message {
            topic_id: topic(topic_seed),
            sequence_nr: seq,
            timestamp_ms: 0,
            publisher_id: publisher(pub_seed),
            signature: Bytes::from(vec![0u8; 64]),
            payload: Bytes::from(format!("payload-{seq}")),
            metadata: BTreeMap::new(),
        }
    }

    #[tokio::test]
    async fn two_publishers_same_seq_no_collision() {
        let cache = HotCache::with_defaults();
        let m1 = msg(1, 0xAA, 5);
        let m2 = msg(1, 0xBB, 5); // same topic + seq, different publisher
        let id1 = m1.id();
        let id2 = m2.id();

        cache.store(m1).await.unwrap();
        cache.store(m2).await.unwrap();

        let got1 = cache.get(&id1).await.unwrap().expect("publisher AA msg missing");
        let got2 = cache.get(&id2).await.unwrap().expect("publisher BB msg missing");

        assert_eq!(got1.publisher_id, publisher(0xAA));
        assert_eq!(got2.publisher_id, publisher(0xBB));
    }

    #[tokio::test]
    async fn get_since_returns_all_publishers() {
        let cache = HotCache::with_defaults();
        // Two publishers, seqs 1..=3 each
        for seq in 1u64..=3 {
            cache.store(msg(2, 0xAA, seq)).await.unwrap();
            cache.store(msg(2, 0xBB, seq)).await.unwrap();
        }
        let results = cache.get_since(&topic(2), 1, 100).await.unwrap();
        // seq > 1 for both publishers = 4 messages
        assert_eq!(results.len(), 4);
    }
}
