// Page alignment is insufficient without the native single-allocation bound.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_span_v1(requested: u64, cpu: u64, gpu_va: u64) -> bool {
    0 < requested <= cpu && (cpu as int) - (requested as int) < 4096
        && cpu % 4096 == 0 && gpu_va == cpu
}
pub proof fn mutated_oversized_accepted_v1(requested: u64, cpu: u64, gpu_va: u64)
    requires requested == cpu, cpu > 2147483648, cpu % 4096 == 0, gpu_va == cpu,
    ensures !mutated_span_v1(requested, cpu, gpu_va),
{}
}
