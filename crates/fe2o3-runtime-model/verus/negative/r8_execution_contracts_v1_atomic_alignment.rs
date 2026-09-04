use vstd::prelude::*;

verus! {

pub open spec fn mutated_valid_atomic_location_v1(width_bits: nat, byte_offset: nat) -> bool {
    width_bits == 32 || width_bits == 64
}

pub proof fn mutated_valid_atomic_location_is_aligned_v1(
    width_bits: nat,
    byte_offset: nat,
)
    requires
        mutated_valid_atomic_location_v1(width_bits, byte_offset),
        byte_offset % (width_bits / 8) != 0,
    ensures byte_offset % (width_bits / 8) == 0,
{
}

} // verus!
