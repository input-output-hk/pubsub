// Properties and invariants for the Topic Registry smart contract.
//
// Each property is first expressed as a `pure def` taking a TopicRegistry
// parameter — making it independently testable without a running state
// machine — and then lifted to a `val` that references the live `registry`
// variable from topic_registry_env.
//
// Structural invariants:
//   inv_aliveTopicHasOwner             — every live topic has ≥ 1 owner
//   inv_counterMatchesRegistry         — counter equals the number of ever-created topics
//   inv_topicIDsBoundedByCounter       — every assigned topicID < counter
//   inv_deletedTopicsHaveNoRoles       — deleted topics have no owners/admins/publishers
//
// Messaging infrastructure invariants (Group B):
//   inv_logContainsOnlyRequests        — audit log never contains response messages
//   inv_outboxContainsOnlyResponses    — outbox never contains request messages
//   inv_outboxSizeMatchesLog           — every processed request produces exactly one response
//   inv_authLogSizeMatchesLog          — every processed message has an authorization record
//
// Epoch invariants (Group C):
//   inv_publishedAtEpochLeCurrentEpoch — no topic is stamped with a future epoch
//                                        (retentionPeriod is stored but expiration is client-side)
//
// Ownership semantics invariants (Group D):
//   inv_adminsAlwaysManageable         — alive topics with admins always have an owner
//   inv_publishersAlwaysManageable     — alive topics with publishers have an owner or admin
//
// Fix regression invariants (Group A):
//   inv_aliveTopic_has_positive_R_and_T — alive topics always have R > 0 and T > 0
//   inv_tombstoneIsClean                — dead topics are fully zeroed tombstones
//
// Authorization post-condition invariants:
//   inv_successImpliesAuthorized        — successful auth-required responses had authorized senders
//   inv_unauthorizedImpliesFailure      — unauthorized senders always get failure responses
//
// Temporal properties (liveness):
//   temp_inboxEventuallyDrained         — every enqueued request is eventually processed

module topic_registry_props {

    import types.* from "./types"
    import basicSpells.* from "spells/basicSpells"
    import topic_registry_env.* from "./topic_registry_env"

    // =========================================================================
    // Pure predicate versions (reusable in unit tests)
    // =========================================================================

    // 1. Every alive topic has at least one owner.
    //    This is the core ownership safety property: the contract must never
    //    leave a topic in a state where no one can manage or delete it.
    pure def pred_aliveTopicHasOwner(reg: TopicRegistry): bool =
        reg.topics.keys().forall(tid =>
            not(reg.topics.get(tid).alive) or
            reg.topics.get(tid).owners.size() >= 1
        )

    // 2. The counter equals the number of topic entries in the map.
    //    Topics are never removed from the map (only soft-deleted), so the
    //    counter and the map size must grow together.
    pure def pred_counterMatchesRegistry(reg: TopicRegistry): bool =
        reg.counter == reg.topics.keys().size()

    // 3. Every topicID stored in the map is strictly less than the counter.
    //    This confirms that IDs are assigned sequentially and never invented
    //    out of thin air.
    pure def pred_topicIDsBoundedByCounter(reg: TopicRegistry): bool =
        reg.topics.keys().forall(tid => tid.topicID < reg.counter)

    // Helper: true iff the message is a request (not a response).
    pure def isRequest(msg: Message): bool =
        match msg {
            | CreateTopicRequest(_)          => true
            | DeleteTopicRequest(_)          => true
            | AddOwnerRequest(_)             => true
            | RemoveOwnerRequest(_)          => true
            | AddAdminRequest(_)             => true
            | RemoveAdminRequest(_)          => true
            | AddPublisherRequest(_)         => true
            | RemovePublisherRequest(_)      => true
            | SetReplicationFactorRequest(_) => true
            | SetRetentionPeriodRequest(_)   => true
            | _                             => false
        }

    // B1. The audit log contains only request messages.
    //     Responses are produced by the contract, not consumed by it, so they
    //     should never appear as processed inbox messages.
    pure def pred_logContainsOnlyRequests(reg: TopicRegistry): bool =
        reg.log.forall(msg => isRequest(msg))

    // B2. The outbox contains only response messages.
    //     The contract only emits responses; request messages must never be
    //     placed on the outbox.
    pure def pred_outboxContainsOnlyResponses(reg: TopicRegistry): bool =
        reg.outbox.foldl(true, (acc, msg) => acc and not(isRequest(msg)))

