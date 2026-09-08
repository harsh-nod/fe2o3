use fe2o3_device::{
    CurrentTarget, Gfx950Fp4E2M1, Global, KernelCapabilityBrand, KernelContext, ReadOnly,
    RegisteredLaunch, StrictIeee, SubgroupWidth64,
};

enum Kernel {}

type Root<'kernel> = KernelCapabilityBrand<'kernel, Kernel, CurrentTarget, RegisteredLaunch>;

fn transpose_then_read<'kernel>(
    mut context: KernelContext<'kernel, Kernel>,
    key: Global<'kernel, u8, ReadOnly, Root<'kernel>>,
) {
    let policy = context.numerical_policy::<StrictIeee>();
    context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        let wave16 = subgroup.gfx950_wave16(workgroup.epoch());
        let transpose = wave16.transpose_tile::<Gfx950Fp4E2M1>();
        let staged = subgroup.with_matrix(workgroup.epoch(), |matrix, _lane| {
            let policy_matrix = matrix.with_numerical_policy(&policy);
            let gfx950 = policy_matrix.gfx950();
            let matrix = gfx950
                .fp4_a_global_row_major(&key, 0, 16, 128, 128)
                .unwrap();
            transpose.stage_k_transposed(&matrix, 0, 0)
        });
        let (workgroup, published) = staged.publish(workgroup);
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        let _ = published.read_mfma_fragment(subgroup.lane());
    });
}

fn main() {}
