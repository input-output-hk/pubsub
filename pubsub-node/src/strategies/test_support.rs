//! Shared declarative builders for strategy unit tests (the `strategies/`
//! sibling of `connection_state::test_support`): alias-derived ids and the
//! recurring set/map/view fixtures every strategy test constructs.
//!
//! Test-only (`#[cfg(test)]` at the module declaration); adding a field to
//! [`NodeView`] is a one-place change here rather than a per-test-module sweep.

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use crate::connection_state::{LinkDirection, LinkRole, LinkState, LinkStore};
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

/// A link store holding relay fan-out destinations (`In`/`Relay`, `Active`)
/// from `(peer, topic)` entries — the former downstream set.
pub(crate) fn downstream(entries: &[(&str, &str)]) -> LinkStore {
    let mut store = LinkStore::new();
    for (p, t) in entries {
        store.insert(
            peer(p),
            topic(t),
            LinkRole::Relay,
            LinkDirection::In,
            LinkState::Active,
        );
    }
    store
}

/// A link store from explicit `(peer, topic, role, direction, state)` entries.
pub(crate) fn links(entries: &[(&str, &str, LinkRole, LinkDirection, LinkState)]) -> LinkStore {
    let mut store = LinkStore::new();
    for (p, t, role, direction, state) in entries {
        store.insert(peer(p), topic(t), *role, *direction, *state);
    }
    store
}

/// A [`NodeView`] over the borrowed fixtures (epoch nonce 0 — the v1 default).
pub(crate) fn view<'a>(
    subs: &'a BTreeSet<TopicId>,
    cands: &'a BTreeMap<TopicId, BTreeSet<PeerId>>,
    links: &'a LinkStore,
) -> NodeView<'a> {
    view_with_nonce(subs, cands, links, 0)
}

/// A [`NodeView`] over the borrowed fixtures at an explicit epoch nonce.
pub(crate) fn view_with_nonce<'a>(
    subs: &'a BTreeSet<TopicId>,
    cands: &'a BTreeMap<TopicId, BTreeSet<PeerId>>,
    links: &'a LinkStore,
    epoch_nonce: u64,
) -> NodeView<'a> {
    NodeView {
        subscriptions: subs,
        candidates: cands,
        links,
        epoch_nonce,
    }
}
