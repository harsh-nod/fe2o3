// Omitting coverage lets a negative mathematical padding difference pass.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_span_v1(requested: u64, cpu: u64, gpu_va: u64) -> bool {
    requested != 0 && cpu <= 2147483648 && (cpu as int) - (requested as int) < 4096
        && cpu % 4096 == 0 && gpu_va == cpu
}
pub proof fn mutated_short_mapping_accepted_v1(requested: u64, cpu: u64, gpu_va: u64)
    requires 0 < cpu < requested <= 2147483648, cpu % 4096 == 0, gpu_va == cpu,
    ensures !mutated_span_v1(requested, cpu, gpu_va),
{}
}
