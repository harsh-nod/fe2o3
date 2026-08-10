use fe2o3_core::{DeviceBuffer, GpuContext};
use fe2o3_host::{Gfx942OcmlArtifactIdentityV1, Gfx942OcmlSinErrorV1, Gfx942OcmlSinF32KernelV1};
use std::path::PathBuf;

#[test]
#[ignore = "requires a direct-worker OCML HSACO and gfx942:xnack-"]
fn direct_linked_ocml_sin_executes_with_exact_lifetimes() {
    let path = PathBuf::from(
        std::env::var_os("FE2O3_GFX942_OCML_HSACO")
            .expect("FE2O3_GFX942_OCML_HSACO must name retained worker output"),
    );
    let bytes = std::fs::read(path).unwrap();
    let identity = Gfx942OcmlArtifactIdentityV1::calculate(&bytes).unwrap();
    let context = GpuContext::new(0).unwrap();
    let stream = context.create_stream().unwrap();
    let kernel = unsafe {
        Gfx942OcmlSinF32KernelV1::load_reviewed_hsaco_unchecked(&context, &bytes, identity)
    }
    .unwrap();

    let input_values = [
        -3.0_f32, -1.5, -0.75, -0.25, -0.0, 0.0, 0.125, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 6.0, 12.0,
        24.0, 48.0,
    ];
    let input = DeviceBuffer::from_host(&stream, &input_values).unwrap();
    let mut output = DeviceBuffer::zeroed(&stream, input_values.len()).unwrap();
    kernel
        .launch_scoped(&stream, &input, &mut output, |operation| {
            operation.is_complete()
        })
        .unwrap()
        .unwrap();
    let actual = output.to_host_vec(&stream).unwrap();
    for (index, (actual, input)) in actual.iter().zip(input_values).enumerate() {
        let expected = input.sin();
        let tolerance = 8.0 * f32::EPSILON * expected.abs().max(1.0);
        assert!(
            (actual - expected).abs() <= tolerance,
            "OCML mismatch at {index}: expected {expected}, got {actual}"
        );
    }

    let mut mutated = bytes.clone();
    *mutated.last_mut().unwrap() ^= 1;
    assert!(matches!(
        unsafe {
            Gfx942OcmlSinF32KernelV1::load_reviewed_hsaco_unchecked(&context, &mutated, identity)
        },
        Err(Gfx942OcmlSinErrorV1::ArtifactSubstitution)
    ));

    let other_context = GpuContext::new(0).unwrap();
    let other_stream = other_context.create_stream().unwrap();
    assert!(matches!(
        kernel.launch_scoped(&other_stream, &input, &mut output, |_| ()),
        Err(Gfx942OcmlSinErrorV1::ContextSubstitution)
    ));
}
