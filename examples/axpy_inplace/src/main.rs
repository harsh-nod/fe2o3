use fe2o3_core::{DeviceBuffer, GpuContext, LaunchConfig};
use fe2o3_device::{DisjointSlice, kernel, thread};
use fe2o3_host::launch;
use std::path::PathBuf;

#[kernel]
pub fn axpy_inplace(alpha: f32, x: &[f32], mut y: DisjointSlice<f32>) {
    let idx = thread::index_1d();
    let i = idx.get();
    let Some(y_value) = y.get_mut(idx) else {
        return;
    };
    if i >= x.len() {
        fe2o3_device::trap();
        return;
    }
    *y_value = alpha * x[i] + *y_value;
}

fn main() -> fe2o3_core::Result<()> {
    const N: usize = 1024;
    const ALPHA: f32 = 0.75;

    let context = GpuContext::new(0)?;
    let stream = context.default_stream();

    let x_host: Vec<f32> = (0..N).map(|i| i as f32 * 0.5).collect();
    let y_host: Vec<f32> = (0..N).map(|i| 10.0 + i as f32 * 0.25).collect();

    let x_dev = DeviceBuffer::from_host(&stream, &x_host)?;
    let y_dev = DeviceBuffer::from_host(&stream, &y_host)?;

    let hsaco_dir = std::env::var_os("FE2O3_HSACO_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    // SAFETY: `axpy_inplace.hsaco` is compiler-generated for the `axpy_inplace`
    // kernel here. The subsequent launch remains independently unsafe.
    // This example requires that output to target this device and contain no init/fini kernels.
    let module =
        unsafe { context.load_module_from_file_unchecked(hsaco_dir.join("axpy_inplace.hsaco")) }?;
    // SAFETY: `axpy_inplace` expects an f32 and two f32 slice ABIs; `x_dev`
    // and `y_dev` are distinct N-element allocations kept alive until sync.
    unsafe {
        launch! {
            kernel: axpy_inplace,
            stream: stream,
            module: module,
            config: LaunchConfig::for_num_elems(N as u32),
            args: [scalar(ALPHA), slice(x_dev), slice_mut(y_dev)]
        }
    }?;

    let y_result = y_dev.to_host_vec(&stream)?;
    for i in 0..N {
        let expected = ALPHA * x_host[i] + y_host[i];
        assert!(
            (y_result[i] - expected).abs() < 1e-5,
            "mismatch at {i}: expected {expected}, got {}",
            y_result[i]
        );
    }

    println!("axpy_inplace passed for {N} elements");
    Ok(())
}
