#![no_std]
use fe2o3_device::{KernelContext, StrictIeee, SubgroupWidth64, kernel};

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]))]
pub fn policy_reusable_matrix_constructors(mut context: KernelContext<'_>, enabled: u32) {
    if enabled == 0 {
        return;
    }
    let policy = context.numerical_policy::<StrictIeee>();
    context.with_workgroup(|workgroup| {
        let mut phases = workgroup.into_reusable();
        phases.with_phase(|phase| {
            {
                let subgroup = phase.subgroup::<SubgroupWidth64>();
                subgroup.with_matrix(phase.epoch(), |matrix, _lane| {
                    let bound = matrix.with_numerical_policy(&policy);
                    let _narrowed = bound.gfx950();
                });
            }
            (phase.finish_reusable_phase(), ())
        });
    });
}
