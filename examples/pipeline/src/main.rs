use fe2o3_core::{DeviceBuffer, GpuContext, LaunchConfig};
use fe2o3_device::{DisjointSlice, kernel, thread};
use fe2o3_host::launch;
use std::path::PathBuf;

#[kernel]
pub fn scale_stage(alpha: f32, x: &[f32], mut tmp: DisjointSlice<f32>) {
    let i = thread::index_1d().get();
    let idx = thread::index_1d();
    if let Some(value) = tmp.get_mut(idx) {
        *value = alpha * x[i];
    }
}

#[kernel]
pub fn bias_stage(tmp: &[f32], beta: f32, mut out: DisjointSlice<f32>) {
    let i = thread::index_1d().get();
    let idx = thread::index_1d();
    if let Some(value) = out.get_mut(idx) {
        *value = tmp[i] + beta;
    }
}

fn main() -> fe2o3_core::Result<()> {
    const N: usize = 1024;
    const ALPHA: f32 = 1.25;
    const BETA: f32 = 3.5;

    let context = GpuContext::new(0)?;
    let stream = context.default_stream();

    let x_host: Vec<f32> = (0..N).map(|i| i as f32 * 0.125).collect();

    let x_dev = DeviceBuffer::from_host(&stream, &x_host)?;
    let tmp_dev = DeviceBuffer::<f32>::zeroed(&stream, N)?;
    let out_dev = DeviceBuffer::<f32>::zeroed(&stream, N)?;

    let hsaco_dir = std::env::var_os("FE2O3_HSACO_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    // SAFETY: `scale_stage.hsaco` is compiler-generated for the `scale_stage`
    // kernel here. The subsequent launch remains independently unsafe.
    // This example requires that output to target this device and contain no init/fini kernels.
    let scale_module =
        unsafe { context.load_module_from_file_unchecked(hsaco_dir.join("scale_stage.hsaco")) }?;
    // SAFETY: `scale_stage` expects an f32 and two f32 slice ABIs; `x_dev`
    // and `tmp_dev` are distinct N-element allocations kept alive until sync.
    unsafe {
        launch! {
            kernel: scale_stage,
            stream: stream,
            module: scale_module,
            config: LaunchConfig::for_num_elems(N as u32),
            args: [scalar(ALPHA), slice(x_dev), slice_mut(tmp_dev)]
        }
    }?;

    // SAFETY: `bias_stage.hsaco` is compiler-generated for the `bias_stage`
    // kernel here. The subsequent launch remains independently unsafe.
    // This example requires that output to target this device and contain no init/fini kernels.
    let bias_module =
        unsafe { context.load_module_from_file_unchecked(hsaco_dir.join("bias_stage.hsaco")) }?;
    // SAFETY: `bias_stage` expects a slice, f32, and writable slice ABI;
    // same-stream ordering initializes N tmp elements before the distinct
    // N-element output is written, and both allocations live through sync.
    unsafe {
        launch! {
            kernel: bias_stage,
            stream: stream,
            module: bias_module,
            config: LaunchConfig::for_num_elems(N as u32),
            args: [slice(tmp_dev), scalar(BETA), slice_mut(out_dev)]
        }
    }?;

    let out_host = out_dev.to_host_vec(&stream)?;
    for i in 0..N {
        let expected = ALPHA * x_host[i] + BETA;
        assert!(
            (out_host[i] - expected).abs() < 1e-5,
            "mismatch at {i}: expected {expected}, got {}",
            out_host[i]
        );
    }

    println!("pipeline passed for {N} elements");
    Ok(())
}
