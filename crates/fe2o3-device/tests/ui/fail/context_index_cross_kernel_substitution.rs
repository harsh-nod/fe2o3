use fe2o3_device::capability_memory::{DisjointWrite, Global};
use fe2o3_device::{
    CurrentTarget, Index1D, KernelCapabilityBrand, KernelContext, RegisteredLaunch,
};

enum FirstKernel {}
enum SecondKernel {}

type SecondBrand<'kernel> =
    KernelCapabilityBrand<'kernel, SecondKernel, CurrentTarget, RegisteredLaunch>;

fn substitute_index_brand<'kernel>(
    context: KernelContext<'kernel, FirstKernel>,
    output: &mut Global<'kernel, u32, DisjointWrite<Index1D>, SecondBrand<'kernel>>,
) {
    let index = context.invocation().index_1d().into_disjoint();
    let _ = output.store(index, 7);
}

fn main() {}
