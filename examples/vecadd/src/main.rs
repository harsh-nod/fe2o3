use fe2o3_core::{DeviceBuffer, GpuContext, LaunchConfig};
use fe2o3_device::{DisjointSlice, kernel, thread};
use fe2o3_host::launch;
use std::path::PathBuf;

#[kernel]
pub fn vecadd(a: &[f32], b: &[f32], mut c: DisjointSlice<f32>) {
    let idx = thread::index_1d();
    if let Some(out) = c.get_mut(idx) {
        *out = a[idx.get()] + b[idx.get()];
    }
}

fn main() -> fe2o3_core::Result<()> {
    const N: usize = 1024;

    let context = GpuContext::new(0)?;
    let stream = context.default_stream();

    let a_host: Vec<f32> = (0..N).map(|i| i as f32).collect();
    let b_host: Vec<f32> = (0..N).map(|i| (i * 2) as f32).collect();

    let a_dev = DeviceBuffer::from_host(&stream, &a_host)?;
    let b_dev = DeviceBuffer::from_host(&stream, &b_host)?;
    let c_dev = DeviceBuffer::<f32>::zeroed(&stream, N)?;

    let hsaco_dir = std::env::var_os("FE2O3_HSACO_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    // SAFETY: `vecadd.hsaco` is compiler-generated for the `vecadd` kernel in
    // this exact example. The subsequent launch remains independently unsafe.
    // This example requires that output to target this device and contain no init/fini kernels.
    let module =
        unsafe { context.load_module_from_file_unchecked(hsaco_dir.join("vecadd.hsaco")) }?;
    // SAFETY: `vecadd` expects three f32 slice ABIs; a, b, and c are distinct
    // N-element allocations kept alive until stream synchronization.
    unsafe {
        launch! {
            kernel: vecadd,
            stream: stream,
            module: module,
            config: LaunchConfig::for_num_elems(N as u32),
            args: [slice(a_dev), slice(b_dev), slice_mut(c_dev)]
        }
    }?;

    let c_host = c_dev.to_host_vec(&stream)?;
    for i in 0..N {
        let expected = a_host[i] + b_host[i];
        assert!(
            (c_host[i] - expected).abs() < 1e-5,
            "mismatch at {i}: expected {expected}, got {}",
            c_host[i]
        );
    }

    println!("vecadd passed for {N} elements");
    Ok(())
}
