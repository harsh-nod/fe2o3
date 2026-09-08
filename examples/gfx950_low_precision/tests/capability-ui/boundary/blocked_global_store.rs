#![forbid(unsafe_code)]

use fe2o3_device::{Blocked, DisjointBlock, DisjointWrite, Global, Index1D};

fn blocked_store<Brand>(
    output: &mut Global<'_, f32, DisjointWrite<Blocked<Index1D, 16, 4>>, Brand>,
    block: DisjointBlock<Index1D, 16, 4, Brand>,
) {
    let _ = output.store_block(&block, 0, 1.0);
}
