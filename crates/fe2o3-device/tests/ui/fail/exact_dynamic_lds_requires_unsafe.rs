use fe2o3_device::{DynamicLds, WorkgroupLdsScope};

fn forge<'group>(scope: &'group mut WorkgroupLdsScope<'group>) {
    let _ = DynamicLds::<i32>::exact_from_compiler::<64>(scope, 0);
}

fn main() {
    let _ = forge;
}
