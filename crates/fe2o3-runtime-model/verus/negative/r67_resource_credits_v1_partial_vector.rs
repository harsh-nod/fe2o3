// Expected negative: the final resource dimension is omitted from admission.
use vstd::prelude::*;
verus! {
pub open spec fn partial_admission_v1(used: Seq<u64>, charge: Seq<u64>, capacity: Seq<u64>) -> bool {
    used.len() == 19 && charge.len() == 19 && capacity.len() == 19
        && (forall|i: int| 0 <= i < 18 ==> used[i] as int + charge[i] as int <= capacity[i] as int)
}
pub proof fn mutated_partial_vector_v1(used: Seq<u64>, charge: Seq<u64>, capacity: Seq<u64>)
    requires partial_admission_v1(used, charge, capacity),
    ensures forall|i: int| 0 <= i < 19 ==> used[i] as int + charge[i] as int <= capacity[i] as int,
{}
}
