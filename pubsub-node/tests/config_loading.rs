use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use pubsub_node::{load_node_config, ConfigError, PeerId};
use tempfile::tempdir;

// Note (feature 008): `subscribed_topics` was removed from the node config —
// a node's topics now come from its subscription-registry entry (ADR 0013), so
// the former 002-US4 `subscribed_topics` parsing tests were removed with the
// field. Config now carries only `[[peers]]`.

// US3 AS-1: a TOML file with three [[peers]] entries loads as a
// config whose `peers` has length 3 with ids in declaration order.
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

// Unknown top-level field surfaces as `ConfigError::Parse` (the
// `deny_unknown_fields` discipline from 001 still applies).
#[test]
fn unknown_top_level_field_yields_parse_error() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("unknown-field.toml");
    fs::write(
        &path,
        r#"
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
