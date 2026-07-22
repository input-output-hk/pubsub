//! Per-handshake connection-control handlers (ADR 0033).
//!
//! The message vocabulary names the establishment protocol —
//! [`Message::RelayConnection`](crate::message::Message),
//! [`Message::PublisherConnection`](crate::message::Message),
//! [`Message::SymmetricConnection`](crate::message::Message) — and `apply`'s
//! dispatch routes each variant to its module here ([`relay`], [`publisher`],
//! [`symmetric`]), so no handler recovers the handshake by testing a kind
//! field mid-flight. The lifecycle mechanics every handshake shares — the
//! verification prelude, the readiness gate on inbound requests, the refusal
//! arms, dial promotion, teardown — live in this module as helper functions
//! the per-kind handlers compose.

pub(super) mod publisher;
pub(super) mod relay;
pub(super) mod symmetric;

use std::collections::BTreeMap;

use super::{signed_connection, Effect, NodeState};
use crate::connection_state::{LinkKey, LinkKind, LinkState};
use crate::message::{ConnectionAction, ConnectionMessage, HandshakeKind, PlainConnection};
use crate::peer::PeerId;
use crate::strategies::acceptance::Admission;
use crate::topic::TopicId;

/// The verification prelude every handshake shares (data-model §4), run on
/// the **carried emitter** — the transport frame's sender is never consulted
/// (FR-011/015): the carried emitter must not be the node itself, and the
/// signature must verify over the handshake's preimage under the emitter's
/// key (the handshake kind is bound in, so a control message cannot be
/// replayed across vocabularies). Returns the validated content, or `None`
/// after a cause-tagged drop.
pub(super) fn verified(
    state: &NodeState,
    connection: ConnectionMessage,
    kind: HandshakeKind,
) -> Option<PlainConnection> {
    let ConnectionMessage { plain, signature } = connection;

    // FR-015: a control message whose carried emitter is the node itself is
    // dropped (checked before signature verification — self-connections are
    // unrepresentable end to end).
    if plain.emitter == state.self_id {
        tracing::info!(
            target: "pubsub_node::node",
            event = "message_dropped",
            cause = "self_emitter",
            self_id = %state.self_id,
        );
        return None;
    }

    // FR-011/015: verify the signature against the carried emitter's key.
    if state
        .verifier
        .verify(
            plain.emitter.as_public_key(),
            &plain.signed_bytes(kind),
            &signature,
        )
        .is_err()
    {
        tracing::info!(
            target: "pubsub_node::node",
            event = "message_dropped",
            cause = "invalid_signature",
            self_id = %state.self_id,
            emitter = %plain.emitter,
        );
        return None;
    }

    Some(plain)
}

/// The readiness gate on inbound requests. Before `Synced` the candidate view
/// is partially folded, so a bucket count derived from it can floor to 1 and
/// the edge predicate degenerate to always-true — an un-synced acceptor would
/// fail OPEN, admitting an edge the full view would reject (and the
/// idempotent re-accept would then pin it). Drop silently until readiness.
/// This closes the pre-snapshot window only; post-sync membership deltas keep
/// the documented B-agreement assumption in play (ADR 0031).
pub(super) fn gate_synced(state: &NodeState, emitter: &PeerId, topic: &TopicId) -> bool {
    if state.synced {
        return true;
    }
    tracing::info!(
        target: "pubsub_node::node",
        event = "message_dropped",
        cause = "not_synced",
        self_id = %state.self_id,
        emitter = %emitter,
        topic = %topic,
    );
    false
}

/// The `Accepted` reply to a request this node just admitted, under the
/// accepting handshake's vocabulary.
pub(super) fn accepted_reply(
    state: &NodeState,
    to: PeerId,
    topic: TopicId,
    kind: HandshakeKind,
) -> Vec<Effect> {
    let message = signed_connection(
        &state.self_id,
        state.signer.as_ref(),
        kind,
        ConnectionAction::Accepted { topic },
    );
    vec![Effect::Send { to, message }]
}

