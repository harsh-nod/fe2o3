use vstd::prelude::*;
verus! {
pub open spec fn mutated_full_transfer_window_packet_signature_v1() -> nat { 164 }
pub proof fn mutated_256_mib_may_fit_one_window_and_64_packets_v1()
    ensures mutated_full_transfer_window_packet_signature_v1() == 265, {}
}
