use vstd::prelude::*;

verus! {

pub open spec fn workgroup_pointer_value_representable(address: nat) -> bool {
    address <= 4_294_967_295
}

pub proof fn mutated_one_past_workgroup_pointer_is_representable()
    ensures
        workgroup_pointer_value_representable(4_294_967_296),
{
}

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
