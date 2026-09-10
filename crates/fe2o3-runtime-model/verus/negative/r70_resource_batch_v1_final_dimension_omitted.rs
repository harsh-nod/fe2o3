// Checking only eighteen coordinates silently omits allocation-record cost.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_vector_admission_v1(used: Seq<u64>, charge: Seq<u64>, capacity: Seq<u64>) -> bool {
    used.len() == 19 && charge.len() == 19 && capacity.len() == 19
        && (forall|d: int| 0 <= d < 18 ==>
            (used[d] as int) + (charge[d] as int) <= (capacity[d] as int))
}
pub proof fn mutated_final_dimension_omitted_v1(used: Seq<u64>, charge: Seq<u64>, capacity: Seq<u64>)
    requires
        used.len() == 19, charge.len() == 19, capacity.len() == 19,
        forall|d: int| 0 <= d < 18 ==> (used[d] as int) + (charge[d] as int) <= (capacity[d] as int),
        (used[18] as int) + (charge[18] as int) > (capacity[18] as int),
    ensures !mutated_vector_admission_v1(used, charge, capacity),
{}
}
