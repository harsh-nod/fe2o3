use fe2o3_device::{WorkgroupLdsScope, WorkgroupPipeline};

pub fn excessive_pipeline_ring(mut scope: WorkgroupLdsScope<'_, ()>) {
    let _ = WorkgroupPipeline::<u32, 9, 64, 1, ()>::current(&mut scope);
}
