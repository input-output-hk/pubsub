use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::str::FromStr;
use std::sync::Mutex;

use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use crate::crypto::PublicKey;
use crate::error::ConfigError;
use crate::topic::TopicId;

use super::{
    TopicRegistry, TopicRegistryControl, TopicRegistryError, TopicRegistryEvent,
    TopicRegistryWatch, TopicSnapshot,
};

/// In-process topic registry: the mock source of truth for which topics
/// legitimately exist and who may publish to each.
///
/// Share one instance across nodes via `Arc` (as `InMemoryNetwork` /
/// `InMemorySubscriptionRegistry` are shared) so they observe the same topics.
/// Construct empty with [`Self::new`] or seed from a TOML topic-registry file
/// with [`Self::from_file`].
pub struct InMemoryTopicRegistry {
    inner: Mutex<Inner>,
}

struct Inner {
    /// topic -> its authorized publisher keys (empty ⇒ open). The truth.
    topics: HashMap<TopicId, BTreeSet<PublicKey>>,
    /// Live watches. The watch is global, so there is no per-subscriber filter:
    /// every event fans out to every subscriber. Closed senders are pruned on
    /// send.
    subscribers: Vec<UnboundedSender<TopicRegistryEvent>>,
}

impl Inner {
    /// Deliver `event` to every subscriber, pruning those whose receiver has
    /// been dropped.
    fn fanout(&mut self, event: &TopicRegistryEvent) {
        self.subscribers.retain(|tx| tx.send(event.clone()).is_ok());
    }
}

impl InMemoryTopicRegistry {
    /// Construct an empty registry (no topics, no watchers).
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                topics: HashMap::new(),
                subscribers: Vec::new(),
            }),
        }
    }

    /// Load a TOML topic-registry file into the initial topic set.
    ///
    /// The file is a list of `[[topic]]` tables, each with an `id` and an
    /// optional `publishers` array of lowercase-hex public keys (absent or
    /// empty ⇒ open). Unknown fields are rejected — governance fields are not
    /// part of the mock format. A duplicate `id`, or a `publishers` entry that
    /// is not valid hex, is a load error.
    pub fn from_file(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let raw: RawTopicList = toml::from_str(&content).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })?;

        let mut topics: HashMap<TopicId, BTreeSet<PublicKey>> = HashMap::new();
        for entry in raw.topic {
            let topic = TopicId::from_str(&entry.id)
                .map_err(|err| ConfigError::InvalidTopic(format!("{}: {err}", path.display())))?;
            let publishers = entry
                .publishers
                .into_iter()
                .map(|hex| decode_hex(&hex).map(PublicKey::new))
                .collect::<Result<BTreeSet<PublicKey>, _>>()
                .map_err(|err| {
                    ConfigError::InvalidPublisherKey(format!("{}: {err}", path.display()))
                })?;
            if topics.insert(topic, publishers).is_some() {
                return Err(ConfigError::DuplicateTopicEntry(entry.id));
            }
        }

        Ok(Self {
            inner: Mutex::new(Inner {
                topics,
                subscribers: Vec::new(),
            }),
        })
    }
}

impl Default for InMemoryTopicRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn lock_poisoned() -> ! {
    panic!("topic registry mutex poisoned")
}

/// Decode a lowercase/uppercase hex string into raw bytes (parse-at-the-edge;
/// the crate hand-rolls hex *encoding* in `crypto`, this is the symmetric
/// decode — no `hex` crate dependency).
fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    fn nibble(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    }
    let bytes = s.as_bytes();
    if bytes.len() % 2 != 0 {
        return Err(format!("odd-length hex string {s:?}"));
    }
    bytes
        .chunks(2)
        .map(|pair| match (nibble(pair[0]), nibble(pair[1])) {
            (Some(hi), Some(lo)) => Ok((hi << 4) | lo),
            _ => Err(format!("invalid hex string {s:?}")),
        })
        .collect()
}

impl TopicRegistry for InMemoryTopicRegistry {
    async fn watch(&self) -> Result<(TopicSnapshot, TopicRegistryWatch), TopicRegistryError> {
        let (tx, rx) = unbounded_channel();
        let mut inner = self.inner.lock().unwrap_or_else(|_| lock_poisoned());

        // Snapshot + live, atomically under the lock (so no write is missed or
        // double-delivered at the snapshot/live boundary): capture every
        // currently-registered topic as the snapshot, then register the
        // subscriber so it receives only subsequent live deltas. The snapshot
        // and the live stream do not overlap.
        let snapshot: TopicSnapshot = inner
            .topics
            .iter()
            .map(|(topic, publishers)| (topic.clone(), publishers.clone()))
            .collect();
        inner.subscribers.push(tx);

        Ok((snapshot, TopicRegistryWatch::new(rx)))
    }
}

