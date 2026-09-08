use fe2o3_device::{
    CurrentTarget, Gfx950Fp4E2M1, Gfx950LdsTransposeTile, Gfx950TransposeUninitialized,
    InitialEpoch, KernelCapabilityBrand, KernelContext, RegisteredLaunch, SubgroupBrand,
    SubgroupWidth64,
};

enum Kernel {}

type Root<'kernel> = KernelCapabilityBrand<'kernel, Kernel, CurrentTarget, RegisteredLaunch>;

fn escape<'kernel>(
    context: &mut KernelContext<'kernel, Kernel>,
) -> Gfx950LdsTransposeTile<
    'static,
    Gfx950Fp4E2M1,
    Gfx950TransposeUninitialized,
    SubgroupBrand<'static, SubgroupWidth64, Root<'kernel>, InitialEpoch>,
> {
    context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        subgroup
            .gfx950_wave16(workgroup.epoch())
            .transpose_tile::<Gfx950Fp4E2M1>()
    })
}

fn main() {}