    // B3. Outbox length equals log size.
    //     Every processed request produces exactly one response, so both
    //     collections must grow in lockstep.
    pure def pred_outboxSizeMatchesLog(reg: TopicRegistry): bool =
        length(reg.outbox) == reg.log.size()

    // B4. authLog size equals log size.
    //     Every processed message gets exactly one authorization record.
    pure def pred_authLogSizeMatchesLog(reg: TopicRegistry): bool =
        reg.authLog.size() == reg.log.size()

    // Helper: extract {nonce, success} from any response; returns None for
    // CreateTopicResponse (open to all) and request messages.
    pure def responseNonceAndSuccess(msg: Message): Option[{nonce: int, success: bool}] =
        match msg {
            | CreateTopicResponse(r)          => Some({ nonce: r.cfg.nonce, success: r.topicID != None })
            | DeleteTopicResponse(r)          => Some({ nonce: r.cfg.nonce, success: r.success })
            | AddOwnerResponse(r)             => Some({ nonce: r.cfg.nonce, success: r.success })
            | RemoveOwnerResponse(r)          => Some({ nonce: r.cfg.nonce, success: r.success })
            | AddAdminResponse(r)             => Some({ nonce: r.cfg.nonce, success: r.success })
            | RemoveAdminResponse(r)          => Some({ nonce: r.cfg.nonce, success: r.success })
            | AddPublisherResponse(r)         => Some({ nonce: r.cfg.nonce, success: r.success })
            | RemovePublisherResponse(r)      => Some({ nonce: r.cfg.nonce, success: r.success })
            | SetReplicationFactorResponse(r) => Some({ nonce: r.cfg.nonce, success: r.success })
            | SetRetentionPeriodResponse(r)   => Some({ nonce: r.cfg.nonce, success: r.success })
            | _                               => None
        }

    // 5. Every successful auth-required response corresponds to an authorized request.
    //    Checks that no operation that requires a role ever succeeds for an
    //    unauthorized sender (authorization checked against the pre-state at the
    //    time of processing, recorded in authLog).
    pure def pred_successImpliesAuthorized(reg: TopicRegistry): bool =
        reg.outbox.foldl(true, (acc, msg) =>
            acc and match responseNonceAndSuccess(msg) {
                | None       => true
                | Some(r)    =>
                    not(r.success) or
                    reg.authLog.exists(a => a.nonce == r.nonce and a.authorized)
            }
        )

    // 6. Every unauthorized request produces a failure response.
    //    Checks that the contract never silently succeeds for an unauthorized sender.
    pure def pred_unauthorizedImpliesFailure(reg: TopicRegistry): bool =
        reg.authLog.forall(entry =>
            entry.authorized or
            reg.outbox.foldl(true, (acc, msg) =>
                acc and match responseNonceAndSuccess(msg) {
                    | None    => true
                    | Some(r) => r.nonce != entry.nonce or not(r.success)
                }
            )
        )

    // 4. Deleted topics carry no role memberships.
    //    After a deleteTopic the contract clears owners, admins, and publishers,
    //    so no key should be able to claim authority over a deleted topic.
    pure def pred_deletedTopicsHaveNoRoles(reg: TopicRegistry): bool =
        reg.topics.keys().forall(tid =>
            reg.topics.get(tid).alive or (
                reg.topics.get(tid).owners.size()     == 0 and
                reg.topics.get(tid).admins.size()     == 0 and
                reg.topics.get(tid).publishers.size() == 0
            )
        )

    // A1. Every alive topic has a positive replication factor and retention period.
    //     Guards fix #5: R > 0 and T > 0 validation in setReplicationFactor /
    //     setRetentionPeriod, and the initial values supplied at createTopic.
    pure def pred_aliveTopic_has_positive_R_and_T(reg: TopicRegistry): bool =
        reg.topics.keys().forall(tid =>
            not(reg.topics.get(tid).alive) or (
                reg.topics.get(tid).replicationFactor > 0 and
                reg.topics.get(tid).retentionPeriod    > 0
            )
        )

