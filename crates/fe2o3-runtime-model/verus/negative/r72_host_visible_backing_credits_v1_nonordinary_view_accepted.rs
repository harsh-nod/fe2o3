// A different GPU extent cannot be inferred to share the ordinary backing cost.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_span_v1(requested: u64, cpu: u64, gpu_va: u64) -> bool {
    0 < requested <= cpu <= 2147483648 && (cpu as int) - (requested as int) < 4096
        && cpu % 4096 == 0
}
pub proof fn mutated_nonordinary_view_accepted_v1(requested: u64, cpu: u64, gpu_va: u64)
    requires 0 < requested <= cpu <= 2147483648,
        (cpu as int) - (requested as int) < 4096, cpu % 4096 == 0,
        gpu_va != 0, gpu_va != cpu,
    ensures !mutated_span_v1(requested, cpu, gpu_va),
{}
}
