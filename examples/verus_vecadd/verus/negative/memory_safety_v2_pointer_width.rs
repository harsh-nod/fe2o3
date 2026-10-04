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

} // verus!
