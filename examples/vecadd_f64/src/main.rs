use fe2o3_core::{DeviceBuffer, GpuContext, LaunchConfig};
use fe2o3_device::{DisjointSlice, kernel, thread};
use fe2o3_host::launch;
use std::path::PathBuf;

#[kernel]
pub fn vecadd_f64(a: &[f64], b: &[f64], mut c: DisjointSlice<f64>) {
    let idx = thread::index_1d();
    if let Some(value) = c.get_mut(idx) {
        *value = a[idx.get()] + b[idx.get()];
    }
}

fn main() -> fe2o3_core::Result<()> {
    const N: usize = 1024;

    let context = GpuContext::new(0)?;
    let stream = context.default_stream();

    let a_host: Vec<f64> = (0..N).map(|i| i as f64 * 0.25).collect();
    let b_host: Vec<f64> = (0..N).map(|i| 500.0 - i as f64 * 0.125).collect();

    let a_dev = DeviceBuffer::from_host(&stream, &a_host)?;
    let b_dev = DeviceBuffer::from_host(&stream, &b_host)?;
    let c_dev = DeviceBuffer::<f64>::zeroed(&stream, N)?;

    let hsaco_dir = std::env::var_os("FE2O3_HSACO_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let module = context.load_module_from_file(hsaco_dir.join("vecadd_f64.hsaco"))?;

    launch! {
        kernel: vecadd_f64,
        stream: stream,
        module: module,
        config: LaunchConfig::for_num_elems(N as u32),
        args: [slice(a_dev), slice(b_dev), slice_mut(c_dev)]
    }?;

    let c_host = c_dev.to_host_vec(&stream)?;
    for i in 0..N {
        let expected = a_host[i] + b_host[i];
        assert!(
            (c_host[i] - expected).abs() < 1e-10,
            "mismatch at {i}: got {}, expected {expected}",
            c_host[i]
        );
    }

    println!("vecadd_f64 passed for {N} elements");
    Ok(())
}
