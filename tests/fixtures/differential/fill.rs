mod common;

use common::{LEFT_F32, LENGTHS, POISON_F32, RIGHT_F32, case_seed, emit_bits32};
use fe2o3_core::{DeviceBuffer, DevicePtr, GpuContext, LaunchConfig};
use fe2o3_device::{kernel, thread};
use fe2o3_host::launch;
use std::path::PathBuf;

#[kernel]
pub fn differential_fill(bounds: &[f32], output: &mut [f32]) {
    let index = thread::index_1d().get();
    if index < bounds.len() && index < output.len() {
        output[index] = 42.5;
    }
}

fn main() -> fe2o3_core::Result<()> {
    let context = GpuContext::new(0)?;
    let stream = context.default_stream();
    let hsaco_dir = std::env::var_os("FE2O3_HSACO_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    // SAFETY: the harness just compiled this exact source for the observed target.
    let module = unsafe {
        context.load_module_from_file_unchecked(hsaco_dir.join("differential_fill.hsaco"))
    }?;

    for length in LENGTHS {
        let seed = case_seed(1, length);
        let bounds_host = vec![0.0_f32; length];
        let mut output_host = vec![f32::from_bits(POISON_F32); length + 2];
        output_host[0] = f32::from_bits(LEFT_F32);
        output_host[length + 1] = f32::from_bits(RIGHT_F32);
        let bounds = DeviceBuffer::from_host(&stream, &bounds_host)?;
        let output = DeviceBuffer::from_host(&stream, &output_host)?;
        if length != 0 {
            // SAFETY: this points at the allocation's N-element interior. The
            // surrounding elements are retained as inaccessible canaries.
            let output_interior = DevicePtr::from_raw(unsafe { output.raw_device_ptr().add(1) });
            // SAFETY: both buffers are distinct and the raw output view is N
            // elements long. All resources live through the synchronized copy.
            unsafe {
                launch! {
                    kernel: differential_fill,
                    stream: stream,
                    module: module,
                    config: LaunchConfig::for_num_elems(length as u32),
                    args: [slice(bounds), raw(output_interior), raw(length)]
                }
            }?;
        }
        emit_bits32("fill", seed, &output.to_host_vec(&stream)?);
    }
    Ok(())
}
