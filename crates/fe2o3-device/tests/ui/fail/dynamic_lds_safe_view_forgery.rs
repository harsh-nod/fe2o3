use fe2o3_device::{DynamicLds, WorkgroupLdsScope};

fn forge<'workgroup>(scope: &'workgroup mut WorkgroupLdsScope<'workgroup>) {
    let _ = DynamicLds::<u32>::from_raw_parts(scope, core::ptr::null_mut(), 0);
}

fn main() {}
