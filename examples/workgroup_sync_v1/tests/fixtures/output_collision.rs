use fe2o3_device::{DisjointWrite, Global, Index1D};

fn collide<Brand>(output: &mut Global<'_, u32, DisjointWrite<Index1D>, Brand>) {
    let _ = output.store(0, 1);
}
