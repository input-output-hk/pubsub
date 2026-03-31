// Topic Registry Smart Contract
//
// Models the Cardano on-chain smart contract described in the Pub/Sub
// Architecture report (Chapter 2). The contract manages topic lifecycle
// and enforces role-based access control (owners, admins, publishers).
//
// Authorization rules (from Table 2.1):
//   - createTopic:             any node (sender becomes owner)
//   - deleteTopic:             owner only
//   - addOwner/removeOwner:    owner only  (removeOwner: cannot remove last owner)
//   - addAdmin/removeAdmin:    owner only
//   - addPublisher/removePublisher: owner or admin
//   - setReplicationFactor:    owner or admin
//   - setRetentionPeriod:      owner or admin
//
// Note: topic expiration is a client-side concern and is not enforced by the contract.

module topic_registry {

    import types.* from "./types"
    import basicSpells.* from "spells/basicSpells"

    // -------------------------------------------------------------------------
    // Helpers
    // -------------------------------------------------------------------------

    pure def topicAlive(registry: TopicRegistry, topicID: TopicID): bool =
        registry.topics.has(topicID) and registry.topics.get(topicID).alive

    pure def isOwner(registry: TopicRegistry, topicID: TopicID, pk: PublicKey): bool =
        topicAlive(registry, topicID) and
        registry.topics.get(topicID).owners.contains(pk)

    pure def isAdmin(registry: TopicRegistry, topicID: TopicID, pk: PublicKey): bool =
        topicAlive(registry, topicID) and
        registry.topics.get(topicID).admins.contains(pk)

    pure def isOwnerOrAdmin(registry: TopicRegistry, topicID: TopicID, pk: PublicKey): bool =
        isOwner(registry, topicID, pk) or isAdmin(registry, topicID, pk)

    // Returns the nonce of a request message; -1 for response messages
    // (responses are never processed by the contract so -1 is safe).
    pure def msgNonce(msg: Message): int =
        match msg {
            | CreateTopicRequest(r)          => r.cfg.nonce
            | DeleteTopicRequest(r)          => r.cfg.nonce
            | AddOwnerRequest(r)             => r.cfg.nonce
            | RemoveOwnerRequest(r)          => r.cfg.nonce
            | AddAdminRequest(r)             => r.cfg.nonce
            | RemoveAdminRequest(r)          => r.cfg.nonce
            | AddPublisherRequest(r)         => r.cfg.nonce
            | RemovePublisherRequest(r)      => r.cfg.nonce
            | SetReplicationFactorRequest(r) => r.cfg.nonce
            | SetRetentionPeriodRequest(r)   => r.cfg.nonce
            | _                              => -1
        }

    // Checks whether a request is authorized against the current registry state.
    // createTopic is open to all; response messages return false.
    pure def isAuthorizedMsg(registry: TopicRegistry, msg: Message): bool =
        match msg {
            | CreateTopicRequest(_)          => true
            | DeleteTopicRequest(r)          => isOwner(registry, r.topicID, r.cfg.sender)
            | AddOwnerRequest(r)             => isOwner(registry, r.topicID, r.cfg.sender)
            | RemoveOwnerRequest(r)          => isOwner(registry, r.topicID, r.cfg.sender)
            | AddAdminRequest(r)             => isOwner(registry, r.topicID, r.cfg.sender)
            | RemoveAdminRequest(r)          => isOwner(registry, r.topicID, r.cfg.sender)
            | AddPublisherRequest(r)         => isOwnerOrAdmin(registry, r.topicID, r.cfg.sender)
            | RemovePublisherRequest(r)      => isOwnerOrAdmin(registry, r.topicID, r.cfg.sender)
            | SetReplicationFactorRequest(r) => isOwnerOrAdmin(registry, r.topicID, r.cfg.sender)
            | SetRetentionPeriodRequest(r)   => isOwnerOrAdmin(registry, r.topicID, r.cfg.sender)
            | _                              => false
        }

    // Produces the tombstone record kept after a topic is deleted.
    // All data is cleared; only the topicID key and alive=false are retained.
    pure def tombstone: Topic = {
        name             : "",
        admins           : Set(),
        owners           : Set(),
        publishers       : Set(),
        replicationFactor: 0,
        retentionPeriod  : 0,
        alive            : false,
        publishedAtEpoch : 0,
    }

    // -------------------------------------------------------------------------
    // Operation implementations
    // -------------------------------------------------------------------------

    // createTopic: the caller becomes the sole initial owner regardless of what
    // topicData.owners contains — the spec's createTopic signature has no owners[]
    // parameter. Initial admins and publishers from topicData are kept as-is.
    // publishedAtEpoch is set to the current epoch so retention is measured from creation.
    // R and T must be positive. Returns the new topicID on success.
    pure def processCreateTopic(registry: TopicRegistry, cfg: MsgCfg, topicData: Topic): Transition =
        val failResp = CreateTopicResponse({ cfg: cfg, topicID: None })
        if (topicData.replicationFactor <= 0 or topicData.retentionPeriod <= 0)
            { post_state: registry, response: Some(failResp) }
        else
            val newID: TopicID = { topicID: registry.counter }
            val newTopic: Topic = {
                ...topicData,
                owners          : Set(cfg.sender),
                alive           : true,
                publishedAtEpoch: registry.epoch,
            }
            val newState: TopicRegistry = {
                ...registry,
                topics: registry.topics.put(newID, newTopic),
                counter: registry.counter + 1,
            }
            { post_state: newState, response: Some(CreateTopicResponse({ cfg: cfg, topicID: Some(newID) })) }

