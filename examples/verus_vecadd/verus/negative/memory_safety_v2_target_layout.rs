use vstd::prelude::*;

verus! {

pub struct TargetLayout {
    pub workgroup_pointer_bits: nat,
    pub workgroup_pointer_alignment: nat,
}

pub open spec fn gfx942_workgroup_layout(target: TargetLayout) -> bool {
    target.workgroup_pointer_bits == 32 && target.workgroup_pointer_alignment == 32
}

pub proof fn mutated_64_bit_workgroup_layout_is_gfx942()
    ensures
        gfx942_workgroup_layout(TargetLayout {
            workgroup_pointer_bits: 64,
            workgroup_pointer_alignment: 32,
        }),
{
}

pub open spec fn address_space_writable(address_space: nat) -> bool {
    (address_space == 0 || address_space == 1 || address_space == 3
        || address_space == 4 || address_space == 5)
        && address_space != 4
}

pub proof fn mutated_constant_memory_is_writable()
    ensures
        address_space_writable(4),
{
}

} // verus!
