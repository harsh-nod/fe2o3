use fe2o3_device::{
    Global, KernelContext, MatrixCapability, ReadOnly, SubgroupLane, SubgroupWidth64,
};

fn checked_matrix_view<Brand>(
    input: &Global<'_, u16, ReadOnly, Brand>,
    matrix: &MatrixCapability<Brand>,
    lane: &SubgroupLane<SubgroupWidth64, Brand>,
) {
    let bits = [0_u16; 256];
    let lhs = matrix
        .bf16_a_row_major(&bits, 0, 16, 16, 16)
        .unwrap()
        .load_m16k16(lane, 0, 0);
    let rhs = matrix
        .bf16_b_row_major(&bits, 0, 16, 16, 16)
        .unwrap()
        .load_k16n16(lane, 0, 0);
    let accumulator = matrix.bf16_zero_accumulator(lane);
    let _ = matrix.multiply_accumulate(lhs, rhs, accumulator);
    let _ = input.len();
}

fn hierarchy<'kernel, Kernel>(mut context: KernelContext<'kernel, Kernel>) {
    let _private = context.private_memory::<f32, 4>();
    context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        subgroup.with_matrix(workgroup.epoch(), |_matrix, lane| {
            assert!(lane.get() < 64);
        });
    });
}
