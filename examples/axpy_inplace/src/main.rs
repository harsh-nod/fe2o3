use fe2o3_core::{DeviceBuffer, GpuContext, LaunchConfig};
use fe2o3_device::{kernel, thread};
use fe2o3_host::launch;
use std::path::PathBuf;

#[kernel]
pub fn axpy_inplace(alpha: f32, x: &[f32], y: &mut [f32]) {
    let idx = thread::index_1d();
    let i = idx.get();
    if i < y.len() {
        y[i] = alpha * x[i] + y[i];
    }
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
    let module = context.load_module_from_file(hsaco_dir.join("axpy_inplace.hsaco"))?;
    launch! {
        kernel: axpy_inplace,
        stream: stream,
        module: module,
        config: LaunchConfig::for_num_elems(N as u32),
        args: [scalar(ALPHA), slice(x_dev), slice_mut(y_dev)]
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
