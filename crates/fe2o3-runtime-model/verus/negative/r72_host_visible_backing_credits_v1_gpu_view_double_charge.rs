// CPU/GPU views of the same ordinary object are not two backing allocations.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_backing_bytes_v1(cpu: u64, gpu_va: u64) -> int {
    (cpu as int) + (gpu_va as int)
}
pub proof fn mutated_gpu_view_double_charge_v1(cpu: u64, gpu_va: u64)
    requires 0 < cpu <= 2147483648, cpu % 4096 == 0, gpu_va == cpu,
    ensures mutated_backing_bytes_v1(cpu, gpu_va) == (cpu as int),
{}
}
