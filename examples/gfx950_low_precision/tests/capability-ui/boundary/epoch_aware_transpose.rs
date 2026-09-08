#![forbid(unsafe_code)]

use fe2o3_device::{
    Gfx950Fp4E2M1, Gfx950LdsTransposeTile, Gfx950TransposeStaged, InitialEpoch, SubgroupBrand,
    SubgroupWidth64, WorkgroupCapability,
};

fn publish_with_epoch<'workgroup, Brand>(
    workgroup: WorkgroupCapability<'workgroup, Brand, InitialEpoch>,
    tile: Gfx950LdsTransposeTile<
        'workgroup,
        Gfx950Fp4E2M1,
        Gfx950TransposeStaged,
        SubgroupBrand<'workgroup, SubgroupWidth64, Brand, InitialEpoch>,
    >,
) {
    let _ = tile.publish(workgroup);
}