    // A2. Every dead topic is a clean tombstone: name is empty, R = 0, T = 0,
    //     publishedAtEpoch = 0, and all role sets are empty.
    //     Guards fix #1: processDeleteTopic must clear all topic data, not just
    //     the role sets.
    pure def pred_tombstoneIsClean(reg: TopicRegistry): bool =
        reg.topics.keys().forall(tid =>
            reg.topics.get(tid).alive or (
                reg.topics.get(tid).name             == ""   and
                reg.topics.get(tid).replicationFactor == 0   and
                reg.topics.get(tid).retentionPeriod   == 0   and
                reg.topics.get(tid).publishedAtEpoch  == 0   and
                reg.topics.get(tid).owners.size()     == 0   and
                reg.topics.get(tid).admins.size()     == 0   and
                reg.topics.get(tid).publishers.size() == 0
            )
        )

    // C1. For every alive topic, publishedAtEpoch <= current epoch.
    //     A topic cannot be stamped with a future epoch: the registry stamps
    //     publishedAtEpoch = registry.epoch at creation time, so it can never
    //     exceed the epoch value that existed at that point, and the epoch only
    //     grows from there.
    pure def pred_publishedAtEpochLeCurrentEpoch(reg: TopicRegistry): bool =
        reg.topics.keys().forall(tid =>
            not(reg.topics.get(tid).alive) or
            reg.topics.get(tid).publishedAtEpoch <= reg.epoch
        )

    // D1. Every alive topic with admins has at least one owner.
    //     Ensures no admin is ever "stuck": an owner always exists who can
    //     remove any admin (only owners may call removeAdmin).
    pure def pred_adminsAlwaysManageable(reg: TopicRegistry): bool =
        reg.topics.keys().forall(tid =>
            not(reg.topics.get(tid).alive)                  or
            reg.topics.get(tid).admins.size() == 0          or
            reg.topics.get(tid).owners.size() >= 1
        )

    // D2. Every alive topic with a non-empty publishers set has at least one
    //     owner or admin.  Ensures the publisher list can always be managed
    //     (only owners and admins may call addPublisher / removePublisher).
    pure def pred_publishersAlwaysManageable(reg: TopicRegistry): bool =
        reg.topics.keys().forall(tid =>
            not(reg.topics.get(tid).alive)                  or
            reg.topics.get(tid).publishers.size() == 0      or
            reg.topics.get(tid).owners.size()     >= 1      or
            reg.topics.get(tid).admins.size()     >= 1
        )

    // =========================================================================
    // Invariants referencing the live state
    // =========================================================================

    val inv_aliveTopicHasOwner =
        pred_aliveTopicHasOwner(registry)

    val inv_counterMatchesRegistry =
        pred_counterMatchesRegistry(registry)

    val inv_topicIDsBoundedByCounter =
        pred_topicIDsBoundedByCounter(registry)

    val inv_deletedTopicsHaveNoRoles =
        pred_deletedTopicsHaveNoRoles(registry)

    val inv_logContainsOnlyRequests =
        pred_logContainsOnlyRequests(registry)

    val inv_outboxContainsOnlyResponses =
        pred_outboxContainsOnlyResponses(registry)

    val inv_outboxSizeMatchesLog =
        pred_outboxSizeMatchesLog(registry)

    val inv_authLogSizeMatchesLog =
        pred_authLogSizeMatchesLog(registry)

    val inv_publishedAtEpochLeCurrentEpoch =
        pred_publishedAtEpochLeCurrentEpoch(registry)

    val inv_adminsAlwaysManageable =
        pred_adminsAlwaysManageable(registry)

    val inv_publishersAlwaysManageable =
        pred_publishersAlwaysManageable(registry)

    val inv_aliveTopic_has_positive_R_and_T =
        pred_aliveTopic_has_positive_R_and_T(registry)

    val inv_tombstoneIsClean =
        pred_tombstoneIsClean(registry)

    val inv_successImpliesAuthorized =
        pred_successImpliesAuthorized(registry)

    val inv_unauthorizedImpliesFailure =
        pred_unauthorizedImpliesFailure(registry)

    // =========================================================================
    // Temporal properties
    // =========================================================================

    // 5. Every request enqueued into the inbox is eventually processed.
    //    Requires weak fairness of contract_step: if the inbox is non-empty
    //    (i.e. contract_step is enabled) continuously, it must eventually fire.
    //    Without this fairness assumption the environment can starve the contract
    //    by enqueuing messages indefinitely, which TLC confirmed as a violation.
    temporal temp_inboxEventuallyDrained =
        weakFair(contract_step, registry) implies always(eventually(length(registry.inbox) == 0))
}
    