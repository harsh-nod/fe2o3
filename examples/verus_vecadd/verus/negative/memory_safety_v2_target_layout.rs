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

} // verus!
