module types {
    
    import basicSpells.* from "spells/basicSpells"
    import rareSpells.* from "spells/rareSpells"

   type PublicKey = {pk : int}
   type TopicID = {topicID : int}

   type MsgCfg = {
    sender : PublicKey,
    nonce: int,
   }

   type Topic = {
        name : str,
        admins: Set[PublicKey],
        owners: Set[PublicKey],
        publishers: Set[PublicKey], // empty -- open topic
        replicationFactor : int,
        retentionPeriod : int,      // stored for clients; not interpreted by the registry
        alive: bool,                // soft-delete flag; topicID is retained forever to prevent ID reassignment (spec §2.1)
        publishedAtEpoch : int,     // epoch at which the topic was created
    }

    type Message = 
        | CreateTopicRequest({cfg: MsgCfg, topic : Topic})
        | CreateTopicResponse({cfg: MsgCfg, topicID : Option[TopicID]})
        
        | DeleteTopicRequest({cfg: MsgCfg, topicID : TopicID})
        | DeleteTopicResponse({cfg: MsgCfg, success : bool})

        | AddOwnerRequest({cfg: MsgCfg, topicID : TopicID, newOwner : PublicKey})
        | AddOwnerResponse({cfg: MsgCfg, success : bool})

        | RemoveOwnerRequest({cfg: MsgCfg, topicID : TopicID, oldOwner : PublicKey})
        | RemoveOwnerResponse({cfg: MsgCfg, success : bool})

        | AddAdminRequest({cfg: MsgCfg, topicID : TopicID, newAdmin : PublicKey})
        | AddAdminResponse({cfg: MsgCfg, success : bool})

        | RemoveAdminRequest({cfg: MsgCfg, topicID : TopicID, oldAdmin : PublicKey})
        | RemoveAdminResponse({cfg: MsgCfg, success : bool})

        | AddPublisherRequest({cfg: MsgCfg, topicID : TopicID, newPublisher : PublicKey})
        | AddPublisherResponse({cfg: MsgCfg, success : bool})

        | RemovePublisherRequest({cfg: MsgCfg, topicID : TopicID, oldPublisher : PublicKey})
        | RemovePublisherResponse({cfg: MsgCfg, success : bool})

        | SetReplicationFactorRequest({cfg: MsgCfg, topicID : TopicID, R : int})
        | SetReplicationFactorResponse({cfg: MsgCfg, success : bool})

        | SetRetentionPeriodRequest({cfg: MsgCfg, topicID : TopicID, T : int})
        | SetRetentionPeriodResponse({cfg: MsgCfg, success : bool})


    type TopicRegistry = {
        topics  : TopicID -> Topic,
        counter : int,
        epoch   : int,

        inbox   : List[Message],
        outbox  : List[Message],
        nonce   : int,
        log     : Set[Message],
        authLog : Set[{nonce: int, authorized: bool}],
    }

    type Transition = {
        post_state : TopicRegistry,
        response : Option[Message]
    }

    pure def get_create_topic_requests(msgs: Set[Message]): Set[{cfg: MsgCfg, topic : Topic}] =
        msgs.filterMap(m => match m { | CreateTopicRequest(p) => Some(p) | _ => None })

    pure def get_create_topic_responses(msgs: Set[Message]): Set[{cfg: MsgCfg, topicID : Option[TopicID]}] =
        msgs.filterMap(m => match m { | CreateTopicResponse(p) => Some(p) | _ => None })

    pure def get_delete_topic_requests(msgs: Set[Message]): Set[{cfg: MsgCfg, topicID : TopicID}] =
        msgs.filterMap(m => match m { | DeleteTopicRequest(p) => Some(p) | _ => None })

    pure def get_delete_topic_responses(msgs: Set[Message]): Set[{cfg: MsgCfg, success : bool}] =
        msgs.filterMap(m => match m { | DeleteTopicResponse(p) => Some(p) | _ => None })

