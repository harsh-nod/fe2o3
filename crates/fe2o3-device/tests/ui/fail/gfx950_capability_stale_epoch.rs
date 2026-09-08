use fe2o3_device::{
    CurrentTarget, Gfx950Fp4E2M1, Gfx950LdsTransposeTile, Gfx950TransposeStaged, InitialEpoch,
    KernelCapabilityBrand, NextEpoch, RegisteredLaunch, SubgroupBrand, SubgroupWidth64,
    WorkgroupCapability,
};

enum KernelA {}
type Root<'a> = KernelCapabilityBrand<'a, KernelA, CurrentTarget, RegisteredLaunch>;

fn reject<'a>(
    tile: Gfx950LdsTransposeTile<
        'a,
        Gfx950Fp4E2M1,
        Gfx950TransposeStaged,
        SubgroupBrand<'a, SubgroupWidth64, Root<'a>, InitialEpoch>,
    >,
    next: WorkgroupCapability<'a, Root<'a>, NextEpoch<InitialEpoch>>,
) {
    let _ = tile.publish(next);
}

fn main() {}