/// The two silent refusals (ADR 0025): no reply, leaking nothing to the
/// requester (a non-member, or an adversary whose edge predicate does not
/// hold this interval). Distinct log causes only.
pub(super) fn silent_refusal(
    state: &NodeState,
    admission: Admission,
    emitter: &PeerId,
    topic: &TopicId,
) -> Vec<Effect> {
    let cause = if admission == Admission::RejectMembership {
        "membership_validation_failed"
    } else {
        "illegitimate_request"
    };
    tracing::info!(
        target: "pubsub_node::node",
        event = "message_dropped",
        cause,
        self_id = %state.self_id,
        emitter = %emitter,
        topic = %topic,
    );
    Vec::new()
}

/// The over-capacity refusal (ADR 0025): drop without recording anything, but
/// send an explicit `Rejected` under the refusing handshake's vocabulary so
/// the dialer drops its pending entry. Not misbehaviour — no severance.
pub(super) fn reject_over_capacity(
    state: &NodeState,
    emitter: PeerId,
    topic: TopicId,
    kind: HandshakeKind,
) -> Vec<Effect> {
    tracing::info!(
        target: "pubsub_node::node",
        event = "message_dropped",
        cause = "downstream_capacity_reached",
        self_id = %state.self_id,
        emitter = %emitter,
        topic = %topic,
    );
    let message = signed_connection(
        &state.self_id,
        state.signer.as_ref(),
        kind,
        ConnectionAction::Rejected { topic },
    );
    vec![Effect::Send {
        to: emitter,
        message,
    }]
}

/// Promote the dialed entry `key` in `dialed` from `AwaitingAccept` to
/// `Active`. `false` — nothing pending (absent, or already `Active`) — means
/// the acceptance was unsolicited under the promoting handshake's rules.
pub(super) fn promote_dialed(dialed: &mut BTreeMap<LinkKey, LinkState>, key: &LinkKey) -> bool {
    if let Some(entry) = dialed.get_mut(key) {
        if *entry == LinkState::AwaitingAccept {
            *entry = LinkState::Active;
            return true;
        }
    }
    false
}

/// Drop the pending `AwaitingAccept` dial `key` from `dialed` — the peer
/// refused it for over-capacity, so the dialer stops waiting on an `Accepted`
/// that will never come. There is no retry and no back-fill (the realized
/// degree may settle below target; re-forming links is a future
/// heartbeat/reshuffle layer). `false` — no pending entry — means the
/// rejection was unsolicited.
pub(super) fn drop_rejected_dial(dialed: &mut BTreeMap<LinkKey, LinkState>, key: &LinkKey) -> bool {
    if matches!(dialed.get(key), Some(LinkState::AwaitingAccept)) {
        dialed.remove(key);
        return true;
    }
    false
}

/// Transition for a verified `Terminated` from `emitter` on `topic` for a
/// link of `kind`: removes the matching entry in either direction (both, if
/// both are held); a coexisting link of the *other* kind to the same
/// peer/topic is untouched. A `Terminated` for a link not held is dropped; a
/// `Terminated` is never replied to.
pub(super) fn remove_terminated(
    state: &mut NodeState,
    emitter: &PeerId,
    topic: &TopicId,
    kind: LinkKind,
) -> Vec<Effect> {
    let key = LinkKey::new(topic.clone(), emitter.clone(), kind);
    let removed_upstream = state.upstream.remove(&key).is_some();
    let removed_downstream = state.downstream.remove(&key).is_some();
    if !(removed_upstream || removed_downstream) {
        log_unsolicited(state, "unknown_termination", emitter, topic);
    }
    Vec::new()
}

/// Cause-tagged drop log for a control message that matched no held or
/// pending link (an unsolicited accept/reject, an unknown termination).
pub(super) fn log_unsolicited(
    state: &NodeState,
    cause: &'static str,
    emitter: &PeerId,
    topic: &TopicId,
) {
    tracing::info!(
        target: "pubsub_node::node",
        event = "message_dropped",
        cause,
        self_id = %state.self_id,
        emitter = %emitter,
        topic = %topic,
    );
}
