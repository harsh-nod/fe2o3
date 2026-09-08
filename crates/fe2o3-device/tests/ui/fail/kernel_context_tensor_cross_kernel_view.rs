use fe2o3_device::{
    Bf16MfmaAMatrix, CurrentTarget, KernelCapabilityBrand, RegisteredLaunch, Wave64, WaveLane,
};

enum KernelA {}
enum KernelB {}

type BrandA<'kernel> = KernelCapabilityBrand<'kernel, KernelA, CurrentTarget, RegisteredLaunch>;
type BrandB<'kernel> = KernelCapabilityBrand<'kernel, KernelB, CurrentTarget, RegisteredLaunch>;

fn substitute_lane<'wave>(
    view: &Bf16MfmaAMatrix<'_, BrandA<'wave>>,
    lane: &'wave WaveLane<Wave64, BrandB<'wave>>,
) {
    let _ = view.load_m16k16(lane, 0, 0);
}

fn main() {}
