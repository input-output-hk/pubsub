//! The **relay** handshake: the dialer asks to receive the acceptor's relayed
//! traffic. Dials live in `upstream` (full `AwaitingAccept→Active` lifecycle),
//! accepted links in `downstream` (`Active` on insert — presence means
//! accepted); every entry is `LinkKind::Relay`.

use std::sync::Arc;

use super::super::{Effect, NodeState};
use crate::connection_state::{LinkKey, LinkKind, LinkState};
use crate::message::{ConnectionAction, ConnectionMessage, HandshakeKind};
use crate::peer::PeerId;
use crate::strategies::acceptance::Admission;
use crate::topic::TopicId;

const KIND: HandshakeKind = HandshakeKind::Relay;

/// Transition for an inbound relay-handshake control message: the shared
/// verification prelude, then one arm per action. A **symmetric** node drops
/// relay handshakes outright — its relay-class links are established
/// exclusively by the symmetric handshake, and admitting a directional
/// request would record a one-way link on a node whose teardown/severance
/// mechanics assume every relay link is mirrored (the reverse of the
/// `symmetric_edges_disabled` guard; ADR 0034).
pub(in crate::state) fn handle(
    state: &mut NodeState,
    connection: ConnectionMessage,
) -> Vec<Effect> {
    if state.symmetric_edges {
        tracing::info!(
            target: "pubsub_node::node",
            event = "message_dropped",
            cause = "relay_handshake_disabled",
            self_id = %state.self_id,
        );
        return Vec::new();
    }
    let Some(plain) = super::verified(state, connection, KIND) else {
        return Vec::new();
    };
    match plain.action {
        ConnectionAction::Request { topic } => handle_request(state, plain.emitter, topic),
        ConnectionAction::Accepted { topic } => handle_accepted(state, &plain.emitter, &topic),
        ConnectionAction::Terminated { topic } => {
            super::remove_terminated(state, &plain.emitter, &topic, LinkKind::Relay)
        }
        ConnectionAction::Rejected { topic } => handle_rejected(state, &plain.emitter, &topic),
    }
}

/// A verified relay `Request`: the accept/reject *policy* is the injected
/// relay acceptance instance; the handler owns the mechanics. Membership
/// validation runs against the **membership-derived** view (registration
/// gates delivery, not acceptance — the S7 pin). An accepted request records
/// the emitter as relay downstream (idempotently, `Active` — the dialer will
/// receive from this node) and replies `Accepted`; refusals are the shared
/// arms.
fn handle_request(state: &mut NodeState, emitter: PeerId, topic: TopicId) -> Vec<Effect> {
    if !super::gate_synced(state, &emitter, &topic) {
        return Vec::new();
    }
    let strategy = Arc::clone(&state.acceptance_strategy);
    let admission = strategy.admit(&emitter, &topic, &state.view());
    match admission {
        Admission::Accept => {
            // Idempotent: the map absorbs a duplicate; a re-dial re-sends
            // Accepted.
            let key = LinkKey::new(topic.clone(), emitter.clone(), LinkKind::Relay);
            state.downstream.insert(key, LinkState::Active);
            super::accepted_reply(state, emitter, topic, KIND)
        }
        Admission::RejectMembership | Admission::RejectIllegitimate => {
            super::silent_refusal(state, admission, &emitter, &topic)
        }
        Admission::RejectOverCapacity => super::reject_over_capacity(state, emitter, topic, KIND),
    }
}

/// A verified relay `Accepted`: activates the matching `AwaitingAccept`
/// upstream dial. An `Accepted` with no matching pending entry (absent, or
/// already `Active`) is dropped and creates/modifies nothing.
fn handle_accepted(state: &mut NodeState, emitter: &PeerId, topic: &TopicId) -> Vec<Effect> {
    let key = LinkKey::new(topic.clone(), emitter.clone(), LinkKind::Relay);
    if !super::promote_dialed(&mut state.upstream, &key) {
        super::log_unsolicited(state, "unsolicited_accept", emitter, topic);
    }
    Vec::new()
}

/// A verified relay `Rejected` (over-capacity refusal of this node's dial,
/// feature 005 / ADR 0025): drops the matching pending upstream entry. A
/// `Rejected` with no matching pending entry is dropped and changes nothing;
/// a rejection is never treated as misbehaviour.
fn handle_rejected(state: &mut NodeState, emitter: &PeerId, topic: &TopicId) -> Vec<Effect> {
    let key = LinkKey::new(topic.clone(), emitter.clone(), LinkKind::Relay);
    if !super::drop_rejected_dial(&mut state.upstream, &key) {
        super::log_unsolicited(state, "unsolicited_reject", emitter, topic);
    }
    Vec::new()
}
