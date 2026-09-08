use fe2o3_device::capability_memory::{DisjointWrite, Global};
use fe2o3_device::{
    CurrentTarget, DisjointIndex, DisjointRowStripe2D, DisjointTile2D, Index1D, Invocation3D,
    KernelCapabilityBrand, KernelContext, RegisteredLaunch, ThreadIndex,
};

enum Kernel {}

type Brand<'kernel> = KernelCapabilityBrand<'kernel, Kernel, CurrentTarget, RegisteredLaunch>;

fn store_with_context_index<'kernel>(
    context: KernelContext<'kernel, Kernel>,
    mut output: Global<'kernel, u32, DisjointWrite<Index1D>, Brand<'kernel>>,
) {
    let invocation: Invocation3D<Brand<'kernel>> = context.invocation();
    let index: ThreadIndex<Index1D, Brand<'kernel>> = invocation.index_1d();
    let value = index.get() as u32;
    let index: DisjointIndex<Index1D, Brand<'kernel>> = index.into_disjoint();
    let _: bool = output.store(index, value);

    let index = context.invocation().index_1d();
    let _: Option<DisjointTile2D<Index1D, 64, 16, 16, 4, Brand<'kernel>>> =
        index.checked_tiled_2d::<64, 16, 16, 4>();
    let index = context.invocation().index_1d();
    let _: Option<DisjointRowStripe2D<Index1D, 64, 4, Brand<'kernel>>> =
        index.checked_row_striped_2d::<64, 4>();
}

fn main() {
    let _ = store_with_context_index;
}
