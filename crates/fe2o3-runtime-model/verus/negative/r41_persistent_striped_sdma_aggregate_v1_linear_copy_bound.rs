// Expected-negative R41 mutation: one request exceeds the gfx942 linear-copy packet maximum.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_copy_bytes_v1() -> nat { 0x003f_ffe1 }
pub proof fn mutated_linear_copy_is_single_packet_bounded_v1()
    ensures mutated_copy_bytes_v1() <= 0x003f_ffe0,
{}
}
