use fe2o3_device::{
    CurrentTarget, DisjointRowStripe2D, DisjointTile2D, Index1D, KernelCapabilityBrand,
    RegisteredLaunch, ThreadIndex,
};

enum KernelA {}
enum KernelB {}

type BrandA<'kernel> = KernelCapabilityBrand<'kernel, KernelA, CurrentTarget, RegisteredLaunch>;
type BrandB<'kernel> = KernelCapabilityBrand<'kernel, KernelB, CurrentTarget, RegisteredLaunch>;

fn substitute<'kernel>(index: ThreadIndex<Index1D, BrandA<'kernel>>) {
    let _: Option<DisjointTile2D<Index1D, 64, 16, 16, 4, BrandB<'kernel>>> =
        index.checked_tiled_2d::<64, 16, 16, 4>();
}

fn substitute_row<'kernel>(index: ThreadIndex<Index1D, BrandA<'kernel>>) {
    let _: Option<DisjointRowStripe2D<Index1D, 64, 4, BrandB<'kernel>>> =
        index.checked_row_striped_2d::<64, 4>();
}

fn main() {}
