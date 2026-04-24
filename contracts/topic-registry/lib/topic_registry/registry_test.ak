// Integration tests for the registry head (CreateTopic / TickEpoch) operations.
// Mirrors topic_registry_test.qnt from the formal Quint spec.

use aiken/collection/list
use topic_registry/test_helpers.{alice, bob}
use topic_registry/types.{RegistryHeadDatum, TopicDatum}
use topic_registry/utils
use topic_registry/validation

// ---------------------------------------------------------------------------
// Pure helper: re-implements the CreateTopic validation from validators/registry.ak
// but accepts an explicit policy_id so it can be called from unit tests.
// ---------------------------------------------------------------------------

fn valid_create_topic(
  head_in: RegistryHeadDatum,
  name: ByteArray,
  replication_factor: Int,
  retention_period: Int,
  signatories: List<ByteArray>,
  head_out: RegistryHeadDatum,
  topic_out: TopicDatum,
) -> Bool {
  // R > 0 and T > 0
  let valid_params = replication_factor > 0 && retention_period > 0
  // counter increments by 1, epoch unchanged
  let counter_valid =
    head_out.counter == head_in.counter + 1 && head_out.epoch == head_in.epoch
  // new topic picks up the old counter as its ID
  let topic_id_valid = topic_out.topic_id == head_in.counter
  // name and parameters match the request
  let fields_valid =
    topic_out.name == name && topic_out.replication_factor == replication_factor && topic_out.retention_period == retention_period
  // topic is alive at creation time
  let alive_valid = topic_out.alive == True
  // published_at_epoch is stamped from the head's epoch at creation time
  let epoch_valid = topic_out.published_at_epoch == head_in.epoch
  // exactly one owner, and that owner is one of the tx signatories
  let one_owner = list.length(topic_out.owners) == 1
  let owner_is_signer =
    list.any(signatories, fn(sig) { list.has(topic_out.owners, sig) })
  // no pre-set admins
  let no_preset_admins = topic_out.admins == []
  valid_params && counter_valid && topic_id_valid && fields_valid && alive_valid && epoch_valid && one_owner && owner_is_signer && no_preset_admins
}

// ---------------------------------------------------------------------------
// createTopicTest — basic creation, sender becomes sole owner, counter advances
// ---------------------------------------------------------------------------

test create_topic_basic() {
  let head = RegistryHeadDatum { counter: 0, epoch: 0 }
  let head_out = RegistryHeadDatum { counter: 1, epoch: 0 }
  let topic_out =
    TopicDatum {
      topic_id: 0,
      name: "test",
      owners: [alice],
      admins: [],
      replication_factor: 3,
      retention_period: 10,
      alive: True,
      published_at_epoch: 0,
    }
  valid_create_topic(head, "test", 3, 10, [alice], head_out, topic_out)
}

// ---------------------------------------------------------------------------
// createTopicIgnoresPresetOwnersTest — contract must enforce exactly one owner
// from signatories; injecting two owners must fail
// ---------------------------------------------------------------------------

test create_topic_rejects_two_owners() {
  let head = RegistryHeadDatum { counter: 0, epoch: 0 }
  let head_out = RegistryHeadDatum { counter: 1, epoch: 0 }
  let topic_two_owners =
    TopicDatum {
      topic_id: 0,
      name: "test",
      owners: [alice, bob],
      admins: [],
      replication_factor: 3,
      retention_period: 10,
      alive: True,
      published_at_epoch: 0,
    }
  !valid_create_topic(head, "test", 3, 10, [alice, bob], head_out, topic_two_owners)
}

// ---------------------------------------------------------------------------
// publishedAtEpochStampTest — published_at_epoch must equal registry epoch
// ---------------------------------------------------------------------------

test create_topic_stamps_epoch() {
  let head = RegistryHeadDatum { counter: 0, epoch: 5 }
  let head_out = RegistryHeadDatum { counter: 1, epoch: 5 }
  let topic_out =
    TopicDatum {
      topic_id: 0,
      name: "test",
      owners: [alice],
      admins: [],
      replication_factor: 3,
      retention_period: 10,
      alive: True,
      published_at_epoch: 5,
    }
  valid_create_topic(head, "test", 3, 10, [alice], head_out, topic_out)
}

test create_topic_wrong_epoch_fails() {
  let head = RegistryHeadDatum { counter: 0, epoch: 5 }
  let head_out = RegistryHeadDatum { counter: 1, epoch: 5 }
  let topic_wrong_epoch =
    TopicDatum {
      topic_id: 0,
      name: "test",
      owners: [alice],
      admins: [],
      replication_factor: 3,
      retention_period: 10,
      alive: True,
      published_at_epoch: 0,
    }
  !valid_create_topic(head, "test", 3, 10, [alice], head_out, topic_wrong_epoch)
}

