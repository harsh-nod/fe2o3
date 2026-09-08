use fe2o3_device::{
    CurrentTarget, KernelCapabilityBrand, RegisteredLaunch, Wave64, WaveLane,
};

enum KernelA {}

type Lane = WaveLane<
    Wave64,
    KernelCapabilityBrand<'static, KernelA, CurrentTarget, RegisteredLaunch>,
>;

fn require_sync<T: Sync>() {}

fn main() {
    require_sync::<Lane>();
}
