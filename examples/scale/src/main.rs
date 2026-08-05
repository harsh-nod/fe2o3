use fe2o3_core::{DeviceBuffer, GpuContext, LaunchConfig};
use fe2o3_device::{DisjointSlice, kernel, thread};
use fe2o3_host::launch;
use std::path::PathBuf;

#[kernel]
pub fn scale(alpha: f32, x: &[f32], mut y: DisjointSlice<f32>) {
    let idx = thread::index_1d();
    if let Some(out) = y.get_mut(idx) {
        *out = alpha * x[idx.get()];
    }
}

fn main() -> fe2o3_core::Result<()> {
    const N: usize = 1024;
    const ALPHA: f32 = 2.5;

    let context = GpuContext::new(0)?;
    let stream = context.default_stream();

    let x_host: Vec<f32> = (0..N).map(|i| i as f32 * 0.25).collect();

    let x_dev = DeviceBuffer::from_host(&stream, &x_host)?;
    let y_dev = DeviceBuffer::<f32>::zeroed(&stream, N)?;

    let hsaco_dir = std::env::var_os("FE2O3_HSACO_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    // SAFETY: `scale.hsaco` is compiler-generated for the `scale` kernel in
    // this exact example. The subsequent launch remains independently unsafe.
    // This example requires that output to target this device and contain no init/fini kernels.
    let module = unsafe { context.load_module_from_file_unchecked(hsaco_dir.join("scale.hsaco")) }?;
    // SAFETY: `scale` expects an f32 and two f32 slice ABIs; `x_dev` and
    // `y_dev` are distinct N-element allocations kept alive until sync.
    unsafe {
        launch! {
            kernel: scale,
            stream: stream,
            module: module,
            config: LaunchConfig::for_num_elems(N as u32),
            args: [scalar(ALPHA), slice(x_dev), slice_mut(y_dev)]
        }
    }?;

    let y_host = y_dev.to_host_vec(&stream)?;
    for i in 0..N {
        let expected = ALPHA * x_host[i];
        assert!(
            (y_host[i] - expected).abs() < 1e-5,
            "mismatch at {i}: expected {expected}, got {}",
            y_host[i]
        );
    }

    println!("scale passed for {N} elements");
    Ok(())
}
