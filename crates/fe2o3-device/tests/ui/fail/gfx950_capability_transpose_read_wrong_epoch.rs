use fe2o3_device::{
    CurrentTarget, Gfx950Fp4E2M1, Gfx950LdsTransposeTile, Gfx950TransposePublished, InitialEpoch,
    KernelCapabilityBrand, NextEpoch, RegisteredLaunch, SubgroupBrand, SubgroupLane,
    SubgroupWidth64,
};

enum Kernel {}

type Root<'kernel> = KernelCapabilityBrand<'kernel, Kernel, CurrentTarget, RegisteredLaunch>;
type PublishedBrand<'workgroup> =
    SubgroupBrand<'workgroup, SubgroupWidth64, Root<'workgroup>, NextEpoch<InitialEpoch>>;
type StaleBrand<'workgroup> =
    SubgroupBrand<'workgroup, SubgroupWidth64, Root<'workgroup>, InitialEpoch>;

fn reject<'workgroup>(
    tile: Gfx950LdsTransposeTile<
        'workgroup,
        Gfx950Fp4E2M1,
        Gfx950TransposePublished,
        PublishedBrand<'workgroup>,
    >,
    stale_lane: &SubgroupLane<SubgroupWidth64, StaleBrand<'workgroup>>,
) {
    let _ = tile.read_mfma_fragment(stale_lane);
}

fn main() {}
