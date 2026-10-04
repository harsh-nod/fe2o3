use vstd::prelude::*;

verus! {

pub open spec fn executable_u64_range_representable(base: nat, len: nat) -> bool {
    base <= 18_446_744_073_709_551_615
        && base + len <= 18_446_744_073_709_551_615
}

pub proof fn mutated_u64_exclusive_end_wrap_is_representable()
    ensures
        executable_u64_range_representable(18_446_744_073_709_551_615, 1),
{
}

} // verus!