// ---------------------------------------------------------------------------
// createTopicInvalidParamsTest — R = 0 or T = 0 must be rejected
// ---------------------------------------------------------------------------

test create_topic_zero_r_fails() {
  let head = RegistryHeadDatum { counter: 0, epoch: 0 }
  let head_out = RegistryHeadDatum { counter: 1, epoch: 0 }
  let topic_out =
    TopicDatum {
      topic_id: 0,
      name: "test",
      owners: [alice],
      admins: [],
      replication_factor: 0,
      retention_period: 10,
      alive: True,
      published_at_epoch: 0,
    }
  !valid_create_topic(head, "test", 0, 10, [alice], head_out, topic_out)
}

test create_topic_zero_t_fails() {
  let head = RegistryHeadDatum { counter: 0, epoch: 0 }
  let head_out = RegistryHeadDatum { counter: 1, epoch: 0 }
  let topic_out =
    TopicDatum {
      topic_id: 0,
      name: "test",
      owners: [alice],
      admins: [],
      replication_factor: 3,
      retention_period: 0,
      alive: True,
      published_at_epoch: 0,
    }
  !valid_create_topic(head, "test", 3, 0, [alice], head_out, topic_out)
}

// ---------------------------------------------------------------------------
// Counter ID assignment — topic_id == old counter (never reused)
// ---------------------------------------------------------------------------

test create_topic_assigns_counter_as_id() {
  let head = RegistryHeadDatum { counter: 7, epoch: 0 }
  let head_out = RegistryHeadDatum { counter: 8, epoch: 0 }
  let topic_out =
    TopicDatum {
      topic_id: 7,
      name: "test",
      owners: [alice],
      admins: [],
      replication_factor: 3,
      retention_period: 10,
      alive: True,
      published_at_epoch: 0,
    }
  valid_create_topic(head, "test", 3, 10, [alice], head_out, topic_out)
}

test create_topic_wrong_id_fails() {
  let head = RegistryHeadDatum { counter: 7, epoch: 0 }
  let head_out = RegistryHeadDatum { counter: 8, epoch: 0 }
  let topic_wrong_id =
    TopicDatum {
      topic_id: 0,
      name: "test",
      owners: [alice],
      admins: [],
      replication_factor: 3,
      retention_period: 10,
      alive: True,
      published_at_epoch: 0,
    }
  !valid_create_topic(head, "test", 3, 10, [alice], head_out, topic_wrong_id)
}

// ---------------------------------------------------------------------------
// No pre-set admins at creation time
// ---------------------------------------------------------------------------

test create_topic_rejects_preset_admins() {
  let head = RegistryHeadDatum { counter: 0, epoch: 0 }
  let head_out = RegistryHeadDatum { counter: 1, epoch: 0 }
  let topic_with_admin =
    TopicDatum {
      topic_id: 0,
      name: "test",
      owners: [alice],
      admins: [bob],
      replication_factor: 3,
      retention_period: 10,
      alive: True,
      published_at_epoch: 0,
    }
  !valid_create_topic(head, "test", 3, 10, [alice], head_out, topic_with_admin)
}

// ---------------------------------------------------------------------------
// TickEpoch — epoch increments, counter stays the same
// ---------------------------------------------------------------------------

fn valid_tick_epoch(head_in: RegistryHeadDatum, head_out: RegistryHeadDatum) -> Bool {
  head_out.epoch == head_in.epoch + 1 && head_out.counter == head_in.counter
}

test tick_epoch_increments_epoch() {
  let head_in = RegistryHeadDatum { counter: 5, epoch: 3 }
  let head_out = RegistryHeadDatum { counter: 5, epoch: 4 }
  valid_tick_epoch(head_in, head_out)
}

test tick_epoch_preserves_counter() {
  let head_in = RegistryHeadDatum { counter: 42, epoch: 0 }
  let head_out = RegistryHeadDatum { counter: 42, epoch: 1 }
  valid_tick_epoch(head_in, head_out)
}

test tick_epoch_wrong_counter_fails() {
  let head_in = RegistryHeadDatum { counter: 5, epoch: 3 }
  let head_out = RegistryHeadDatum { counter: 6, epoch: 4 }
  !valid_tick_epoch(head_in, head_out)
}

// ---------------------------------------------------------------------------
// Tombstone invariant — deleted topic keeps its ID, is fully zeroed
// ---------------------------------------------------------------------------

test tombstone_retains_topic_id() {
  let ts = utils.tombstone(42)
  ts.topic_id == 42
}

test tombstone_is_clean() {
  let ts = utils.tombstone(0)
  utils.is_valid_tombstone(ts)
}

test tombstone_set_remove_from_owners() {
  let owners = [alice, bob]
  validation.set_remove(owners, bob) == Some([alice])
}
