use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use pubsub_node::{
    load_node_config, ConfigError, InMemoryNetwork, Node, PeerId, TestVerifier, TopicId, Verifier,
};
use tempfile::tempdir;

// US3 AS-1: a TOML file with three [[peers]] entries loads as a
// PeerListConfig whose `peers` has length 3 with ids in declaration order.
#[test]
fn loads_three_peer_descriptors_from_toml() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("peers.toml");
    fs::write(
        &path,
        r#"
[[peers]]
id = "node-b"

[[peers]]
id = "node-c"

[[peers]]
id = "node-d"
"#,
    )
    .expect("write toml");

    let cfg = load_node_config(&path).expect("load Ok");

    assert_eq!(cfg.peers.len(), 3, "three peer entries");
    assert_eq!(cfg.peers[0].id, PeerId::from_str("node-b").unwrap());
    assert_eq!(cfg.peers[1].id, PeerId::from_str("node-c").unwrap());
    assert_eq!(cfg.peers[2].id, PeerId::from_str("node-d").unwrap());
}

// US3 AS-2 + FR-001 + CHK047: malformed inputs surface as actionable errors,
// each with a distinct ConfigError variant.
#[test]
fn malformed_toml_yields_actionable_error() {
    // (1) Syntactically invalid TOML: unclosed [[peers] → ConfigError::Parse.
    {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("syntax.toml");
        fs::write(
            &path,
            r#"
[[peers]
id = "node-b"
"#,
        )
        .expect("write toml");

        let err = load_node_config(&path).expect_err("expected Parse error");
        match &err {
            ConfigError::Parse { path: p, .. } => {
                assert_eq!(p, &path, "Parse error carries the offending path");
            }
            other => panic!("expected ConfigError::Parse, got: {other:?}"),
        }
        let rendered = format!("{err}");
        assert!(
            rendered.contains(path.to_str().unwrap()),
            "error chain includes path: {rendered}",
        );
        // toml::de::Error surfaces line/column info via its Display chain.
        // We assert presence of either a numeric line position or a
        // "line N" / "column N" / "at line" marker to keep the assertion
        // robust against minor wording changes in the toml crate.
        assert!(
            rendered.chars().any(|c| c.is_ascii_digit()),
            "error chain includes positional info: {rendered}",
        );
    }

    // (2) Structurally valid TOML with an empty id → ConfigError::InvalidPeer.
    {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("empty_id.toml");
        fs::write(
            &path,
            r#"
[[peers]]
id = ""
"#,
        )
        .expect("write toml");

        let err = load_node_config(&path).expect_err("expected InvalidPeer error");
        match &err {
            ConfigError::InvalidPeer(msg) => {
                assert!(
                    msg.contains(path.to_str().unwrap()),
                    "InvalidPeer message includes path: {msg}",
                );
                assert!(
                    msg.to_lowercase().contains("empty"),
                    "InvalidPeer message names the rule violation: {msg}",
                );
            }
            other => panic!("expected ConfigError::InvalidPeer, got: {other:?}"),
        }
    }

    // (3) Path to a non-existent file → ConfigError::Io.
    {
        let dir = tempdir().expect("tempdir");
        let path: PathBuf = dir.path().join("does-not-exist.toml");

        let err = load_node_config(&path).expect_err("expected Io error");
        match &err {
            ConfigError::Io { path: p, .. } => {
                assert_eq!(p, &path, "Io error carries the offending path");
            }
            other => panic!("expected ConfigError::Io, got: {other:?}"),
        }
        let rendered = format!("{err}");
        assert!(
            rendered.contains(path.to_str().unwrap()),
            "error chain includes path: {rendered}",
        );
    }
}

// ---------------------------------------------------------------------------
// 002 US4 (Subscriptions Loaded from TOML at Node Construction)
//
// Note on TOML layout: `subscribed_topics` is a top-level array; per TOML's
// table-scoping rules, top-level bare keys must appear *before* any
// `[[peers]]` array-of-tables header — otherwise the parser binds them to
// the last array entry. Each TOML below puts `subscribed_topics` first.
// ---------------------------------------------------------------------------

// US4 AS-1: present `subscribed_topics` yields the parsed initial set.
#[test]
fn subscribed_topics_present_yields_initial_set() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("with-topics.toml");
    fs::write(
        &path,
        r#"
subscribed_topics = ["a", "b"]

[[peers]]
id = "node-b"
"#,
    )
    .expect("write toml");

    let cfg = load_node_config(&path).expect("load Ok");

    assert_eq!(
        cfg.subscribed_topics,
        vec![
            TopicId::from_str("a").unwrap(),
            TopicId::from_str("b").unwrap(),
        ],
        "subscribed_topics preserved in declaration order",
    );
    assert_eq!(cfg.peers.len(), 1);
    assert_eq!(cfg.peers[0].id, PeerId::from_str("node-b").unwrap());
}

// US4 AS-2: absent `subscribed_topics` yields the empty set.
#[test]
fn subscribed_topics_absent_yields_empty_set() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("no-topics.toml");
    fs::write(
        &path,
        r#"
[[peers]]
id = "node-b"
"#,
    )
    .expect("write toml");

    let cfg = load_node_config(&path).expect("load Ok");

    assert!(
        cfg.subscribed_topics.is_empty(),
        "absent field defaults to empty Vec",
    );
}

