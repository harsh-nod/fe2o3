use fe2o3_device::{MatrixCapability, SubgroupLane, SubgroupWidth64};

fn wrong_layout<Brand>(
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
    let _ = matrix.multiply_accumulate(rhs, lhs, accumulator);
}