impl TopicRegistryControl for InMemoryTopicRegistry {
    async fn set_topic(
        &self,
        topic: TopicId,
        publishers: BTreeSet<PublicKey>,
    ) -> Result<(), TopicRegistryError> {
        let mut inner = self.inner.lock().unwrap_or_else(|_| lock_poisoned());
        match inner.topics.get(&topic) {
            // Unchanged set: idempotent no-op, no event.
            Some(prev) if *prev == publishers => {}
            // Existing topic, changed publishers: emit one PublishersChanged.
            Some(prev) => {
                let added: BTreeSet<PublicKey> = publishers.difference(prev).cloned().collect();
                let removed: BTreeSet<PublicKey> = prev.difference(&publishers).cloned().collect();
                inner.topics.insert(topic.clone(), publishers);
                inner.fanout(&TopicRegistryEvent::PublishersChanged {
                    topic,
                    added,
                    removed,
                });
            }
            // First registration: emit Registered.
            None => {
                inner.topics.insert(topic.clone(), publishers.clone());
                inner.fanout(&TopicRegistryEvent::Registered { topic, publishers });
            }
        }
        Ok(())
    }

    async fn remove_topic(&self, topic: TopicId) -> Result<(), TopicRegistryError> {
        let mut inner = self.inner.lock().unwrap_or_else(|_| lock_poisoned());
        if inner.topics.remove(&topic).is_some() {
            inner.fanout(&TopicRegistryEvent::Removed { topic });
        }
        Ok(())
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTopicList {
    #[serde(default)]
    topic: Vec<RawTopic>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTopic {
    id: String,
    #[serde(default)]
    publishers: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn topic(s: &str) -> TopicId {
        TopicId::from_str(s).expect("valid topic id")
    }

    fn pk(bytes: &[u8]) -> PublicKey {
        PublicKey::new(bytes.to_vec())
    }

    fn pubs<const N: usize>(keys: [&[u8]; N]) -> BTreeSet<PublicKey> {
        keys.iter().map(|b| pk(b)).collect()
    }

    fn drain(watch: &mut TopicRegistryWatch) -> Vec<TopicRegistryEvent> {
        let mut out = Vec::new();
        while let Some(e) = watch.try_next() {
            out.push(e);
        }
        out
    }

    fn snapshot_topics(snapshot: &TopicSnapshot) -> BTreeSet<String> {
        snapshot.iter().map(|(t, _)| t.to_string()).collect()
    }

    // ---- US1: cold-start snapshot ----

    #[tokio::test]
    async fn cold_start_snapshot_has_all_registered_topics_with_publishers() {
        let reg = InMemoryTopicRegistry::new();
        reg.set_topic(topic("weather"), pubs([b"k1"]))
            .await
            .unwrap();
        reg.set_topic(topic("sports"), pubs([b"k1", b"k2"]))
            .await
            .unwrap();
        reg.set_topic(topic("chat"), pubs([])).await.unwrap(); // open

        let (snapshot, _watch) = reg.watch().await.unwrap();

        let expected: BTreeSet<String> = ["weather", "sports", "chat"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        assert_eq!(snapshot_topics(&snapshot), expected);
        // chat is present with an empty (open) publisher set — not absent.
        let chat = snapshot
            .iter()
            .find_map(|(t, p)| (t == &topic("chat")).then_some(p));
        assert_eq!(chat, Some(&pubs([])));
        // weather carries exactly its one publisher.
        let weather = snapshot
            .iter()
            .find_map(|(t, p)| (t == &topic("weather")).then(|| p.clone()));
        assert_eq!(weather, Some(pubs([b"k1"])));
    }

    // ---- US1: live deltas ----

    #[tokio::test]
    async fn live_register_change_remove() {
        let reg = InMemoryTopicRegistry::new();
        let (snapshot, mut watch) = reg.watch().await.unwrap();
        assert!(snapshot.is_empty()); // empty registry → empty snapshot

        reg.set_topic(topic("news"), pubs([b"k3"])).await.unwrap();
        assert_eq!(
            drain(&mut watch),
            vec![TopicRegistryEvent::Registered {
                topic: topic("news"),
                publishers: pubs([b"k3"]),
            }]
        );

        reg.set_topic(topic("news"), pubs([b"k3", b"k4"]))
            .await
            .unwrap();
        assert_eq!(
            drain(&mut watch),
            vec![TopicRegistryEvent::PublishersChanged {
                topic: topic("news"),
                added: pubs([b"k4"]),
                removed: pubs([]),
            }]
        );

        reg.remove_topic(topic("news")).await.unwrap();
        assert_eq!(
            drain(&mut watch),
            vec![TopicRegistryEvent::Removed {
                topic: topic("news")
            }]
        );
    }

    // ---- US1: idempotency ----

    #[tokio::test]
    async fn unchanged_set_emits_no_event() {
        let reg = InMemoryTopicRegistry::new();
        reg.set_topic(topic("weather"), pubs([b"k1"]))
            .await
            .unwrap();
        reg.set_topic(topic("open"), pubs([])).await.unwrap();
        let (_snapshot, mut watch) = reg.watch().await.unwrap();

        reg.set_topic(topic("weather"), pubs([b"k1"]))
            .await
            .unwrap(); // identical → no event
        reg.set_topic(topic("open"), pubs([])).await.unwrap(); // unchanged empty/open → no event
        assert!(drain(&mut watch).is_empty());
    }

    // ---- US1: open-vs-removed distinction ----

    #[tokio::test]
    async fn empty_publishers_is_distinct_from_removed() {
        let reg = InMemoryTopicRegistry::new();
        reg.set_topic(topic("t"), pubs([])).await.unwrap(); // registered open
                                                            // A fresh watch's snapshot has t present (open).
        let (s1, _w1) = reg.watch().await.unwrap();
        assert_eq!(s1, vec![(topic("t"), pubs([]))]);

        reg.remove_topic(topic("t")).await.unwrap();
        // A fresh watch's snapshot now omits t entirely (empty snapshot).
        let (s2, _w2) = reg.watch().await.unwrap();
        assert!(s2.is_empty());
    }

    // ---- US1: snapshot/live atomicity (FR-007) ----

    #[tokio::test]
    async fn watch_then_immediate_write_delivers_exactly_once() {
        let reg = InMemoryTopicRegistry::new();
        let (snapshot, mut watch) = reg.watch().await.unwrap(); // opened on empty registry
        reg.set_topic(topic("weather"), pubs([b"k1"]))
            .await
            .unwrap();
        // Empty registry → empty snapshot; the live write then appears exactly
        // once on the watch (no gap, no duplicate at the snapshot/live boundary).
        assert!(snapshot.is_empty());
        assert_eq!(
            drain(&mut watch),
            vec![TopicRegistryEvent::Registered {
                topic: topic("weather"),
                publishers: pubs([b"k1"]),
            }]
        );
    }

    // ---- US1: drop ----

    #[tokio::test]
    async fn dropping_a_watch_does_not_disturb_others() {
        let reg = InMemoryTopicRegistry::new();
        let (_snapshot, mut keep) = reg.watch().await.unwrap();
        {
            let _ephemeral = reg.watch().await.unwrap();
        } // dropped here
        reg.set_topic(topic("weather"), pubs([b"k1"]))
            .await
            .unwrap();
        // The surviving watch still receives the event.
        assert_eq!(
            drain(&mut keep),
            vec![TopicRegistryEvent::Registered {
                topic: topic("weather"),
                publishers: pubs([b"k1"]),
            }]
        );
    }

    // ---- US1: from_file ----

    #[tokio::test]
    async fn from_file_loads_topics_and_publishers() {
        let reg = InMemoryTopicRegistry::from_file(Path::new("tests/fixtures/topic-registry.toml"))
            .expect("fixture loads");
        let (snapshot, _watch) = reg.watch().await.unwrap();
        let expected: BTreeSet<String> = ["weather", "sports", "chat"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        assert_eq!(snapshot_topics(&snapshot), expected);
        // weather has two publishers; sports + chat are open (empty).
        for (t, publishers) in &snapshot {
            match t.as_str() {
                "weather" => assert_eq!(publishers.len(), 2),
                "sports" | "chat" => assert!(publishers.is_empty()),
                other => panic!("unexpected topic {other}"),
            }
        }
    }

    #[test]
    fn from_file_rejects_duplicate_topic_id() {
        let path = std::env::temp_dir().join("dup-topic-registry.toml");
        std::fs::write(
            &path,
            "[[topic]]\nid = \"weather\"\n[[topic]]\nid = \"weather\"\n",
        )
        .unwrap();
        assert!(matches!(
            InMemoryTopicRegistry::from_file(&path),
            Err(ConfigError::DuplicateTopicEntry(_))
        ));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn from_file_rejects_bad_publisher_hex() {
        let path = std::env::temp_dir().join("badhex-topic-registry.toml");
        std::fs::write(
            &path,
            "[[topic]]\nid = \"weather\"\npublishers = [\"nothex\"]\n",
        )
        .unwrap();
        assert!(matches!(
            InMemoryTopicRegistry::from_file(&path),
            Err(ConfigError::InvalidPublisherKey(_))
        ));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn from_file_rejects_unknown_field() {
        let path = std::env::temp_dir().join("unknown-topic-registry.toml");
        // `owners` is an on-chain governance field — not part of the mock format.
        std::fs::write(&path, "[[topic]]\nid = \"weather\"\nowners = [\"x\"]\n").unwrap();
        assert!(matches!(
            InMemoryTopicRegistry::from_file(&path),
            Err(ConfigError::Parse { .. })
        ));
        let _ = std::fs::remove_file(&path);
    }
}
