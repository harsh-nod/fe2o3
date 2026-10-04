use fe2o3_core::{DeviceBuffer, DeviceCopy, GpuContext};

#[derive(Clone, Copy, Debug, DeviceCopy, PartialEq)]
#[repr(C)]
struct DerivedPair {
    left: u32,
    right: f32,
}

#[test]
#[ignore = "requires a working HIP device"]
fn derived_struct_bytes_round_trip_through_device_memory() -> fe2o3_core::Result<()> {
    let context = GpuContext::new(0)?;
    let stream = context.default_stream();
    let input = [
        DerivedPair {
            left: 7,
            right: 1.25,
        },
        DerivedPair {
            left: u32::MAX,
            right: -3.5,
        },
    ];

    let buffer = DeviceBuffer::from_host(&stream, &input)?;
    let output = buffer.to_host_vec(&stream)?;

    // This exercises opaque byte transfer and host validity only. No kernel
    // interprets DerivedPair, so this is not device layout or ABI evidence.
    assert_eq!(output, input);
    Ok(())
}
