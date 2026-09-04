use vstd::prelude::*;
verus! {
pub open spec fn mutated_covered_bytes_v1(packet_bytes: nat, packets: nat) -> nat {
    (packet_bytes * packets - 1) as nat
}
pub proof fn mutated_source_and_destination_packets_cover_exact_window_v1()
    ensures mutated_covered_bytes_v1(4, 2) == 8, {}
}
