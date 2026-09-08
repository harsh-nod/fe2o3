use fe2o3_device::{
    CurrentTarget, Gfx950Fp4E2M1, Gfx950Fp4MfmaBFragment, Gfx950LdsTransposeTile,
    Gfx950TransposePublished, InitialEpoch, KernelCapabilityBrand, NextEpoch, RegisteredLaunch,
    SubgroupBrand, SubgroupLane, SubgroupWidth64,
};

enum Kernel {}

type Root<'kernel> = KernelCapabilityBrand<'kernel, Kernel, CurrentTarget, RegisteredLaunch>;
type PublishedBrand<'workgroup> =
    SubgroupBrand<'workgroup, SubgroupWidth64, Root<'workgroup>, NextEpoch<InitialEpoch>>;

fn reject<'workgroup>(
    tile: Gfx950LdsTransposeTile<
        'workgroup,
        Gfx950Fp4E2M1,
        Gfx950TransposePublished,
        PublishedBrand<'workgroup>,
    >,
    lane: SubgroupLane<SubgroupWidth64, PublishedBrand<'workgroup>>,
) -> Gfx950Fp4MfmaBFragment<'workgroup, PublishedBrand<'workgroup>> {
    tile.read_mfma_fragment(&lane)
}

fn main() {}
