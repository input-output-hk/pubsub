use topic_registry/test_helpers.{alice, bob, sample_topic_datum}
use topic_registry/types.{TopicDatum}
use topic_registry/utils

test tombstone_has_correct_topic_id() {
  let ts = utils.tombstone(42)
  ts.topic_id == 42
}

test tombstone_is_not_alive() {
  !utils.tombstone(0).alive
}

test tombstone_has_empty_fields() {
  let ts = utils.tombstone(0)
  ts.name == "" && ts.owners == [] && ts.admins == [] && ts.replication_factor == 0 && ts.retention_period == 0 && ts.published_at_epoch == 0
}

test is_valid_tombstone_accepts_correct() {
  utils.is_valid_tombstone(utils.tombstone(5))
}

test is_valid_tombstone_rejects_alive() {
  let bad = TopicDatum { ..utils.tombstone(0), alive: True }
  !utils.is_valid_tombstone(bad)
}

test is_valid_tombstone_rejects_nonempty_name() {
  let bad = TopicDatum { ..utils.tombstone(0), name: "still here" }
  !utils.is_valid_tombstone(bad)
}

test is_valid_tombstone_rejects_leftover_owners() {
  let bad = TopicDatum { ..utils.tombstone(0), owners: [alice] }
  !utils.is_valid_tombstone(bad)
}

test only_owners_changed_accepts_valid() {
  let input = sample_topic_datum()
  let output = TopicDatum { ..input, owners: [alice, bob] }
  utils.only_owners_changed(input, output)
}

test only_owners_changed_rejects_name_change() {
  let input = sample_topic_datum()
  let output = TopicDatum { ..input, owners: [alice, bob], name: "changed" }
  !utils.only_owners_changed(input, output)
}

test only_admins_changed_accepts_valid() {
  let input = sample_topic_datum()
  let output = TopicDatum { ..input, admins: [bob] }
  utils.only_admins_changed(input, output)
}

test only_replication_factor_changed_accepts_valid() {
  let input = sample_topic_datum()
  let output = TopicDatum { ..input, replication_factor: 5 }
  utils.only_replication_factor_changed(input, output)
}

test only_retention_period_changed_accepts_valid() {
  let input = sample_topic_datum()
  let output = TopicDatum { ..input, retention_period: 200 }
  utils.only_retention_period_changed(input, output)
}
