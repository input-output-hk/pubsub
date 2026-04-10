use aiken/primitive/bytearray
use topic_registry/test_helpers.{alice}
use topic_registry/types

test topic_token_name_starts_with_t() {
  let name = types.topic_token_name(0)
  bytearray.at(name, 0) == 0x74
}

test topic_token_name_has_correct_length() {
  let name = types.topic_token_name(42)
  bytearray.length(name) == 5
}

test topic_token_name_encodes_differently() {
  types.topic_token_name(0) != types.topic_token_name(1)
}

test publisher_token_name_starts_with_p() {
  let name = types.publisher_token_name(0, alice)
  bytearray.at(name, 0) == 0x70
}

test publisher_token_name_has_correct_length() {
  let name = types.publisher_token_name(0, alice)
  bytearray.length(name) == 33
}

test publisher_token_names_differ_by_topic() {
  types.publisher_token_name(0, alice) != types.publisher_token_name(1, alice)
}

test publisher_token_names_differ_by_publisher() {
  let bob = #"bb000000000000000000000000000000000000000000000000000002"
  types.publisher_token_name(0, alice) != types.publisher_token_name(0, bob)
}

test registry_head_token_name_is_correct() {
  types.registry_head_token_name == "registry_head"
}
