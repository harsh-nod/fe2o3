use fe2o3_core::DeviceBuffer;
use fe2o3_host::{GeneratedLdsGemmSlice1HostAdapterV1, ObservedContext};
use fe2o3_hsaco_finalize::InspectedExactLdsGemmCompilerImportV1;

fn reuse_output_while_prepared(
    observed: &ObservedContext,
    compiler_import: &InspectedExactLdsGemmCompilerImportV1,
    a: DeviceBuffer<u16>,
    b: DeviceBuffer<u16>,
    mut c: DeviceBuffer<f32>,
) {
    let prepared = GeneratedLdsGemmSlice1HostAdapterV1::prepare(
        observed,
        compiler_import,
        a.view(..).unwrap(),
        b.view(..).unwrap(),
        c.view_mut(..).unwrap(),
    )
    .unwrap();
    let _reuse = c.len();
    let _still_live = prepared;
}

fn main() {}
