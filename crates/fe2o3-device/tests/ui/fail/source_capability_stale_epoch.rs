use fe2o3_device::{
    InitialEpoch, NextEpoch, WorkgroupCapability, WorkgroupLds, WorkgroupLdsPublished,
};

fn stale<'workgroup, Brand>(
    lds: &WorkgroupLds<'workgroup, u32, 64, WorkgroupLdsPublished, Brand, InitialEpoch>,
    current: &WorkgroupCapability<'workgroup, Brand, NextEpoch<InitialEpoch>>,
) {
    let _ = lds.read(current, 0);
}

fn main() {}
