use fe2o3_device::{DynamicLds, WorkgroupLdsScope};

fn main() {
    let mut scope = WorkgroupLdsScope::current();
    let _ = DynamicLds::<i32>::exact_current::<0>(&mut scope);
}
