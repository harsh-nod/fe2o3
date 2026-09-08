use fe2o3_device::{
    Gfx950Fp8E4M3, Global, KernelContext, ReadOnly, StrictIeee, SubgroupWidth64, kernel,
};

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn typed_fp8_transpose(
    mut context: KernelContext<'_>,
    query: Global<'_, u8, ReadOnly>,
    key: Global<'_, u8, ReadOnly>,
) {
    let policy = context.numerical_policy::<StrictIeee>();
    let _values = context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        let key = subgroup.with_matrix(workgroup.epoch(), |matrix, _lane| {
            let policy_matrix = matrix.with_numerical_policy(&policy);
            let matrix = policy_matrix.gfx950();
            matrix
                .fp8_a_global_row_major(&key, 0, 16, 128, 128)
                .unwrap()
        });
        let staged = subgroup
            .gfx950_wave16(workgroup.epoch())
            .transpose_tile::<Gfx950Fp8E4M3>()
            .stage_k_transposed(&key, 0, 0);
        let (workgroup, key) = staged.publish(workgroup);
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        subgroup.with_matrix(workgroup.epoch(), |matrix, lane| {
            let policy_matrix = matrix.with_numerical_policy(&policy);
            let matrix = policy_matrix.gfx950();
            let query = matrix
                .fp8_a_global_row_major(&query, 0, 16, 128, 128)
                .unwrap()
                .load_m16k128(lane, 0, 0);
            let key = key.read_mfma_fragment(lane);
            let accumulator = matrix.fp8_zero_accumulator(lane);
            matrix
                .multiply_accumulate_fp8(query, key, accumulator)
                .into_values()
        })
    });
}
