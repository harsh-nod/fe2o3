use vstd::prelude::*;
verus! {
pub open spec fn mutated_generic_materialization_count_v1() -> nat { 1 }
pub proof fn mutated_generic_materialization_count_is_zero_v1()
    ensures mutated_generic_materialization_count_v1() == 0, {}
}
