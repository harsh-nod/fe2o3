use vstd::prelude::*;
verus! {
pub open spec fn global_address_space_v1() -> nat { 1 }
pub proof fn mutated_address_space_is_exact_v1()
    ensures global_address_space_v1() == 3,
{
}
}
