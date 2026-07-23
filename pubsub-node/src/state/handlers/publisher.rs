//! The **publisher** handshake (feature 015, M3): the dialer asks to push its
//! own publications to the acceptor. Dials live in `downstream` (full
//! `AwaitingAccept→Active` lifecycle — the node will send), accepted inbound
//! links in `upstream` (`Active` on insert — each owner may push its own
//! publications here); every entry is `LinkKind::Publisher`.

use std::sync::Arc;

use super::super::{Effect, NodeState};
use crate::connection_state::{LinkKey, LinkKind, LinkState};
use crate::message::{ConnectionAction, ConnectionMessage, HandshakeKind};
use crate::peer::PeerId;
use crate::strategies::acceptance::Admission;
use crate::topic::TopicId;

const KIND: HandshakeKind = HandshakeKind::Publisher;

/// Transition for an inbound publisher-handshake control message: the shared
/// verification prelude, then one arm per action.
pub(in crate::state) fn handle(
    state: &mut NodeState,
    connection: ConnectionMessage,
) -> Vec<Effect> {
    let Some(plain) = super::verified(state, connection, KIND) else {
        return Vec::new();
    };
    match plain.action {
        ConnectionAction::Request { topic } => handle_request(state, plain.emitter, topic),
        ConnectionAction::Accepted { topic } => handle_accepted(state, &plain.emitter, &topic),
        ConnectionAction::Terminated { topic } => {
            super::remove_terminated(state, &plain.emitter, &topic, LinkKind::Publisher)
        }
        ConnectionAction::Rejected { topic } => handle_rejected(state, &plain.emitter, &topic),
    }
}

/// A verified publisher `Request`: the policy is the optional publisher
/// acceptance instance — a node with none configured drops the request
/// silently (the feature is off, the M2 baseline; indistinguishable from a
/// silent refusal). An accepted request records the emitter as publisher
/// upstream (idempotently, `Active` — the dialer will push to this node) and
/// replies `Accepted`; refusals are the shared arms, with a capacity disjoint
/// from the relay cap by construction (`admit_prelude` is kind-aware).
fn handle_request(state: &mut NodeState, emitter: PeerId, topic: TopicId) -> Vec<Effect> {
    if !super::gate_synced(state, &emitter, &topic) {
        return Vec::new();
    }
    let Some(strategy) = &state.publisher_acceptance else {
        tracing::info!(
            target: "pubsub_node::node",
            event = "message_dropped",
            cause = "publisher_links_disabled",
            self_id = %state.self_id,
            emitter = %emitter,
            topic = %topic,
        );
        return Vec::new();
    };
    let strategy = Arc::clone(strategy);
    let admission = strategy.admit(&emitter, &topic, &state.view());
    match admission {
        Admission::Accept => {
            let key = LinkKey::new(topic.clone(), emitter.clone(), LinkKind::Publisher);
            state.upstream.insert(key, LinkState::Active);
            super::accepted_reply(state, emitter, topic, KIND)
        }
        Admission::RejectMembership | Admission::RejectIllegitimate => {
            super::silent_refusal(state, admission, &emitter, &topic)
        }
        Admission::RejectOverCapacity => super::reject_over_capacity(state, emitter, topic, KIND),
    }
}

/// A verified publisher `Accepted`: activates the matching `AwaitingAccept`
/// downstream dial (this node's own standing initiation target). An
/// `Accepted` with no matching pending entry is dropped.
fn handle_accepted(state: &mut NodeState, emitter: &PeerId, topic: &TopicId) -> Vec<Effect> {
    let key = LinkKey::new(topic.clone(), emitter.clone(), LinkKind::Publisher);
    if !super::promote_dialed(&mut state.downstream, &key) {
        super::log_unsolicited(state, "unsolicited_accept", emitter, topic);
    }
    Vec::new()
}

/// A verified publisher `Rejected`: drops the matching pending downstream
/// dial. Same no-retry semantics as the relay arm.
fn handle_rejected(state: &mut NodeState, emitter: &PeerId, topic: &TopicId) -> Vec<Effect> {
    let key = LinkKey::new(topic.clone(), emitter.clone(), LinkKind::Publisher);
    if !super::drop_rejected_dial(&mut state.downstream, &key) {
        super::log_unsolicited(state, "unsolicited_reject", emitter, topic);
    }
    Vec::new()
}
