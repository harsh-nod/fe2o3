#![no_std]

use fe2o3_device::{DisjointSlice, KernelContext, KernelLaunch, KernelTarget, kernel, thread};

#[inline(never)]
fn user_scope<K, T: KernelTarget, L: KernelLaunch>(
    ctx: &mut KernelContext<'_, K, T, L>,
    seed: u32,
) -> u32 {
    ctx.with_workgroup(move |_wg| {
        unsafe {}
        seed
    })
}

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn unsafe_workgroup_callback(
    mut ctx: KernelContext<'_>,
    seed: u32,
    mut output: DisjointSlice<u32>,
) {
    let value = user_scope(&mut ctx, seed);
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = value;
    }
}
