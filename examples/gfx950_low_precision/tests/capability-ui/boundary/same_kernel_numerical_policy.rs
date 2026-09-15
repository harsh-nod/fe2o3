#![forbid(unsafe_code)]

use fe2o3_device::{
    CurrentTarget, KernelCapabilityBrand, KernelContext, RegisteredLaunch, StrictIeee,
    SubgroupWidth64,
};

type Root<'kernel, Kernel> =
    KernelCapabilityBrand<'kernel, Kernel, CurrentTarget, RegisteredLaunch>;

fn same_root_policy<'kernel, Kernel>(context: &KernelContext<'kernel, Kernel>) {
    let matrix = context.matrix();
    let policy = context.numerical_policy::<StrictIeee>();
    let _ = matrix.with_numerical_policy::<Root<'kernel, Kernel>, StrictIeee>(&policy);
}

fn derived_matrix_same_root_policy<'kernel, Kernel>(mut context: KernelContext<'kernel, Kernel>) {
    let policy = context.numerical_policy::<StrictIeee>();
    context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        subgroup.with_matrix(workgroup.epoch(), |matrix, _lane| {
            let _ = matrix.with_numerical_policy::<Root<'kernel, Kernel>, StrictIeee>(&policy);
        });
    });
}
