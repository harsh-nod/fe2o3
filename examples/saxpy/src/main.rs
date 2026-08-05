use fe2o3_core::{DeviceBuffer, GpuContext, LaunchConfig};
use fe2o3_device::{DisjointSlice, kernel, thread};
use fe2o3_host::launch;
use std::path::PathBuf;

#[kernel]
pub fn saxpy(alpha: f32, x: &[f32], y: &[f32], mut out: DisjointSlice<f32>) {
    let idx = thread::index_1d();
    if let Some(value) = out.get_mut(idx) {
        *value = alpha * x[idx.get()] + y[idx.get()];
    }
}

fn main() -> fe2o3_core::Result<()> {
    const N: usize = 1024;
    const ALPHA: f32 = 1.75;

    let context = GpuContext::new(0)?;
    let stream = context.default_stream();

    let x_host: Vec<f32> = (0..N).map(|i| i as f32 * 0.5).collect();
    let y_host: Vec<f32> = (0..N).map(|i| 100.0 - i as f32 * 0.25).collect();

    let x_dev = DeviceBuffer::from_host(&stream, &x_host)?;
    let y_dev = DeviceBuffer::from_host(&stream, &y_host)?;
    let out_dev = DeviceBuffer::<f32>::zeroed(&stream, N)?;

    let hsaco_dir = std::env::var_os("FE2O3_HSACO_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let module = context.load_module_from_file(hsaco_dir.join("saxpy.hsaco"))?;
    // SAFETY: `saxpy` expects an f32 and three f32 slice ABIs; x, y, and out
    // are distinct N-element allocations kept alive until synchronization.
    unsafe {
        launch! {
            kernel: saxpy,
            stream: stream,
            module: module,
            config: LaunchConfig::for_num_elems(N as u32),
            args: [scalar(ALPHA), slice(x_dev), slice(y_dev), slice_mut(out_dev)]
        }
    }?;

    let out_host = out_dev.to_host_vec(&stream)?;
    for i in 0..N {
        let expected = ALPHA * x_host[i] + y_host[i];
        assert!(
            (out_host[i] - expected).abs() < 1e-5,
            "mismatch at {i}: expected {expected}, got {}",
            out_host[i]
        );
    }

    println!("saxpy passed for {N} elements");
    Ok(())
}
