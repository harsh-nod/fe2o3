use vstd::prelude::*;
verus! {
pub open spec fn access_width_v1() -> nat { 4 }
pub proof fn mutated_access_width_is_exact_v1()
    ensures access_width_v1() == 8,
{
}
}
