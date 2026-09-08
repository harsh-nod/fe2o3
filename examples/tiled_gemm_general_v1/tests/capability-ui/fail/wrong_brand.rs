use fe2o3_device::{MatrixCapability, SubgroupLane, SubgroupWidth64};

fn wrong_brand<Left, Right>(
    matrix: &MatrixCapability<Left>,
    lane: &SubgroupLane<SubgroupWidth64, Right>,
) {
    let _ = matrix.bf16_zero_accumulator(lane);
}
