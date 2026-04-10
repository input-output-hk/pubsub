use topic_registry/types.{TopicDatum}

pub fn tombstone(topic_id: Int) -> TopicDatum {
  TopicDatum {
    topic_id,
    name: "",
    owners: [],
    admins: [],
    replication_factor: 0,
    retention_period: 0,
    alive: False,
    published_at_epoch: 0,
  }
}

pub fn is_valid_tombstone(datum: TopicDatum) -> Bool {
  datum.name == "" && datum.owners == [] && datum.admins == [] && datum.replication_factor == 0 && datum.retention_period == 0 && !datum.alive && datum.published_at_epoch == 0
}

pub fn only_owners_changed(input: TopicDatum, output: TopicDatum) -> Bool {
  output.topic_id == input.topic_id && output.name == input.name && output.admins == input.admins && output.replication_factor == input.replication_factor && output.retention_period == input.retention_period && output.alive == input.alive && output.published_at_epoch == input.published_at_epoch
}

pub fn only_admins_changed(input: TopicDatum, output: TopicDatum) -> Bool {
  output.topic_id == input.topic_id && output.name == input.name && output.owners == input.owners && output.replication_factor == input.replication_factor && output.retention_period == input.retention_period && output.alive == input.alive && output.published_at_epoch == input.published_at_epoch
}

pub fn only_replication_factor_changed(
  input: TopicDatum,
  output: TopicDatum,
) -> Bool {
  output.topic_id == input.topic_id && output.name == input.name && output.owners == input.owners && output.admins == input.admins && output.retention_period == input.retention_period && output.alive == input.alive && output.published_at_epoch == input.published_at_epoch
}

pub fn only_retention_period_changed(
  input: TopicDatum,
  output: TopicDatum,
) -> Bool {
  output.topic_id == input.topic_id && output.name == input.name && output.owners == input.owners && output.admins == input.admins && output.replication_factor == input.replication_factor && output.alive == input.alive && output.published_at_epoch == input.published_at_epoch
}