// US4 AS-3: explicit empty `subscribed_topics = []` is indistinguishable
// from absent (both yield an empty Vec).
#[test]
fn subscribed_topics_empty_array_yields_empty_set() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("empty-topics.toml");
    fs::write(
        &path,
        r#"
subscribed_topics = []

[[peers]]
id = "node-b"
"#,
    )
    .expect("write toml");

    let cfg = load_node_config(&path).expect("load Ok");

    assert!(
        cfg.subscribed_topics.is_empty(),
        "explicit `[]` yields empty Vec",
    );
}

// US4 AS-4: invalid `subscribed_topics` entries surface as
// `ConfigError::InvalidTopic`. Two sub-cases: empty string + NUL byte.
#[test]
fn invalid_topic_entry_yields_invalid_topic_error() {
    // (a) Empty string → TopicIdError::Empty.
    {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("empty-topic.toml");
        fs::write(
            &path,
            r#"
subscribed_topics = ["valid", ""]

[[peers]]
id = "node-b"
"#,
        )
        .expect("write toml");

        let err = load_node_config(&path).expect_err("expected InvalidTopic");
        match &err {
            ConfigError::InvalidTopic(msg) => {
                assert!(
                    msg.contains(path.to_str().unwrap()),
                    "InvalidTopic message includes path: {msg}",
                );
                assert!(
                    msg.to_lowercase().contains("empty"),
                    "InvalidTopic message names the rule violation: {msg}",
                );
            }
            other => panic!("expected ConfigError::InvalidTopic, got: {other:?}"),
        }
    }

    // (b) Internal NUL byte → TopicIdError::ContainsNul.
    {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("nul-topic.toml");
        fs::write(
            &path,
            "subscribed_topics = [\"valid\", \"bad\\u0000topic\"]\n\n[[peers]]\nid = \"node-b\"\n",
        )
        .expect("write toml");

        let err = load_node_config(&path).expect_err("expected InvalidTopic");
        match &err {
            ConfigError::InvalidTopic(msg) => {
                assert!(
                    msg.contains(path.to_str().unwrap()),
                    "InvalidTopic message includes path: {msg}",
                );
                assert!(
                    msg.to_lowercase().contains("nul"),
                    "InvalidTopic message names the NUL violation: {msg}",
                );
            }
            other => panic!("expected ConfigError::InvalidTopic, got: {other:?}"),
        }
    }
}

// US4 AS-5: unknown top-level field surfaces as `ConfigError::Parse` (the
// `deny_unknown_fields` discipline from 001 still applies under the
// extended schema).
#[test]
fn unknown_top_level_field_yields_parse_error() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("unknown-field.toml");
    fs::write(
        &path,
        r#"
subscribed_topics = ["t1"]
unexpected_field = "value"

[[peers]]
id = "node-b"
"#,
    )
    .expect("write toml");

    let err = load_node_config(&path).expect_err("expected Parse error");
    match &err {
        ConfigError::Parse { path: p, source } => {
            assert_eq!(p, &path, "Parse error carries the offending path");
            let source_msg = format!("{source}");
            assert!(
                source_msg.contains("unexpected_field"),
                "Parse error names the unknown field: {source_msg}",
            );
        }
        other => panic!("expected ConfigError::Parse, got: {other:?}"),
    }
}

// US4 AS-6 / FR-010 + CHK025: duplicate entries in `subscribed_topics` are
// preserved in the returned Vec; the consumer's HashSet boundary at
// Node construction absorbs them. Per the test discipline locked in
// CHK027 — the test asserts on the deduplicated `subscriptions()` snapshot,
// NOT on the warn log content.
#[tokio::test]
async fn duplicate_subscribed_topic_yields_dedup_set() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("dup-topics.toml");
    fs::write(
        &path,
        r#"
subscribed_topics = ["t1", "t2", "t1"]

[[peers]]
id = "node-b"
"#,
    )
    .expect("write toml");

    let cfg = load_node_config(&path).expect("load Ok");

    let t1 = TopicId::from_str("t1").unwrap();
    let t2 = TopicId::from_str("t2").unwrap();

    // Loader-side return-shape contract: Vec retains duplicates verbatim.
    assert_eq!(
        cfg.subscribed_topics,
        vec![t1.clone(), t2.clone(), t1.clone()],
        "loader preserves the original Vec including duplicates",
    );

    // Construct a Node using the HashSet boundary the CLI uses (per
    // src/main.rs); duplicates are absorbed.
    let initial_subscriptions: HashSet<TopicId> = cfg.subscribed_topics.iter().cloned().collect();
    let network = Arc::new(InMemoryNetwork::new());
    let verifier: Arc<dyn Verifier> = Arc::new(TestVerifier);
    let node = Node::new(
        PeerId::from_str("node-x").unwrap(),
        cfg,
        initial_subscriptions,
        network,
        verifier,
    )
    .await
    .expect("construct node");

    let mut got = node.subscriptions();
    got.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    assert_eq!(got, vec![t1, t2], "HashSet boundary dedups to {{t1, t2}}");
}
