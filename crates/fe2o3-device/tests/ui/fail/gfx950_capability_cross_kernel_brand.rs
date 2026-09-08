use fe2o3_device::{
    CurrentTarget, GlobalGfx950Fp4MfmaAMatrix, KernelCapabilityBrand, RegisteredLaunch,
    SubgroupBrand, SubgroupLane, SubgroupWidth64,
};

enum KernelA {}
enum KernelB {}
type RootA<'a> = KernelCapabilityBrand<'a, KernelA, CurrentTarget, RegisteredLaunch>;
type RootB<'a> = KernelCapabilityBrand<'a, KernelB, CurrentTarget, RegisteredLaunch>;
type MatrixA<'a> = SubgroupBrand<'a, SubgroupWidth64, RootA<'a>, fe2o3_device::InitialEpoch>;
type MatrixB<'a> = SubgroupBrand<'a, SubgroupWidth64, RootB<'a>, fe2o3_device::InitialEpoch>;

fn reject<'a>(
    matrix: &GlobalGfx950Fp4MfmaAMatrix<'_, '_, MatrixA<'a>, RootA<'a>>,
    lane: &SubgroupLane<SubgroupWidth64, MatrixB<'a>>,
) {
    let _ = matrix.load_m16k128(lane, 0, 0);
}

fn main() {}
