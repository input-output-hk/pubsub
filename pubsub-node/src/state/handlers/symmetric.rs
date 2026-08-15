//! The **symmetric** handshake (M4, ADR 0034): one accept decision
//! establishes a bidirectional relay-class link. The acceptor records the
//! peer in **both** `downstream` and `upstream`; the dialer mirrors on the
//! acceptance. Reciprocity is *constructed* by these mechanics — not hoped
//! for from the two ends' independent predicate draws agreeing — so it holds
//! regardless of the selection strategy or the two ends' bucket-count views:
//! a capacity refusal is a whole-edge refusal (nothing inserted on either
//! end), and teardown removes both halves together.
//!
//! The stored entries are ordinary `LinkKind::Relay` links — a bidirectional
//! link is the same link present in both maps, nothing new is stored — so
//! M4's flooding is the existing relay fan-out and its receive gate the
//! existing relay arm.

use std::sync::Arc;

use super::super::{Effect, NodeState};
use crate::connection_state::{LinkKey, LinkKind, LinkState};
use crate::message::{ConnectionAction, ConnectionMessage, HandshakeKind};
use crate::peer::PeerId;
use crate::strategies::acceptance::Admission;
use crate::topic::TopicId;

const KIND: HandshakeKind = HandshakeKind::Symmetric;

/// Transition for an inbound symmetric-handshake control message. A node not
/// configured for symmetric edges drops it outright — the feature is off by
/// construction (a directional-model node must never mirror a link), the
/// same doctrine as the disabled publisher seam.
pub(in crate::state) fn handle(
    state: &mut NodeState,
    connection: ConnectionMessage,
) -> Vec<Effect> {
    if !state.symmetric_edges {
        tracing::info!(
            target: "pubsub_node::node",
            event = "message_dropped",
            cause = "symmetric_edges_disabled",
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
        // Teardown atomicity: the shared kind-scoped removal already takes
        // the entry out of BOTH directions — for a symmetric link that is
        // exactly the mirror-pair teardown.
        ConnectionAction::Terminated { topic } => {
            super::remove_terminated(state, &plain.emitter, &topic, LinkKind::Relay)
        }
        ConnectionAction::Rejected { topic } => handle_rejected(state, &plain.emitter, &topic),
    }
}

/// A verified symmetric `Request`: the policy is the relay acceptance
/// instance (the symmetric handshake establishes relay-class links; in
/// symmetric mode that instance is configured with the symmetric predicate).
/// One decision covers both directions — an accept records the emitter as
/// **both** relay downstream and relay upstream (`Active`) and replies
/// `Accepted` under the symmetric vocabulary. The refusal arms are the
/// shared ones; a capacity refusal inserts nothing, so no one-sided half of
/// the pair can survive it.
///
/// A **crossing** — the emitter is a peer this node's own dial is already
/// awaiting — short-circuits ahead of the policy (ADR 0042): answering the
/// node's own selection is not an admission decision, so it faces neither
/// gate nor cap and spends no budget. Sound without re-verification: the
/// pair predicate is symmetric, so this node's own dial already proved the
/// edge, and membership was checked when it dialed. A granted admission of
/// a **fresh** request (no pending dial) spends one budget unit — the count
/// a symmetric acceptance instance's cap bounds; the prelude's idempotent
/// re-accept of an already-held link spends nothing.
fn handle_request(state: &mut NodeState, emitter: PeerId, topic: TopicId) -> Vec<Effect> {
    if !super::gate_synced(state, &emitter, &topic) {
        return Vec::new();
    }
    let key = LinkKey::new(topic.clone(), emitter.clone(), LinkKind::Relay);
    if state.upstream.get(&key) == Some(&LinkState::AwaitingAccept) {
        state.downstream.insert(key.clone(), LinkState::Active);
        state.upstream.insert(key, LinkState::Active);
        return super::accepted_reply(state, emitter, topic, KIND);
    }
    let strategy = Arc::clone(&state.acceptance_strategy);
    let admission = strategy.admit(&emitter, &topic, &state.view());
    match admission {
        Admission::Accept => {
            // Only a FIRST insertion is an admission — the idempotent
            // re-accept of an already-held link spends no budget.
            if state
                .downstream
                .insert(key.clone(), LinkState::Active)
                .is_none()
            {
                *state
                    .admitted_counts
                    .entry((topic.clone(), LinkKind::Relay))
                    .or_insert(0) += 1;
            }
            state.upstream.insert(key, LinkState::Active);
            super::accepted_reply(state, emitter, topic, KIND)
        }
        Admission::RejectMembership | Admission::RejectIllegitimate => {
            super::silent_refusal(state, admission, &emitter, &topic)
        }
        Admission::RejectOverCapacity => super::reject_over_capacity(state, emitter, topic, KIND),
    }
}

/// A verified symmetric `Accepted` — the dialer's mirror step: an acceptance
/// of this node's own dial activates the upstream entry AND inserts the
/// downstream mirror, completing the pair on this end. An upstream entry
/// already `Active` is re-affirmed idempotently rather than treated as
/// unsolicited: in the crossing case this node accepted the peer's own
/// request (inserting both halves) while its dial's acceptance was still in
/// flight. Only an acceptance matching no upstream entry at all is dropped.
fn handle_accepted(state: &mut NodeState, emitter: &PeerId, topic: &TopicId) -> Vec<Effect> {
    let key = LinkKey::new(topic.clone(), emitter.clone(), LinkKind::Relay);
    if state.upstream.contains_key(&key) {
        state.upstream.insert(key.clone(), LinkState::Active);
        state.downstream.insert(key, LinkState::Active);
        return Vec::new();
    }
    super::log_unsolicited(state, "unsolicited_accept", emitter, topic);
    Vec::new()
}

/// A verified symmetric `Rejected`: the peer refused this node's dial (one
/// accept decision per edge, so the whole edge simply does not form — on
/// both ends alike). Drops the pending upstream entry; no mirror exists yet,
/// so nothing else is held.
fn handle_rejected(state: &mut NodeState, emitter: &PeerId, topic: &TopicId) -> Vec<Effect> {
    let key = LinkKey::new(topic.clone(), emitter.clone(), LinkKind::Relay);
    if !super::drop_rejected_dial(&mut state.upstream, &key) {
        super::log_unsolicited(state, "unsolicited_reject", emitter, topic);
    }
    Vec::new()
}
