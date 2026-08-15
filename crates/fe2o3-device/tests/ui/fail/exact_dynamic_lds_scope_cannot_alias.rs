use fe2o3_device::{DynamicLds, WorkgroupLdsScope};

fn duplicate<'group>(scope: &'group mut WorkgroupLdsScope<'group>) {
    let first = unsafe { DynamicLds::<i32>::exact_from_compiler::<64>(scope, 0) };
    let second = unsafe { DynamicLds::<i32>::exact_from_compiler::<64>(scope, 0) };
    let _ = (first, second);
}

fn main() {
    let _ = duplicate;
}
