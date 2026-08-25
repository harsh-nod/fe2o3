use fe2o3_core::GpuContext;
use std::sync::Arc;

fn attempt_raw_load(context: &Arc<GpuContext>) {
    let _ = unsafe { context.load_module_from_bytes_unchecked(&[]) };
}

fn main() {}