    pure def get_add_owner_requests(msgs: Set[Message]): Set[{cfg: MsgCfg, topicID : TopicID, newOwner : PublicKey}] =
        msgs.filterMap(m => match m { | AddOwnerRequest(p) => Some(p) | _ => None })

    pure def get_add_owner_responses(msgs: Set[Message]): Set[{cfg: MsgCfg, success : bool}] =
        msgs.filterMap(m => match m { | AddOwnerResponse(p) => Some(p) | _ => None })

    pure def get_remove_owner_requests(msgs: Set[Message]): Set[{cfg: MsgCfg, topicID : TopicID, oldOwner : PublicKey}] =
        msgs.filterMap(m => match m { | RemoveOwnerRequest(p) => Some(p) | _ => None })

    pure def get_remove_owner_responses(msgs: Set[Message]): Set[{cfg: MsgCfg, success : bool}] =
        msgs.filterMap(m => match m { | RemoveOwnerResponse(p) => Some(p) | _ => None })

    pure def get_add_admin_requests(msgs: Set[Message]): Set[{cfg: MsgCfg, topicID : TopicID, newAdmin : PublicKey}] =
        msgs.filterMap(m => match m { | AddAdminRequest(p) => Some(p) | _ => None })

    pure def get_add_admin_responses(msgs: Set[Message]): Set[{cfg: MsgCfg, success : bool}] =
        msgs.filterMap(m => match m { | AddAdminResponse(p) => Some(p) | _ => None })

    pure def get_remove_admin_requests(msgs: Set[Message]): Set[{cfg: MsgCfg, topicID : TopicID, oldAdmin : PublicKey}] =
        msgs.filterMap(m => match m { | RemoveAdminRequest(p) => Some(p) | _ => None })

    pure def get_remove_admin_responses(msgs: Set[Message]): Set[{cfg: MsgCfg, success : bool}] =
        msgs.filterMap(m => match m { | RemoveAdminResponse(p) => Some(p) | _ => None })

    pure def get_add_publisher_requests(msgs: Set[Message]): Set[{cfg: MsgCfg, topicID : TopicID, newPublisher : PublicKey}] =
        msgs.filterMap(m => match m { | AddPublisherRequest(p) => Some(p) | _ => None })

    pure def get_add_publisher_responses(msgs: Set[Message]): Set[{cfg: MsgCfg, success : bool}] =
        msgs.filterMap(m => match m { | AddPublisherResponse(p) => Some(p) | _ => None })

    pure def get_remove_publisher_requests(msgs: Set[Message]): Set[{cfg: MsgCfg, topicID : TopicID, oldPublisher : PublicKey}] =
        msgs.filterMap(m => match m { | RemovePublisherRequest(p) => Some(p) | _ => None })

    pure def get_remove_publisher_responses(msgs: Set[Message]): Set[{cfg: MsgCfg, success : bool}] =
        msgs.filterMap(m => match m { | RemovePublisherResponse(p) => Some(p) | _ => None })

    pure def get_set_replication_factor_requests(msgs: Set[Message]): Set[{cfg: MsgCfg, topicID : TopicID, R : int}] =
        msgs.filterMap(m => match m { | SetReplicationFactorRequest(p) => Some(p) | _ => None })

    pure def get_set_replication_factor_responses(msgs: Set[Message]): Set[{cfg: MsgCfg, success : bool}] =
        msgs.filterMap(m => match m { | SetReplicationFactorResponse(p) => Some(p) | _ => None })

    pure def get_set_retention_period_requests(msgs: Set[Message]): Set[{cfg: MsgCfg, topicID : TopicID, T : int}] =
        msgs.filterMap(m => match m { | SetRetentionPeriodRequest(p) => Some(p) | _ => None })

    pure def get_set_retention_period_responses(msgs: Set[Message]): Set[{cfg: MsgCfg, success : bool}] =
        msgs.filterMap(m => match m { | SetRetentionPeriodResponse(p) => Some(p) | _ => None })

}