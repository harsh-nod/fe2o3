use fe2o3_device::{DisjointWrite, Global, Index1D};

fn read_write_only<Brand>(output: &Global<'_, f32, DisjointWrite<Index1D>, Brand>) {
    let _ = output.load(0);
}
