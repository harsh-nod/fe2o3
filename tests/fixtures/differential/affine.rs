mod common;

use common::{LEFT_F32, LENGTHS, POISON_F32, RIGHT_F32, case_seed, emit_f32, sample_f32};
use fe2o3_core::{DeviceBuffer, DevicePtr, GpuContext, LaunchConfig};
use fe2o3_device::{kernel, thread};
use fe2o3_host::launch;
use std::path::PathBuf;

#[kernel]
pub fn differential_affine(alpha: f32, bias: f32, input: &[f32], output: &mut [f32]) {
    let index = thread::index_1d().get();
    if index < input.len() && index < output.len() {
        output[index] = alpha * input[index] + bias;
    }
}

fn main() -> fe2o3_core::Result<()> {
    const ALPHA: f32 = 1.25;
    const BIAS: f32 = -0.75;

    let context = GpuContext::new(0)?;
    let stream = context.default_stream();
    let hsaco_dir = std::env::var_os("FE2O3_HSACO_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    // SAFETY: the harness just compiled this exact source for the observed target.
    let module = unsafe {
        context.load_module_from_file_unchecked(hsaco_dir.join("differential_affine.hsaco"))
    }?;

    for length in LENGTHS {
        let seed = case_seed(3, length);
        let input_host: Vec<f32> = (0..length).map(|index| sample_f32(seed, index)).collect();
        let mut output_host = vec![f32::from_bits(POISON_F32); length + 2];
        output_host[0] = f32::from_bits(LEFT_F32);
        output_host[length + 1] = f32::from_bits(RIGHT_F32);
        let input = DeviceBuffer::from_host(&stream, &input_host)?;
        let output = DeviceBuffer::from_host(&stream, &output_host)?;
        if length != 0 {
            // SAFETY: this points at the allocation's N-element interior. The
            // surrounding elements are retained as inaccessible canaries.
            let output_interior = DevicePtr::from_raw(unsafe { output.raw_device_ptr().add(1) });
            // SAFETY: input and output are distinct, the raw output view is N
            // elements long, and both allocations live through synchronization.
            unsafe {
                launch! {
                    kernel: differential_affine,
                    stream: stream,
                    module: module,
                    config: LaunchConfig::for_num_elems(length as u32),
                    args: [scalar(ALPHA), scalar(BIAS), slice(input), raw(output_interior), raw(length)]
                }
            }?;
        }
        emit_f32("affine", seed, &output.to_host_vec(&stream)?);
    }
    Ok(())
}
