use fe2o3_device::{DisjointWrite, Global, Index1D, ThreadIndex};

fn cross_kernel_brand<OutputBrand, IndexBrand>(
    output: &mut Global<'_, f32, DisjointWrite<Index1D>, OutputBrand>,
    index: ThreadIndex<Index1D, IndexBrand>,
) {
    let _ = output.store(index.into_disjoint(), 1.0);
}
