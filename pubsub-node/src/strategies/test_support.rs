//! Shared declarative builders for strategy unit tests (the `strategies/`
//! sibling of `connection_state::test_support`): alias-derived ids and the
//! recurring set/map/view fixtures every strategy test constructs.
//!
//! Test-only (`#[cfg(test)]` at the module declaration); adding a field to
//! [`NodeView`] is a one-place change here rather than a per-test-module sweep.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::str::FromStr;

use crate::peer::PeerId;
use crate::strategies::view::NodeView;
use crate::topic::TopicId;

/// An alias-derived [`PeerId`].
pub(crate) fn peer(s: &str) -> PeerId {
    PeerId::from_str(s).expect("valid peer id")
}

/// A [`TopicId`] from its string form.
pub(crate) fn topic(s: &str) -> TopicId {
    TopicId::from_str(s).expect("valid topic id")
}

/// A subscription set from topic names.
pub(crate) fn subscriptions(topics: &[&str]) -> BTreeSet<TopicId> {
    topics.iter().map(|t| topic(t)).collect()
}

/// A per-topic candidate map from `(topic, members)` entries.
pub(crate) fn candidates(entries: &[(&str, &[&str])]) -> BTreeMap<TopicId, BTreeSet<PeerId>> {
    entries
        .iter()
        .map(|(t, peers)| (topic(t), peers.iter().map(|p| peer(p)).collect()))
        .collect()
}

/// A downstream set from `(peer, topic)` entries.
pub(crate) fn downstream(entries: &[(&str, &str)]) -> HashSet<(PeerId, TopicId)> {
    entries.iter().map(|(p, t)| (peer(p), topic(t))).collect()
}

/// A [`NodeView`] over the borrowed fixtures (epoch nonce 0 — the v1 default).
pub(crate) fn view<'a>(
    subs: &'a BTreeSet<TopicId>,
    cands: &'a BTreeMap<TopicId, BTreeSet<PeerId>>,
    down: &'a HashSet<(PeerId, TopicId)>,
) -> NodeView<'a> {
    NodeView {
        subscriptions: subs,
        candidates: cands,
        downstream: down,
        epoch_nonce: 0,
    }
}
