use fe2o3_device::{DynamicLds, WorkgroupLdsScope};

fn caller_asserted_epoch(scope: &mut WorkgroupLdsScope<'_>) {
    let _ = DynamicLds::<i32>::exact_current::<64>(scope, 7);
}

fn main() {}
