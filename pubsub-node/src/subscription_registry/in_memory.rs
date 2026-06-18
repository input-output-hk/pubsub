use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::str::FromStr;
use std::sync::Mutex;

use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use crate::error::ConfigError;
use crate::peer::PeerId;
use crate::topic::TopicId;

use super::{
    MembershipEvent, MembershipSnapshot, MembershipWatch, SubscriptionRegistry,
    SubscriptionRegistryControl, SubscriptionRegistryError,
};

/// In-process subscription list: the mock source of truth for node membership.
///
/// Share one instance across nodes via `Arc` (as `InMemoryNetwork` is shared)
/// so they observe the same membership. Construct empty with [`Self::new`] or
/// seed from a TOML subscription-list file with [`Self::from_file`].
pub struct InMemorySubscriptionRegistry {
    inner: Mutex<Inner>,
}

struct Inner {
    /// node -> its topic set (the membership truth).
    membership: HashMap<PeerId, BTreeSet<TopicId>>,
    /// Live watches: each subscriber's watched-topic filter + delta sender.
    subscribers: Vec<Subscriber>,
}

struct Subscriber {
    topics: BTreeSet<TopicId>,
    tx: UnboundedSender<MembershipEvent>,
}

impl Inner {
    /// Deliver one event per subscriber for which `make(watched_topics)`
    /// yields `Some`, pruning subscribers whose receiver has been dropped.
    fn fanout(&mut self, make: impl Fn(&BTreeSet<TopicId>) -> Option<MembershipEvent>) {
        self.subscribers.retain(|sub| match make(&sub.topics) {
            Some(event) => sub.tx.send(event).is_ok(),
            None => !sub.tx.is_closed(),
        });
    }
}

impl InMemorySubscriptionRegistry {
    /// Construct an empty registry (no entries, no watchers).
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                membership: HashMap::new(),
                subscribers: Vec::new(),
            }),
        }
    }

    /// Load a TOML subscription-list file into the initial membership.
    ///
    /// The file is a list of `[[entry]]` tables, each with a `node_id` and a
    /// `topics` array; an optional `deposit` field is accepted but ignored
    /// (out of scope). Unknown fields are rejected. A duplicate `node_id`
    /// is a load error.
    pub fn from_file(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let raw: RawSubscriptionList =
            toml::from_str(&content).map_err(|source| ConfigError::Parse {
                path: path.to_path_buf(),
                source,
            })?;

        let mut membership: HashMap<PeerId, BTreeSet<TopicId>> = HashMap::new();
        for entry in raw.entry {
            let node = PeerId::from_str(&entry.node_id)
                .map_err(|err| ConfigError::InvalidPeer(format!("{}: {err}", path.display())))?;
            let topics = entry
                .topics
                .into_iter()
                .map(|raw_topic| {
                    TopicId::from_str(&raw_topic).map_err(|err| {
                        ConfigError::InvalidTopic(format!("{}: {err}", path.display()))
                    })
                })
                .collect::<Result<BTreeSet<TopicId>, _>>()?;
            if membership.insert(node, topics).is_some() {
                return Err(ConfigError::DuplicateSubscriptionEntry(entry.node_id));
            }
        }

        Ok(Self {
            inner: Mutex::new(Inner {
                membership,
                subscribers: Vec::new(),
            }),
        })
    }
}

impl Default for InMemorySubscriptionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn lock_poisoned() -> ! {
    panic!("subscription registry mutex poisoned")
}

