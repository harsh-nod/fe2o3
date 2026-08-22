use fe2o3_device::{DynamicLds, WorkgroupLdsScope};

fn duplicate<'group>(scope: &'group mut WorkgroupLdsScope<'group>) {
    let first = DynamicLds::<i32>::exact_current::<64>(scope);
    let second = DynamicLds::<i32>::exact_current::<64>(scope);
    let _ = (first, second);
}

fn main() {
    let _ = duplicate;
}
