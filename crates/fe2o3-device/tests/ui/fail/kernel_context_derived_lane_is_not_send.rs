use fe2o3_device::{
    CurrentTarget, KernelCapabilityBrand, RegisteredLaunch, Wave64, WaveLane,
};

enum KernelA {}

type Lane = WaveLane<
    Wave64,
    KernelCapabilityBrand<'static, KernelA, CurrentTarget, RegisteredLaunch>,
>;

fn require_send<T: Send>() {}

fn main() {
    require_send::<Lane>();
}
