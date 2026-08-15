use fe2o3_core::{DeviceBufferView, DeviceBufferViewMut};
use fe2o3_host::{
    GeneratedLdsGemmSlice1HostAdapterErrorV1, GeneratedLdsGemmSlice1HostAdapterV1,
    ObservedContext,
};
use fe2o3_hsaco_finalize::{
    InspectedExactLdsGemmCompilerImportIdentityV1, InspectedExactLdsGemmCompilerImportV1,
};

fn prepare_then_consume_import<'a, 'b, 'c>(
    observed: &ObservedContext,
    compiler_import: InspectedExactLdsGemmCompilerImportV1,
    a: DeviceBufferView<'a, u16>,
    b: DeviceBufferView<'b, u16>,
    c: DeviceBufferViewMut<'c, f32>,
) -> Result<
    (
        GeneratedLdsGemmSlice1HostAdapterV1<'a, 'b, 'c>,
        InspectedExactLdsGemmCompilerImportIdentityV1,
    ),
    GeneratedLdsGemmSlice1HostAdapterErrorV1,
> {
    let prepared =
        GeneratedLdsGemmSlice1HostAdapterV1::prepare(observed, &compiler_import, a, b, c)?;
    let consumed_identity = compiler_import.identity();
    Ok((prepared, consumed_identity))
}

fn main() {}
