use fe2o3_device::{
    Blocked, CurrentTarget, DisjointBlock, DisjointWrite, Global, Index1D, KernelCapabilityBrand,
    RegisteredLaunch,
};

enum KernelA {}
enum KernelB {}
type RootA<'a> = KernelCapabilityBrand<'a, KernelA, CurrentTarget, RegisteredLaunch>;
type RootB<'a> = KernelCapabilityBrand<'a, KernelB, CurrentTarget, RegisteredLaunch>;

fn reject<'a>(
    output: &mut Global<'a, f32, DisjointWrite<Blocked<Index1D, 64, 4>>, RootA<'a>>,
    block: &DisjointBlock<Index1D, 64, 4, RootB<'a>>,
) {
    let _ = output.store_block(block, 0, 1.0);
}

fn main() {}
