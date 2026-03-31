// Execution environment for the Topic Registry smart contract.
//
// Separates two concerns:
//   - contract_step: the contract processes one message from the inbox
//   - env_* actions: the external world enqueues request messages
//
// The `next` action interleaves both, giving a full closed-loop model
// suitable for simulation and model checking.

module topic_registry_env {

    import types.* from "./types"
    import basicSpells.* from "spells/basicSpells"
    import topic_registry.* from "./topic_registry"

    // -------------------------------------------------------------------------
    // Bounded universe (keeps the state space finite for model checking)
    // -------------------------------------------------------------------------

    pure val PARTICIPANTS: Set[PublicKey] = Set({ pk: 1 }, { pk: 2 }, { pk: 3 })
    pure val TOPIC_NAMES:  Set[str]      = Set("alpha", "beta", "gamma")

    // -------------------------------------------------------------------------
    // State
    // -------------------------------------------------------------------------

    var registry: TopicRegistry

    // -------------------------------------------------------------------------
    // Init
    // -------------------------------------------------------------------------

    action init = {
        registry' = {
            topics   : Map(),
            counter  : 0,
            epoch    : 0,
            inbox    : List(),
            outbox   : List(),
            nonce    : 0,
            log      : Set(),
            authLog  : Set(),
        }
    }

    // -------------------------------------------------------------------------
    // Contract step: dequeue one inbox message, process it, route the response
    // to the outbox, and record the request in the audit log.
    // -------------------------------------------------------------------------

    action contract_step = all {
        length(registry.inbox) > 0,
        val msg        = head(registry.inbox)
        val authorized = isAuthorizedMsg(registry, msg)  // checked against pre-state
        val transition = processMessage(registry, msg)
        val newOutbox  = match transition.response {
            | Some(resp) => append(registry.outbox, resp)
            | None       => registry.outbox
        }
        registry' = {
            ...transition.post_state,
            inbox   : tail(registry.inbox),
            outbox  : newOutbox,
            log     : registry.log.union(Set(msg)),
            authLog : registry.authLog.union(Set({ nonce: msgNonce(msg), authorized: authorized })),
        }
    }

    // -------------------------------------------------------------------------
    // Environment actions: enqueue requests into the inbox
    // -------------------------------------------------------------------------

    // Helper: bump nonce after enqueueing a message.
    pure def enqueue(reg: TopicRegistry, msg: Message): TopicRegistry = {
        ...reg,
        inbox : append(reg.inbox, msg),
        nonce : reg.nonce + 1,
    }

    action env_createTopic = {
        nondet sender = oneOf(PARTICIPANTS)
        nondet name   = oneOf(TOPIC_NAMES)
        val cfg   = { sender: sender, nonce: registry.nonce }
        val topic = {
            name             : name,
            admins           : Set(),
            owners           : Set(),
            publishers       : Set(),
            replicationFactor: 1,
            retentionPeriod  : 1,
            alive            : false,
            publishedAtEpoch : 0,
        }
        registry' = enqueue(registry, CreateTopicRequest({ cfg: cfg, topic: topic }))
    }

    action env_deleteTopic = all {
        registry.topics.keys() != Set(),
        nondet sender  = oneOf(PARTICIPANTS)
        nondet topicID = oneOf(registry.topics.keys())
        val cfg = { sender: sender, nonce: registry.nonce }
        registry' = enqueue(registry, DeleteTopicRequest({ cfg: cfg, topicID: topicID }))
    }

    action env_addOwner = all {
        registry.topics.keys() != Set(),
        nondet sender   = oneOf(PARTICIPANTS)
        nondet topicID  = oneOf(registry.topics.keys())
        nondet newOwner = oneOf(PARTICIPANTS)
        val cfg = { sender: sender, nonce: registry.nonce }
        registry' = enqueue(registry, AddOwnerRequest({ cfg: cfg, topicID: topicID, newOwner: newOwner }))
    }

    action env_removeOwner = all {
        registry.topics.keys() != Set(),
        nondet sender   = oneOf(PARTICIPANTS)
        nondet topicID  = oneOf(registry.topics.keys())
        nondet oldOwner = oneOf(PARTICIPANTS)
        val cfg = { sender: sender, nonce: registry.nonce }
        registry' = enqueue(registry, RemoveOwnerRequest({ cfg: cfg, topicID: topicID, oldOwner: oldOwner }))
    }

    action env_addAdmin = all {
        registry.topics.keys() != Set(),
        nondet sender   = oneOf(PARTICIPANTS)
        nondet topicID  = oneOf(registry.topics.keys())
        nondet newAdmin = oneOf(PARTICIPANTS)
        val cfg = { sender: sender, nonce: registry.nonce }
        registry' = enqueue(registry, AddAdminRequest({ cfg: cfg, topicID: topicID, newAdmin: newAdmin }))
    }

    action env_removeAdmin = all {
        registry.topics.keys() != Set(),
        nondet sender   = oneOf(PARTICIPANTS)
        nondet topicID  = oneOf(registry.topics.keys())
        nondet oldAdmin = oneOf(PARTICIPANTS)
        val cfg = { sender: sender, nonce: registry.nonce }
        registry' = enqueue(registry, RemoveAdminRequest({ cfg: cfg, topicID: topicID, oldAdmin: oldAdmin }))
    }

    action env_addPublisher = all {
        registry.topics.keys() != Set(),
        nondet sender       = oneOf(PARTICIPANTS)
        nondet topicID      = oneOf(registry.topics.keys())
        nondet newPublisher = oneOf(PARTICIPANTS)
        val cfg = { sender: sender, nonce: registry.nonce }
        registry' = enqueue(registry, AddPublisherRequest({ cfg: cfg, topicID: topicID, newPublisher: newPublisher }))
    }

    action env_removePublisher = all {
        registry.topics.keys() != Set(),
        nondet sender       = oneOf(PARTICIPANTS)
        nondet topicID      = oneOf(registry.topics.keys())
        nondet oldPublisher = oneOf(PARTICIPANTS)
        val cfg = { sender: sender, nonce: registry.nonce }
        registry' = enqueue(registry, RemovePublisherRequest({ cfg: cfg, topicID: topicID, oldPublisher: oldPublisher }))
    }

    action env_setReplicationFactor = all {
        registry.topics.keys() != Set(),
        nondet sender  = oneOf(PARTICIPANTS)
        nondet topicID = oneOf(registry.topics.keys())
        nondet R       = oneOf(Set(1, 2, 3))
        val cfg = { sender: sender, nonce: registry.nonce }
        registry' = enqueue(registry, SetReplicationFactorRequest({ cfg: cfg, topicID: topicID, R: R }))
    }

    // Advance the global epoch by one tick (models time passing).
    action env_tickEpoch = {
        registry' = { ...registry, epoch: registry.epoch + 1 }
    }

    action env_setRetentionPeriod = all {
        registry.topics.keys() != Set(),
        nondet sender  = oneOf(PARTICIPANTS)
        nondet topicID = oneOf(registry.topics.keys())
        nondet T       = oneOf(Set(1, 5, 10))
        val cfg = { sender: sender, nonce: registry.nonce }
        registry' = enqueue(registry, SetRetentionPeriodRequest({ cfg: cfg, topicID: topicID, T: T }))
    }

    // -------------------------------------------------------------------------
    // Closed-loop next action
    // -------------------------------------------------------------------------

    action env_next = any {
        contract_step,
        env_createTopic,
        env_deleteTopic,
        env_addOwner,
        env_removeOwner,
        env_addAdmin,
        env_removeAdmin,
        env_addPublisher,
        env_removePublisher,
        env_setReplicationFactor,
        env_setRetentionPeriod,
        env_tickEpoch,
    }

}
