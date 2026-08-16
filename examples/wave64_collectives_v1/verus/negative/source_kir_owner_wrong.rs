use vstd::prelude::*;

verus! {

pub open spec fn mutated_kernel_ir_owner_v1(_lane: nat) -> nat { 0 }

pub proof fn mutated_kernel_ir_ownership_is_injective_v1()
    ensures mutated_kernel_ir_owner_v1(0) != mutated_kernel_ir_owner_v1(63),
{
}

}