impl SubscriptionRegistry for InMemorySubscriptionRegistry {
    async fn watch(
        &self,
        node: PeerId,
    ) -> Result<(MembershipSnapshot, MembershipWatch), SubscriptionRegistryError> {
        let (tx, rx) = unbounded_channel();
        let mut inner = self.inner.lock().unwrap_or_else(|_| lock_poisoned());

        // The node's own topics scope the watch. (Empty if it has no entry —
        // the node then derives an empty subscription set and no candidates.)
        let own_topics = inner.membership.get(&node).cloned().unwrap_or_default();

        // Snapshot + live, atomically under the lock (so no write is missed or
        // double-delivered at the snapshot/live boundary):
        //   1. the node's own entry first — the node folds this into its
        //      subscription set (present even when empty, so it derives an empty
        //      set rather than nothing);
        let mut snapshot: MembershipSnapshot = vec![(node.clone(), own_topics.clone())];
        //   2. then the current members of those topics (scoped) — candidates.
        for (member, member_topics) in &inner.membership {
            if *member == node {
                continue;
            }
            let scoped: BTreeSet<TopicId> =
                member_topics.intersection(&own_topics).cloned().collect();
            if !scoped.is_empty() {
                snapshot.push((member.clone(), scoped));
            }
        }
        // Live deltas are fanned out scoped to the node's topics. (Re-scoping
        // when the node's *own* entry changes at runtime is deferred to 012;
        // the watched set is the node's topics at watch time.) The subscriber is
        // registered after the snapshot is captured, so the two do not overlap.
        inner.subscribers.push(Subscriber {
            topics: own_topics,
            tx,
        });

        Ok((snapshot, MembershipWatch::new(rx)))
    }
}

impl SubscriptionRegistryControl for InMemorySubscriptionRegistry {
    async fn set_topics(
        &self,
        node: PeerId,
        topics: BTreeSet<TopicId>,
    ) -> Result<(), SubscriptionRegistryError> {
        let mut inner = self.inner.lock().unwrap_or_else(|_| lock_poisoned());
        match inner.membership.get(&node) {
            // Unchanged set: idempotent no-op, no event.
            Some(prev) if *prev == topics => {}
            // Existing entry, changed set: emit one scoped TopicsChanged.
            Some(prev) => {
                let added: BTreeSet<TopicId> = topics.difference(prev).cloned().collect();
                let removed: BTreeSet<TopicId> = prev.difference(&topics).cloned().collect();
                inner.membership.insert(node.clone(), topics);
                inner.fanout(|watched| {
                    let scoped_added: BTreeSet<TopicId> =
                        added.intersection(watched).cloned().collect();
                    let scoped_removed: BTreeSet<TopicId> =
                        removed.intersection(watched).cloned().collect();
                    if scoped_added.is_empty() && scoped_removed.is_empty() {
                        None
                    } else {
                        Some(MembershipEvent::TopicsChanged {
                            node: node.clone(),
                            added: scoped_added,
                            removed: scoped_removed,
                        })
                    }
                });
            }
            // First registration: emit a scoped Joined.
            None => {
                inner.membership.insert(node.clone(), topics.clone());
                inner.fanout(|watched| {
                    let scoped: BTreeSet<TopicId> = topics.intersection(watched).cloned().collect();
                    if scoped.is_empty() {
                        None
                    } else {
                        Some(MembershipEvent::Joined {
                            node: node.clone(),
                            topics: scoped,
                        })
                    }
                });
            }
        }
        Ok(())
    }

