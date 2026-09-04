use vstd::prelude::*;
verus! {
pub open spec fn mutated_256_mib_packet_count_v1() -> nat { 64 }
pub proof fn mutated_256_mib_requires_65_packets_v1()
    ensures mutated_256_mib_packet_count_v1() == 65, {}
}
