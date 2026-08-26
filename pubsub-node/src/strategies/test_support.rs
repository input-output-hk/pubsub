//! Shared declarative builders for strategy unit tests (the `strategies/`
//! sibling of `connection_state::test_support`): alias-derived ids and the
//! recurring set/map/view fixtures every strategy test constructs.
//!
//! Test-only (`#[cfg(test)]` at the module declaration); adding a field to
//! [`NodeView`] is a one-place change here rather than a per-test-module sweep.

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;
use std::sync::{Arc, OnceLock};

use crate::connection_state::{LinkKey, LinkKind, LinkState};
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

/// A per-topic candidate map from `(topic, members)` entries — the stored
/// (`Arc`-shared, full-membership) shape; include `"self"` among the members
/// to exercise the view's read-time self-exclusion (ADR 0038).
pub(crate) fn candidates(entries: &[(&str, &[&str])]) -> BTreeMap<TopicId, Arc<BTreeSet<PeerId>>> {
    entries
        .iter()
        .map(|(t, peers)| (topic(t), Arc::new(peers.iter().map(|p| peer(p)).collect())))
        .collect()
}

/// The id every `view()` fixture carries as the node's own — matches the
/// `peer("self")` identity the strategy constructors use, so a fixture that
/// lists `"self"` among a topic's members observes the read-time exclusion.
pub(crate) fn view_self() -> &'static PeerId {
    static SELF: OnceLock<PeerId> = OnceLock::new();
    SELF.get_or_init(|| peer("self"))
}

/// A downstream link map of accepted **relay** entries (`Active`) from
/// `(peer, topic)` pairs — the pre-015 "downstream set" fixture.
pub(crate) fn downstream(entries: &[(&str, &str)]) -> BTreeMap<LinkKey, LinkState> {
    links_of(entries, LinkKind::Relay)
}

/// A link map with one `Active` entry of `kind` per `(peer, topic)` pair.
pub(crate) fn links_of(entries: &[(&str, &str)], kind: LinkKind) -> BTreeMap<LinkKey, LinkState> {
    entries
        .iter()
        .map(|(p, t)| (LinkKey::new(topic(t), peer(p), kind), LinkState::Active))
        .collect()
}

/// A shared empty link map for the (majority of) view fixtures that hold no
/// upstream links — keeps the `view()` builder signatures unchanged.
pub(crate) fn no_links() -> &'static BTreeMap<LinkKey, LinkState> {
    static EMPTY: OnceLock<BTreeMap<LinkKey, LinkState>> = OnceLock::new();
    EMPTY.get_or_init(BTreeMap::new)
}

/// A [`NodeView`] over the borrowed fixtures (epoch nonce 0 — the v1 default;
/// no upstream links).
pub(crate) fn view<'a>(
    subs: &'a BTreeSet<TopicId>,
    cands: &'a BTreeMap<TopicId, Arc<BTreeSet<PeerId>>>,
    down: &'a BTreeMap<LinkKey, LinkState>,
) -> NodeView<'a> {
    view_with_nonce(subs, cands, down, 0)
}

/// A [`NodeView`] over the borrowed fixtures at an explicit epoch nonce.
pub(crate) fn view_with_nonce<'a>(
    subs: &'a BTreeSet<TopicId>,
    cands: &'a BTreeMap<TopicId, Arc<BTreeSet<PeerId>>>,
    down: &'a BTreeMap<LinkKey, LinkState>,
    epoch_nonce: u64,
) -> NodeView<'a> {
    NodeView {
        subscriptions: subs,
        self_id: view_self(),
        candidates: cands,
        upstream: no_links(),
        downstream: down,
        epoch_nonce,
        admitted_counts: no_admissions(),
    }
}

/// A shared empty admissions-count map — view fixtures spend no budget
/// unless a test says otherwise via [`view_with_admitted`].
pub(crate) fn no_admissions() -> &'static BTreeMap<(TopicId, LinkKind), usize> {
    static EMPTY: OnceLock<BTreeMap<(TopicId, LinkKind), usize>> = OnceLock::new();
    EMPTY.get_or_init(BTreeMap::new)
}

/// A [`NodeView`] over the borrowed fixtures with an explicit
/// admissions-count map (the ADR 0042 budget's spent counts).
pub(crate) fn view_with_admitted<'a>(
    subs: &'a BTreeSet<TopicId>,
    cands: &'a BTreeMap<TopicId, Arc<BTreeSet<PeerId>>>,
    down: &'a BTreeMap<LinkKey, LinkState>,
    admitted: &'a BTreeMap<(TopicId, LinkKind), usize>,
) -> NodeView<'a> {
    NodeView {
        subscriptions: subs,
        self_id: view_self(),
        candidates: cands,
        upstream: no_links(),
        downstream: down,
        epoch_nonce: 0,
        admitted_counts: admitted,
    }
}

/// A [`NodeView`] with an explicit **upstream** link map as well (publisher
/// acceptance fixtures count publisher upstreams).
pub(crate) fn view_with_upstream<'a>(
    subs: &'a BTreeSet<TopicId>,
    cands: &'a BTreeMap<TopicId, Arc<BTreeSet<PeerId>>>,
    up: &'a BTreeMap<LinkKey, LinkState>,
    down: &'a BTreeMap<LinkKey, LinkState>,
) -> NodeView<'a> {
    NodeView {
        subscriptions: subs,
        self_id: view_self(),
        candidates: cands,
        upstream: up,
        downstream: down,
        epoch_nonce: 0,
        admitted_counts: no_admissions(),
    }
}
