use fe2o3_device::{
    CurrentTarget, Gfx950Fp4E2M1, Gfx950LdsTransposeTile, Gfx950TransposeStaged, InitialEpoch,
    KernelCapabilityBrand, NextEpoch, RegisteredLaunch, SubgroupBrand, SubgroupWidth64,
    WorkgroupCapability,
};

enum Kernel {}

type Root<'kernel> = KernelCapabilityBrand<'kernel, Kernel, CurrentTarget, RegisteredLaunch>;
type Brand<'workgroup, 'kernel> =
    SubgroupBrand<'workgroup, SubgroupWidth64, Root<'kernel>, InitialEpoch>;

fn substitute<'first, 'second, 'kernel>(
    staged: Gfx950LdsTransposeTile<
        'first,
        Gfx950Fp4E2M1,
        Gfx950TransposeStaged,
        Brand<'first, 'kernel>,
    >,
    other: WorkgroupCapability<'second, Root<'kernel>, InitialEpoch>,
) -> WorkgroupCapability<'second, Root<'kernel>, NextEpoch<InitialEpoch>> {
    staged.publish(other).0
}

fn main() {}
