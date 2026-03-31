module topic_registry_test {

    import types.* from "./types"
    import basicSpells.* from "spells/basicSpells"
    import topic_registry.* from "./topic_registry"

    // -------------------------------------------------------------------------
    // Fixtures
    // -------------------------------------------------------------------------

    val alice: PublicKey = { pk: 1 }
    val bob:   PublicKey = { pk: 2 }
    val carol: PublicKey = { pk: 3 }

    val emptyTopic: Topic = {
        name             : "test",
        admins           : Set(),
        owners           : Set(),
        publishers       : Set(),
        replicationFactor: 3,
        retentionPeriod  : 10,
        alive            : false,
        publishedAtEpoch : 0,
    }

    val emptyRegistry: TopicRegistry = {
        topics   : Map(),
        counter  : 0,
        epoch    : 0,
        inbox    : List(),
        outbox   : List(),
        nonce    : 0,
        log      : Set(),
        authLog  : Set(),
    }

    // -------------------------------------------------------------------------
    // Tests
    // -------------------------------------------------------------------------

    // Alice creates a topic and becomes the sole owner.
    run createTopicTest = {
        val cfg = { sender: alice, nonce: 0 }
        val t   = processCreateTopic(emptyRegistry, cfg, emptyTopic)
        val tid = { topicID: 0 }
        all {
            assert(t.post_state.topics.has(tid)),
            assert(t.post_state.topics.get(tid).alive),
            assert(t.post_state.topics.get(tid).owners.contains(alice)),
            assert(t.post_state.counter == 1),
            assert(t.response == Some(CreateTopicResponse({ cfg: cfg, topicID: Some(tid) }))),
        }
    }

    // Pre-populated owners in the request are ignored: only the sender becomes
    // owner, regardless of what topicData.owners contains (bug #4 regression).
    run createTopicIgnoresPresetOwnersTest = {
        val cfg          = { sender: alice, nonce: 0 }
        val topicWithBob = { ...emptyTopic, owners: Set(bob) }
        val t            = processCreateTopic(emptyRegistry, cfg, topicWithBob)
        val tid          = { topicID: 0 }
        all {
            assert(t.post_state.topics.get(tid).owners == Set(alice)),
            assert(not(t.post_state.topics.get(tid).owners.contains(bob))),
        }
    }

    // Only the owner can delete a topic; others get a failure response.
    run deleteTopicAuthTest = {
        val cfg    = { sender: alice, nonce: 0 }
        val reg1   = processCreateTopic(emptyRegistry, cfg, emptyTopic).post_state
        val tid    = { topicID: 0 }
        val bobCfg = { sender: bob, nonce: 1 }
        val t_fail = processDeleteTopic(reg1, bobCfg, tid)
        val t_ok   = processDeleteTopic(reg1, cfg, tid)
        all {
            assert(t_fail.post_state.topics.get(tid).alive),    // still alive
            assert(not(t_ok.post_state.topics.get(tid).alive)), // now deleted
        }
    }

    // Cannot remove the last owner.
    run lastOwnerProtectionTest = {
        val cfg  = { sender: alice, nonce: 0 }
        val reg1 = processCreateTopic(emptyRegistry, cfg, emptyTopic).post_state
        val tid  = { topicID: 0 }
        val t    = processRemoveOwner(reg1, cfg, tid, alice)
        assert(t.post_state.topics.get(tid).owners.contains(alice))
    }

    // Admin can add a publisher; non-owner/non-admin cannot.
    run publisherManagementTest = {
        val aliceCfg = { sender: alice, nonce: 0 }
        val bobCfg   = { sender: bob,   nonce: 1 }
        val carolCfg = { sender: carol, nonce: 2 }
        val tid      = { topicID: 0 }
        val reg1 = processCreateTopic(emptyRegistry, aliceCfg, emptyTopic).post_state
        val reg2 = processAddAdmin(reg1, aliceCfg, tid, bob).post_state
        val t_ok   = processAddPublisher(reg2, bobCfg, tid, carol)
        val t_fail = processAddPublisher(t_ok.post_state, carolCfg, tid, alice)
        all {
            assert(t_ok.post_state.topics.get(tid).publishers.contains(carol)),
            assert(not(t_fail.post_state.topics.get(tid).publishers.contains(alice))),
        }
    }

    // Removing a non-member owner/admin/publisher must fail (bug #2 regression).
    run removeNonMemberFailsTest = {
        val aliceCfg = { sender: alice, nonce: 0 }
        val bobCfg   = { sender: bob,   nonce: 1 }
        val tid      = { topicID: 0 }
        val reg1 = processCreateTopic(emptyRegistry, aliceCfg, emptyTopic).post_state
        val reg2 = processAddAdmin(reg1, aliceCfg, tid, bob).post_state
        // carol is not an owner — removeOwner should fail
        val t_owner = processRemoveOwner(reg2, aliceCfg, tid, carol)
        // carol is not an admin — removeAdmin should fail
        val t_admin = processRemoveAdmin(reg2, aliceCfg, tid, carol)
        // carol is not a publisher — removePublisher should fail
        val t_pub   = processRemovePublisher(reg2, aliceCfg, tid, carol)
        all {
            assert(not(t_owner.post_state.topics.get(tid).owners.size()     == 0)),
            assert(t_owner.response == Some(RemoveOwnerResponse({ cfg: aliceCfg, success: false }))),
            assert(t_admin.response == Some(RemoveAdminResponse({ cfg: aliceCfg, success: false }))),
            assert(t_pub.response   == Some(RemovePublisherResponse({ cfg: aliceCfg, success: false }))),
        }
    }

    // publishedAtEpoch is stamped with the registry epoch at creation time.
    run publishedAtEpochStampTest = {
        val cfg  = { sender: alice, nonce: 0 }
        val reg0 = { ...emptyRegistry, epoch: 5 }
        val reg1 = processCreateTopic(reg0, cfg, emptyTopic).post_state
        val tid  = { topicID: 0 }
        assert(reg1.topics.get(tid).publishedAtEpoch == 5)
    }

    // createTopic rejects zero or negative R and T (R/T validation at creation time).
    run createTopicInvalidParamsTest = {
        val cfg = { sender: alice, nonce: 0 }
        val t_rf_zero = processCreateTopic(emptyRegistry, cfg, { ...emptyTopic, replicationFactor: 0 })
        val t_rf_neg  = processCreateTopic(emptyRegistry, cfg, { ...emptyTopic, replicationFactor: -1 })
        val t_rp_zero = processCreateTopic(emptyRegistry, cfg, { ...emptyTopic, retentionPeriod: 0 })
        val t_rp_neg  = processCreateTopic(emptyRegistry, cfg, { ...emptyTopic, retentionPeriod: -1 })
        all {
            assert(t_rf_zero.response == Some(CreateTopicResponse({ cfg: cfg, topicID: None }))),
            assert(t_rf_neg.response  == Some(CreateTopicResponse({ cfg: cfg, topicID: None }))),
            assert(t_rp_zero.response == Some(CreateTopicResponse({ cfg: cfg, topicID: None }))),
            assert(t_rp_neg.response  == Some(CreateTopicResponse({ cfg: cfg, topicID: None }))),
            // counter must not advance on failure
            assert(t_rf_zero.post_state.counter == 0),
            assert(t_rp_zero.post_state.counter == 0),
        }
    }

    // Zero and negative replication factors / retention periods are rejected (bug #5 regression).
    run invalidParamRejectionTest = {
        val cfg  = { sender: alice, nonce: 0 }
        val reg1 = processCreateTopic(emptyRegistry, cfg, emptyTopic).post_state
        val tid  = { topicID: 0 }
        val t_rf_zero = processSetReplicationFactor(reg1, cfg, tid, 0)
        val t_rf_neg  = processSetReplicationFactor(reg1, cfg, tid, -1)
        val t_rp_zero = processSetRetentionPeriod(reg1, cfg, tid, 0)
        val t_rp_neg  = processSetRetentionPeriod(reg1, cfg, tid, -5)
        val t_rf_ok   = processSetReplicationFactor(reg1, cfg, tid, 1)
        val t_rp_ok   = processSetRetentionPeriod(reg1, cfg, tid, 1)
        all {
            assert(t_rf_zero.response == Some(SetReplicationFactorResponse({ cfg: cfg, success: false }))),
            assert(t_rf_neg.response  == Some(SetReplicationFactorResponse({ cfg: cfg, success: false }))),
            assert(t_rp_zero.response == Some(SetRetentionPeriodResponse({ cfg: cfg, success: false }))),
            assert(t_rp_neg.response  == Some(SetRetentionPeriodResponse({ cfg: cfg, success: false }))),
            assert(t_rf_ok.response   == Some(SetReplicationFactorResponse({ cfg: cfg, success: true }))),
            assert(t_rp_ok.response   == Some(SetRetentionPeriodResponse({ cfg: cfg, success: true }))),
            // state unchanged on failure
            assert(t_rf_zero.post_state.topics.get(tid).replicationFactor == emptyTopic.replicationFactor),
            assert(t_rp_zero.post_state.topics.get(tid).retentionPeriod   == emptyTopic.retentionPeriod),
        }
    }

    // Deleted topic: the topicID stays in the registry, counter is not reset.
    run deletedTopicIDRetentionTest = {
        val cfg  = { sender: alice, nonce: 0 }
        val reg1 = processCreateTopic(emptyRegistry, cfg, emptyTopic).post_state
        val tid  = { topicID: 0 }
        val reg2 = processDeleteTopic(reg1, cfg, tid).post_state
        all {
            assert(reg2.topics.has(tid)),
            assert(not(reg2.topics.get(tid).alive)),
            assert(reg2.counter == 1),
        }
    }
}
