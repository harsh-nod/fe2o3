use fe2o3_device::{MatrixCapability, SubgroupLane, SubgroupWidth32};

fn target_width_mismatch<Brand>(
    matrix: &MatrixCapability<Brand>,
    lane: &SubgroupLane<SubgroupWidth32, Brand>,
) {
    let _ = matrix.bf16_zero_accumulator(lane);
}
