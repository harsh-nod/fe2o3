use fe2o3_core::{DeviceBuffer, GpuContext, LaunchConfig};
use fe2o3_device::{DisjointSlice, kernel, thread};
use fe2o3_host::launch;
use std::path::PathBuf;

#[kernel]
pub fn raw_parenthesized_sub(x: &[f32], mut out: DisjointSlice<f32>) {
    let base = thread::index_1d().get();
    let idx = thread::index_1d();
    let source = (base + 1) - base;
    if source < x.len() {
        if let Some(value) = out.get_mut(idx) {
            *value = x[source];
        }
    }
}

fn main() -> fe2o3_core::Result<()> {
    const N: usize = 1024;

    let context = GpuContext::new(0)?;
    let stream = context.default_stream();

    let x_host: Vec<f32> = (0..N).map(|i| i as f32 * 0.0625 + 7.0).collect();
    let x_dev = DeviceBuffer::from_host(&stream, &x_host)?;
    let out_dev = DeviceBuffer::<f32>::zeroed(&stream, N)?;

    let hsaco_dir = std::env::var_os("FE2O3_HSACO_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    // SAFETY: `raw_parenthesized_sub.hsaco` is compiler-generated for the
    // identically named kernel in this example. The subsequent launch remains unsafe.
    // This example requires that output to target this device and contain no init/fini kernels.
    let module = unsafe {
        context.load_module_from_file_unchecked(hsaco_dir.join("raw_parenthesized_sub.hsaco"))
    }?;

    // SAFETY: `raw_parenthesized_sub` expects two f32 slice ABIs; the distinct
    // buffers have N elements and live through sync, and every thread's
    // simplified source index is 1.
    unsafe {
        launch! {
            kernel: raw_parenthesized_sub,
            stream: stream,
            module: module,
            config: LaunchConfig::for_num_elems(N as u32),
            args: [slice(x_dev), slice_mut(out_dev)]
        }
    }?;

    let out_host = out_dev.to_host_vec(&stream)?;
    for (i, value) in out_host.iter().copied().enumerate() {
        let expected = x_host[1];
        assert!(
            (value - expected).abs() < 1e-5,
            "mismatch at {i}: got {value}, expected {expected}",
        );
    }

    println!("raw_parenthesized_sub passed for {N} elements");
    Ok(())
}
