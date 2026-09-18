#![no_std]

use fe2o3_device::{DisjointSlice, KernelContext, WorkgroupCapability, kernel, thread};

fn user_callback<K>(seed: u32) -> impl for<'wg> FnOnce(WorkgroupCapability<'wg, K>) -> u32 {
    move |_wg| {
        unsafe {}
        seed
    }
}

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn unsafe_external_workgroup_callback(
    mut ctx: KernelContext<'_>,
    seed: u32,
    mut output: DisjointSlice<u32>,
) {
    let value = ctx.with_workgroup(user_callback(seed));
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = value;
    }
}
