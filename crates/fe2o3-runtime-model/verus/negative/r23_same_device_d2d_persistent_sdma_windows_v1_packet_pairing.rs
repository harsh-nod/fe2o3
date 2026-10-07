use vstd::prelude::*;
verus! {
pub open spec fn mutated_destination_packet_bytes_v1(source_bytes: nat) -> nat {
    source_bytes + 1
}
pub proof fn mutated_source_destination_packet_lengths_match_v1()
    ensures mutated_destination_packet_bytes_v1(16) == 16, {}
}
