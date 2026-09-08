use fe2o3_device::{
    CurrentTarget, Gfx950Fp4E2M1, Gfx950Fp4MfmaBFragment, Gfx950Fp8E4M3, Gfx950Fp8MfmaBFragment,
    Gfx950LdsTransposeTile, Gfx950TransposePublished, InitialEpoch, KernelCapabilityBrand,
    NextEpoch, RegisteredLaunch, SubgroupBrand, SubgroupLane, SubgroupWidth64,
};

enum Kernel {}

type Root<'kernel> = KernelCapabilityBrand<'kernel, Kernel, CurrentTarget, RegisteredLaunch>;
type PublishedBrand<'workgroup> =
    SubgroupBrand<'workgroup, SubgroupWidth64, Root<'workgroup>, NextEpoch<InitialEpoch>>;

fn read_fp4<'operation, 'workgroup>(
    tile: Gfx950LdsTransposeTile<
        'workgroup,
        Gfx950Fp4E2M1,
        Gfx950TransposePublished,
        PublishedBrand<'workgroup>,
    >,
    lane: &'operation SubgroupLane<SubgroupWidth64, PublishedBrand<'workgroup>>,
) -> Gfx950Fp4MfmaBFragment<'operation, PublishedBrand<'workgroup>> {
    tile.read_mfma_fragment(lane)
}

fn read_fp8<'operation, 'workgroup>(
    tile: Gfx950LdsTransposeTile<
        'workgroup,
        Gfx950Fp8E4M3,
        Gfx950TransposePublished,
        PublishedBrand<'workgroup>,
    >,
    lane: &'operation SubgroupLane<SubgroupWidth64, PublishedBrand<'workgroup>>,
) -> Gfx950Fp8MfmaBFragment<'operation, PublishedBrand<'workgroup>> {
    tile.read_mfma_fragment(lane)
}

fn main() {}
