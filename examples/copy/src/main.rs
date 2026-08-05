use fe2o3_core::{DeviceBuffer, GpuContext, LaunchConfig};
use fe2o3_device::{DisjointSlice, kernel, thread};
use fe2o3_host::launch;
use std::path::PathBuf;

#[kernel]
pub fn copy(x: &[f32], mut out: DisjointSlice<f32>) {
    let i = thread::index_1d().get();
    let idx = thread::index_1d();
    if let Some(value) = out.get_mut(idx) {
        *value = x[i];
    }
}

fn main() -> fe2o3_core::Result<()> {
    const N: usize = 1024;

    let context = GpuContext::new(0)?;
    let stream = context.default_stream();

    let x_host: Vec<f32> = (0..N).map(|i| i as f32 * 0.5 - 12.0).collect();
    let x_dev = DeviceBuffer::from_host(&stream, &x_host)?;
    let out_dev = DeviceBuffer::<f32>::zeroed(&stream, N)?;

    let hsaco_dir = std::env::var_os("FE2O3_HSACO_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    // SAFETY: `copy.hsaco` is compiler-generated for the `copy` kernel in this
    // exact example. The subsequent launch remains independently unsafe.
    // This example requires that output to target this device and contain no init/fini kernels.
    let module = {
        let image = std::fs::read(hsaco_dir.join("copy.hsaco"))?;
        unsafe { context.load_module_from_bytes_unchecked(&image) }?
    };

    // SAFETY: `copy` expects read-only and writable f32 slice ABIs; `x_dev`
    // and `out_dev` are distinct N-element allocations kept alive until sync.
    unsafe {
        launch! {
            kernel: copy,
            stream: stream,
            module: module,
            config: LaunchConfig::for_num_elems(N as u32),
            args: [slice(x_dev), slice_mut(out_dev)]
        }
    }?;

    let out_host = out_dev.to_host_vec(&stream)?;
    for i in 0..N {
        assert!(
            (out_host[i] - x_host[i]).abs() < 1e-5,
            "mismatch at {i}: got {}, expected {}",
            out_host[i],
            x_host[i]
        );
    }

    println!("copy passed for {N} elements");
    Ok(())
}
