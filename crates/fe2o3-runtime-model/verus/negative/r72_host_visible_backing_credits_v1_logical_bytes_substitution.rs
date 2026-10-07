// Charging the requested logical bytes loses the retained padding.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_charge_v1(requested: u64, cpu: u64) -> Seq<u64> {
    Seq::new(19, |i: int| if i == 2 { requested } else if i == 18 { 1u64 } else { 0u64 })
}
pub proof fn mutated_logical_bytes_substitution_v1(requested: u64, cpu: u64)
    requires 0 < requested < cpu <= 2147483648, (cpu as int) - (requested as int) < 4096,
        cpu % 4096 == 0,
    ensures mutated_charge_v1(requested, cpu)[2] == cpu,
{}
}