    // deleteTopic: soft-delete — keeps the topicID in the registry forever to
    // prevent reassignment, but marks the topic as not alive and clears its data.
    pure def processDeleteTopic(registry: TopicRegistry, cfg: MsgCfg, topicID: TopicID): Transition =
        val failResp = DeleteTopicResponse({ cfg: cfg, success: false })
        if (not(isOwner(registry, topicID, cfg.sender)))
            { post_state: registry, response: Some(failResp) }
        else
            val newState = { ...registry, topics: registry.topics.put(topicID, tombstone) }
            { post_state: newState, response: Some(DeleteTopicResponse({ cfg: cfg, success: true })) }

    // addOwner: only existing owners may add new owners.
    pure def processAddOwner(registry: TopicRegistry, cfg: MsgCfg, topicID: TopicID, newOwner: PublicKey): Transition =
        val failResp = AddOwnerResponse({ cfg: cfg, success: false })
        if (not(isOwner(registry, topicID, cfg.sender)))
            { post_state: registry, response: Some(failResp) }
        else
            val topic = registry.topics.get(topicID)
            val newTopic = { ...topic, owners: topic.owners.union(Set(newOwner)) }
            val newState = { ...registry, topics: registry.topics.put(topicID, newTopic) }
            { post_state: newState, response: Some(AddOwnerResponse({ cfg: cfg, success: true })) }

    // removeOwner: only owners may remove owners; the last owner cannot be removed;
    // the target must actually be an owner.
    pure def processRemoveOwner(registry: TopicRegistry, cfg: MsgCfg, topicID: TopicID, oldOwner: PublicKey): Transition =
        val failResp = RemoveOwnerResponse({ cfg: cfg, success: false })
        if (not(isOwner(registry, topicID, cfg.sender)))
            { post_state: registry, response: Some(failResp) }
        else
            // get is safe here: isOwner implies topicAlive implies the key exists
            val topic = registry.topics.get(topicID)
            if (not(topic.owners.contains(oldOwner)) or topic.owners.size() == 1)
                { post_state: registry, response: Some(failResp) }
            else
                val newTopic = { ...topic, owners: topic.owners.exclude(Set(oldOwner)) }
                val newState = { ...registry, topics: registry.topics.put(topicID, newTopic) }
                { post_state: newState, response: Some(RemoveOwnerResponse({ cfg: cfg, success: true })) }

    // addAdmin: only owners may add admins.
    pure def processAddAdmin(registry: TopicRegistry, cfg: MsgCfg, topicID: TopicID, newAdmin: PublicKey): Transition =
        val failResp = AddAdminResponse({ cfg: cfg, success: false })
        if (not(isOwner(registry, topicID, cfg.sender)))
            { post_state: registry, response: Some(failResp) }
        else
            val topic = registry.topics.get(topicID)
            val newTopic = { ...topic, admins: topic.admins.union(Set(newAdmin)) }
            val newState = { ...registry, topics: registry.topics.put(topicID, newTopic) }
            { post_state: newState, response: Some(AddAdminResponse({ cfg: cfg, success: true })) }

    // removeAdmin: only owners may remove admins; the target must actually be an admin.
    pure def processRemoveAdmin(registry: TopicRegistry, cfg: MsgCfg, topicID: TopicID, oldAdmin: PublicKey): Transition =
        val failResp = RemoveAdminResponse({ cfg: cfg, success: false })
        if (not(isOwner(registry, topicID, cfg.sender)))
            { post_state: registry, response: Some(failResp) }
        else
            val topic = registry.topics.get(topicID)
            if (not(topic.admins.contains(oldAdmin)))
                { post_state: registry, response: Some(failResp) }
            else
                val newTopic = { ...topic, admins: topic.admins.exclude(Set(oldAdmin)) }
                val newState = { ...registry, topics: registry.topics.put(topicID, newTopic) }
                { post_state: newState, response: Some(RemoveAdminResponse({ cfg: cfg, success: true })) }

    // addPublisher: owners and admins may add publishers.
    // An empty publishers set means the topic is open (any node may publish).
    pure def processAddPublisher(registry: TopicRegistry, cfg: MsgCfg, topicID: TopicID, newPublisher: PublicKey): Transition =
        val failResp = AddPublisherResponse({ cfg: cfg, success: false })
        if (not(isOwnerOrAdmin(registry, topicID, cfg.sender)))
            { post_state: registry, response: Some(failResp) }
        else
            val topic = registry.topics.get(topicID)
            val newTopic = { ...topic, publishers: topic.publishers.union(Set(newPublisher)) }
            val newState = { ...registry, topics: registry.topics.put(topicID, newTopic) }
            { post_state: newState, response: Some(AddPublisherResponse({ cfg: cfg, success: true })) }