    async fn unregister(&self, node: PeerId) -> Result<(), SubscriptionRegistryError> {
        let mut inner = self.inner.lock().unwrap_or_else(|_| lock_poisoned());
        if let Some(prev) = inner.membership.remove(&node) {
            inner.fanout(|watched| {
                if prev.is_disjoint(watched) {
                    None
                } else {
                    Some(MembershipEvent::Left { node: node.clone() })
                }
            });
        }
        Ok(())
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSubscriptionList {
    #[serde(default)]
    entry: Vec<RawEntry>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    node_id: String,
    #[serde(default)]
    topics: Vec<String>,
    /// Accepted but ignored (out of scope for this feature); present so a real
    /// deposit field does not trip `deny_unknown_fields`.
    #[serde(default)]
    #[allow(dead_code)]
    deposit: Option<toml::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(s: &str) -> PeerId {
        PeerId::from_str(s).expect("valid peer id")
    }

    fn topics<const N: usize>(ts: [&str; N]) -> BTreeSet<TopicId> {
        ts.iter()
            .map(|t| TopicId::from_str(t).expect("valid topic"))
            .collect()
    }

    fn snapshot_nodes(snapshot: &MembershipSnapshot) -> BTreeSet<String> {
        snapshot.iter().map(|(n, _)| n.to_string()).collect()
    }

    fn drain(watch: &mut MembershipWatch) -> Vec<MembershipEvent> {
        let mut out = Vec::new();
        while let Some(e) = watch.try_next() {
            out.push(e);
        }
        out
    }

    // ---- US2: write/state observed through the watch stream ----
    //
    // There is no point-read: a node learns its own id + topics from the head
    // `Joined` of its own watch's cold-start burst (the event-stream replacement
    // for the removed `entry()` read-back), and write semantics are observed as
    // deltas on a watcher's stream.

    #[tokio::test]
    async fn set_topics_upsert_reflected_in_own_watch_head() {
        let reg = InMemorySubscriptionRegistry::new();
        reg.set_topics(peer("a"), topics(["t1"])).await.unwrap();
        reg.set_topics(peer("a"), topics(["t1", "t2"]))
            .await
            .unwrap();
        // Watching as `a`, the snapshot head is a's own entry — its id and its
        // (upserted) topics, exactly what a node folds into its subscription set.
        let (snapshot, _watch) = reg.watch(peer("a")).await.unwrap();
        assert_eq!(snapshot, vec![(peer("a"), topics(["t1", "t2"]))]);
    }

    #[tokio::test]
    async fn empty_topics_is_distinct_from_unregister() {
        // The distinction is observable on a watcher's stream by what re-adding a
        // topic emits: a present-but-empty entry *changes* (TopicsChanged), an
        // absent (unregistered) entry is a *first registration* (Joined).
        let reg = InMemorySubscriptionRegistry::new();
        reg.set_topics(peer("w"), topics(["t1"])).await.unwrap();
        let (_snapshot, mut watch) = reg.watch(peer("w")).await.unwrap(); // w's own entry; no members of t1 yet

        // Registered on t1, then emptied — the entry is retained.
        reg.set_topics(peer("a"), topics(["t1"])).await.unwrap();
        reg.set_topics(peer("a"), topics([])).await.unwrap();
        let _ = drain(&mut watch); // Joined a, then TopicsChanged removing t1
                                   // Re-adding t1 to the retained (empty) entry is a change, not a join.
        reg.set_topics(peer("a"), topics(["t1"])).await.unwrap();
        assert_eq!(
            drain(&mut watch),
            vec![MembershipEvent::topics_changed("a", ["t1"], [])]
        );

        // Unregistering removes the entry; re-adding t1 is then a first join.
        reg.unregister(peer("a")).await.unwrap();
        let _ = drain(&mut watch); // Left a
        reg.set_topics(peer("a"), topics(["t1"])).await.unwrap();
        assert_eq!(
            drain(&mut watch),
            vec![MembershipEvent::joined("a", ["t1"])]
        );
    }

    #[tokio::test]
    async fn watch_of_unregistered_node_snapshots_only_empty_self() {
        // No entry ⇒ the snapshot is just the node's own empty entry (so it
        // derives an empty subscription set and no candidates).
        let reg = InMemorySubscriptionRegistry::new();
        let (snapshot, _watch) = reg.watch(peer("ghost")).await.unwrap();
        assert_eq!(snapshot, vec![(peer("ghost"), topics([]))]);
    }

    #[tokio::test]
    async fn from_file_loads_entries() {
        let reg = InMemorySubscriptionRegistry::from_file(Path::new(
            "tests/fixtures/subscription-list.toml",
        ))
        .expect("fixture loads");
        // node-b loaded with {t1, t2}: its own snapshot head reports them.
        let (snapshot_b, _wb) = reg.watch(peer("node-b")).await.unwrap();
        assert!(snapshot_b.contains(&(peer("node-b"), topics(["t1", "t2"]))));
        // node-d absent from the file: its snapshot is an empty self entry.
        let (snapshot_d, _wd) = reg.watch(peer("node-d")).await.unwrap();
        assert_eq!(snapshot_d, vec![(peer("node-d"), topics([]))]);
    }

    #[test]
    fn from_file_rejects_duplicate_node_id() {
        let dir = std::env::temp_dir();
        let path = dir.join("dup-subscription-list.toml");
        std::fs::write(
            &path,
            "[[entry]]\nnode_id = \"a\"\ntopics = [\"t1\"]\n[[entry]]\nnode_id = \"a\"\ntopics = [\"t2\"]\n",
        )
        .unwrap();
        assert!(matches!(
            InMemorySubscriptionRegistry::from_file(&path),
            Err(ConfigError::DuplicateSubscriptionEntry(_))
        ));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn from_file_rejects_unknown_field() {
        let dir = std::env::temp_dir();
        let path = dir.join("bad-subscription-list.toml");
        std::fs::write(
            &path,
            "[[entry]]\nnode_id = \"a\"\ntopics = [\"t1\"]\nbogus = 1\n",
        )
        .unwrap();
        assert!(matches!(
            InMemorySubscriptionRegistry::from_file(&path),
            Err(ConfigError::Parse { .. })
        ));
        let _ = std::fs::remove_file(&path);
    }

    // ---- US1: node-keyed membership stream via watch(node) ----

    #[tokio::test]
    async fn cold_start_replays_own_entry_then_scoped_members() {
        let reg = InMemorySubscriptionRegistry::new();
        reg.set_topics(peer("a"), topics(["t1"])).await.unwrap();
        reg.set_topics(peer("b"), topics(["t1", "t2"]))
            .await
            .unwrap();
        reg.set_topics(peer("c"), topics(["t2"])).await.unwrap();

        // Watch as `a` (own topics {t1}): snapshot is a's own entry first,
        // then members of {t1} (b); c (t2-only) is absent.
        let (snapshot, _watch) = reg.watch(peer("a")).await.unwrap();
        let expected: BTreeSet<String> = ["a", "b"].iter().map(|s| (*s).to_string()).collect();
        assert_eq!(snapshot_nodes(&snapshot), expected);
        // a's own entry reports its topics; the member b is scoped to {t1}.
        for (_, t) in &snapshot {
            assert_eq!(t, &topics(["t1"]));
        }
    }

    #[tokio::test]
    async fn live_join_leave_and_topics_changed() {
        let reg = InMemorySubscriptionRegistry::new();
        // The watcher is registered for {t1, t2}; we watch as it.
        reg.set_topics(peer("w"), topics(["t1", "t2"]))
            .await
            .unwrap();
        let (_snapshot, mut watch) = reg.watch(peer("w")).await.unwrap(); // snapshot: w's own entry, no members yet

        reg.set_topics(peer("d"), topics(["t1"])).await.unwrap();
        assert_eq!(
            drain(&mut watch),
            vec![MembershipEvent::joined("d", ["t1"])]
        );

        // d moves t1 -> t2; both within w's watched set {t1,t2}.
        reg.set_topics(peer("d"), topics(["t2"])).await.unwrap();
        assert_eq!(
            drain(&mut watch),
            vec![MembershipEvent::topics_changed("d", ["t2"], ["t1"])]
        );

        reg.unregister(peer("d")).await.unwrap();
        assert_eq!(drain(&mut watch), vec![MembershipEvent::left("d")]);
    }

    #[tokio::test]
    async fn changes_outside_watched_topics_are_not_delivered() {
        let reg = InMemorySubscriptionRegistry::new();
        reg.set_topics(peer("w"), topics(["t1"])).await.unwrap();
        let (_snapshot, mut watch) = reg.watch(peer("w")).await.unwrap(); // w's own entry

        // c is only on t2; a watcher of {t1} sees nothing.
        reg.set_topics(peer("c"), topics(["t2"])).await.unwrap();
        reg.set_topics(peer("c"), topics(["t2", "t3"]))
            .await
            .unwrap();
        reg.unregister(peer("c")).await.unwrap();
        assert!(drain(&mut watch).is_empty());
    }

    #[tokio::test]
    async fn unchanged_set_emits_no_event() {
        let reg = InMemorySubscriptionRegistry::new();
        reg.set_topics(peer("a"), topics(["t1"])).await.unwrap();
        reg.set_topics(peer("w"), topics(["t1"])).await.unwrap();
        let (_snapshot, mut watch) = reg.watch(peer("w")).await.unwrap(); // snapshot: w's own entry + member a

        reg.set_topics(peer("a"), topics(["t1"])).await.unwrap(); // identical → no event
        assert!(drain(&mut watch).is_empty());
    }
}
