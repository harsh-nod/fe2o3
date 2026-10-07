// Omitting positive requested size admits an empty ordinary span.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_span_v1(requested: u64, cpu: u64, gpu_va: u64) -> bool {
    requested <= cpu <= 2147483648 && (cpu as int) - (requested as int) < 4096
        && cpu % 4096 == 0 && gpu_va == cpu
}
pub proof fn mutated_zero_accepted_v1(requested: u64, cpu: u64, gpu_va: u64)
    requires requested == 0, cpu == 0, gpu_va == 0,
    ensures !mutated_span_v1(requested, cpu, gpu_va),
{}
}