    // removePublisher: owners and admins may remove publishers; the target must
    // actually be a publisher.
    pure def processRemovePublisher(registry: TopicRegistry, cfg: MsgCfg, topicID: TopicID, oldPublisher: PublicKey): Transition =
        val failResp = RemovePublisherResponse({ cfg: cfg, success: false })
        if (not(isOwnerOrAdmin(registry, topicID, cfg.sender)))
            { post_state: registry, response: Some(failResp) }
        else
            val topic = registry.topics.get(topicID)
            if (not(topic.publishers.contains(oldPublisher)))
                { post_state: registry, response: Some(failResp) }
            else
                val newTopic = { ...topic, publishers: topic.publishers.exclude(Set(oldPublisher)) }
                val newState = { ...registry, topics: registry.topics.put(topicID, newTopic) }
                { post_state: newState, response: Some(RemovePublisherResponse({ cfg: cfg, success: true })) }

    // setReplicationFactor: owners and admins may update R; R must be positive.
    pure def processSetReplicationFactor(registry: TopicRegistry, cfg: MsgCfg, topicID: TopicID, R: int): Transition =
        val failResp = SetReplicationFactorResponse({ cfg: cfg, success: false })
        if (not(isOwnerOrAdmin(registry, topicID, cfg.sender)))
            { post_state: registry, response: Some(failResp) }
        else if (R <= 0)
            { post_state: registry, response: Some(failResp) }
        else
            val topic = registry.topics.get(topicID)
            val newTopic = { ...topic, replicationFactor: R }
            val newState = { ...registry, topics: registry.topics.put(topicID, newTopic) }
            { post_state: newState, response: Some(SetReplicationFactorResponse({ cfg: cfg, success: true })) }

    // setRetentionPeriod: owners and admins may update T; T must be positive.
    pure def processSetRetentionPeriod(registry: TopicRegistry, cfg: MsgCfg, topicID: TopicID, T: int): Transition =
        val failResp = SetRetentionPeriodResponse({ cfg: cfg, success: false })
        if (not(isOwnerOrAdmin(registry, topicID, cfg.sender)))
            { post_state: registry, response: Some(failResp) }
        else if (T <= 0)
            { post_state: registry, response: Some(failResp) }
        else
            val topic = registry.topics.get(topicID)
            val newTopic = { ...topic, retentionPeriod: T }
            val newState = { ...registry, topics: registry.topics.put(topicID, newTopic) }
            { post_state: newState, response: Some(SetRetentionPeriodResponse({ cfg: cfg, success: true })) }

    // -------------------------------------------------------------------------
    // Message dispatcher
    // -------------------------------------------------------------------------

    // Dispatch a single request message to the appropriate handler.
    // Response messages that arrive in the inbox are silently ignored
    // (they are produced by the contract, not consumed by it).
    pure def processMessage(registry: TopicRegistry, msg: Message): Transition =
        match msg {
            | CreateTopicRequest(r)         => processCreateTopic(registry, r.cfg, r.topic)
            | DeleteTopicRequest(r)         => processDeleteTopic(registry, r.cfg, r.topicID)
            | AddOwnerRequest(r)            => processAddOwner(registry, r.cfg, r.topicID, r.newOwner)
            | RemoveOwnerRequest(r)         => processRemoveOwner(registry, r.cfg, r.topicID, r.oldOwner)
            | AddAdminRequest(r)            => processAddAdmin(registry, r.cfg, r.topicID, r.newAdmin)
            | RemoveAdminRequest(r)         => processRemoveAdmin(registry, r.cfg, r.topicID, r.oldAdmin)
            | AddPublisherRequest(r)        => processAddPublisher(registry, r.cfg, r.topicID, r.newPublisher)
            | RemovePublisherRequest(r)     => processRemovePublisher(registry, r.cfg, r.topicID, r.oldPublisher)
            | SetReplicationFactorRequest(r) => processSetReplicationFactor(registry, r.cfg, r.topicID, r.R)
            | SetRetentionPeriodRequest(r)  => processSetRetentionPeriod(registry, r.cfg, r.topicID, r.T)
            // Response messages are not handled by the contract
            | CreateTopicResponse(_)        => { post_state: registry, response: None }
            | DeleteTopicResponse(_)        => { post_state: registry, response: None }
            | AddOwnerResponse(_)           => { post_state: registry, response: None }
            | RemoveOwnerResponse(_)        => { post_state: registry, response: None }
            | AddAdminResponse(_)           => { post_state: registry, response: None }
            | RemoveAdminResponse(_)        => { post_state: registry, response: None }
            | AddPublisherResponse(_)       => { post_state: registry, response: None }
            | RemovePublisherResponse(_)    => { post_state: registry, response: None }
            | SetReplicationFactorResponse(_) => { post_state: registry, response: None }
            | SetRetentionPeriodResponse(_) => { post_state: registry, response: None }
        }


}
