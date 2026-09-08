use fe2o3_device::{
    CurrentTarget, Gfx950Fp8E4M3, Gfx950LdsTransposeTile, Gfx950TransposePublished, InitialEpoch,
    KernelCapabilityBrand, NextEpoch, RegisteredLaunch, SubgroupBrand, SubgroupLane,
    SubgroupWidth64,
};

enum KernelA {}
enum KernelB {}

type RootA<'kernel> = KernelCapabilityBrand<'kernel, KernelA, CurrentTarget, RegisteredLaunch>;
type RootB<'kernel> = KernelCapabilityBrand<'kernel, KernelB, CurrentTarget, RegisteredLaunch>;
type PublishedBrandA<'workgroup> =
    SubgroupBrand<'workgroup, SubgroupWidth64, RootA<'workgroup>, NextEpoch<InitialEpoch>>;
type PublishedBrandB<'workgroup> =
    SubgroupBrand<'workgroup, SubgroupWidth64, RootB<'workgroup>, NextEpoch<InitialEpoch>>;

fn reject<'workgroup>(
    tile: Gfx950LdsTransposeTile<
        'workgroup,
        Gfx950Fp8E4M3,
        Gfx950TransposePublished,
        PublishedBrandA<'workgroup>,
    >,
    other_kernel_lane: &SubgroupLane<SubgroupWidth64, PublishedBrandB<'workgroup>>,
) {
    let _ = tile.read_mfma_fragment(other_kernel_lane);
}

fn main() {}
