use aiken/collection/list
use topic_registry/types.{TopicDatum}

pub fn signed_by_owner(datum: TopicDatum, signatories: List<ByteArray>) -> Bool {
  datum.alive && list.any(signatories, fn(sig) { list.has(datum.owners, sig) })
}

pub fn signed_by_admin(datum: TopicDatum, signatories: List<ByteArray>) -> Bool {
  datum.alive && list.any(signatories, fn(sig) { list.has(datum.admins, sig) })
}

pub fn signed_by_owner_or_admin(
  datum: TopicDatum,
  signatories: List<ByteArray>,
) -> Bool {
  signed_by_owner(datum, signatories) || signed_by_admin(datum, signatories)
}
