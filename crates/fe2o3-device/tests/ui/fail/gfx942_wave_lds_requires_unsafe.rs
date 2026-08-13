use fe2o3_device::{Gfx942Collectives, Gfx942StaticLdsU32x256};

fn reject(
    context: &Gfx942Collectives,
    scratch: &mut Gfx942StaticLdsU32x256,
) {
    let _ = context.wave64_reduce_sum_active_u32(1, 7);
    let _ = context.workgroup256_reduce_sum_active_u32(scratch, 1, 7);
}

fn main() {}
