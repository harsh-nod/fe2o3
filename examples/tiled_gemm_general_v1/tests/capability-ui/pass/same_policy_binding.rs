// expected-supported: FE2O3-CAP-GEMM same-kernel-policy-borrow
#![no_std]

#[cfg(not(target_arch = "amdgpu"))]
compile_error!("GEMM device UI must compile the actual AMD target");

use fe2o3_device::{
    ExclusiveReadWrite, Global, KernelContext, StrictIeee, SubgroupWidth64, kernel,
};

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn same_policy_binding(
    mut context: KernelContext<'_>,
    mut output: Global<'_, f32, ExclusiveReadWrite>,
) {
    let policy = context.numerical_policy::<StrictIeee>();
    context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        subgroup.with_matrix(workgroup.epoch(), |matrix, lane| {
            let paired = matrix.with_numerical_policy(&policy);
            let values = paired.bf16_zero_accumulator(lane).into_values();
            let index = lane.get() as usize;
            if let Some(previous) = output.load(index) {
                if !output.store(index, previous + values[0]) {
                    fe2o3_device::trap();
                }
            }
        });
    });
}
