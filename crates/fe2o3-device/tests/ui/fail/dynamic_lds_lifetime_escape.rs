use fe2o3_device::{DynamicLds, WorkgroupLdsScope};

unsafe fn escape<'workgroup>(
    scope: &'workgroup mut WorkgroupLdsScope<'workgroup>,
    base: *mut u8,
) -> DynamicLds<'static, u32> {
    unsafe { DynamicLds::from_raw_parts(scope, base, 4).unwrap() }
}

fn main() {}
