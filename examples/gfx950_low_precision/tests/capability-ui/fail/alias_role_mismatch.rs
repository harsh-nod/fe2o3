#![forbid(unsafe_code)]

use fe2o3_device::{DisjointWrite, ExclusiveReadWrite, Global, Index1D};

fn require_disjoint<Brand>(_: &Global<'_, f32, DisjointWrite<Index1D>, Brand>) {}

fn substitute_exclusive<Brand>(output: &Global<'_, f32, ExclusiveReadWrite, Brand>) {
    require_disjoint(output);
}
