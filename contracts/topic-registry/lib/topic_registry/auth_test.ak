use topic_registry/auth
use topic_registry/test_helpers.{alice, bob, carol, dave, sample_topic_datum}
use topic_registry/types.{TopicDatum}

test signed_by_owner_succeeds() {
  let datum = sample_topic_datum()
  auth.signed_by_owner(datum, [alice])
}

test signed_by_owner_fails_wrong_signer() {
  let datum = sample_topic_datum()
  !auth.signed_by_owner(datum, [bob])
}

test signed_by_owner_fails_dead_topic() {
  let datum = TopicDatum { ..sample_topic_datum(), alive: False }
  !auth.signed_by_owner(datum, [alice])
}

test signed_by_owner_with_multiple_signers() {
  let datum = TopicDatum { ..sample_topic_datum(), owners: [alice, carol] }
  auth.signed_by_owner(datum, [bob, carol])
}

test signed_by_admin_succeeds() {
  let datum = TopicDatum { ..sample_topic_datum(), admins: [bob] }
  auth.signed_by_admin(datum, [bob])
}

test signed_by_admin_fails_non_admin() {
  let datum = TopicDatum { ..sample_topic_datum(), admins: [bob] }
  !auth.signed_by_admin(datum, [carol])
}

test signed_by_owner_or_admin_for_owner() {
  let datum = TopicDatum { ..sample_topic_datum(), admins: [bob] }
  auth.signed_by_owner_or_admin(datum, [alice])
}

test signed_by_owner_or_admin_for_admin() {
  let datum = TopicDatum { ..sample_topic_datum(), admins: [bob] }
  auth.signed_by_owner_or_admin(datum, [bob])
}

test signed_by_owner_or_admin_fails_outsider() {
  let datum = TopicDatum { ..sample_topic_datum(), admins: [bob] }
  !auth.signed_by_owner_or_admin(datum, [dave])
}
