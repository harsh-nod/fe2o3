use fe2o3_core::{DeviceBuffer, GpuContext, LaunchConfig};
use fe2o3_device::{DisjointSlice, kernel, thread};
use fe2o3_host::launch;
use std::path::PathBuf;

#[kernel]
pub fn raw_neighbors(x: &[f32], mut out: DisjointSlice<f32>) {
    let idx = thread::index_1d();
    let center = idx.get();
    if center > 0 && center + 1 < x.len() {
        let left = center - 1;
        let right = center + 1;
        if let Some(value) = out.get_mut(idx) {
            *value = 0.25 * x[left] + 0.75 * x[right];
        }
    }
}

fn main() -> fe2o3_core::Result<()> {
    const N: usize = 1024;

    let context = GpuContext::new(0)?;
    let stream = context.default_stream();

    let x_host: Vec<f32> = (0..N).map(|i| i as f32 * 0.0625 + 1.0).collect();
    let x_dev = DeviceBuffer::from_host(&stream, &x_host)?;
    let out_dev = DeviceBuffer::<f32>::zeroed(&stream, N)?;

    let hsaco_dir = std::env::var_os("FE2O3_HSACO_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    // SAFETY: `raw_neighbors.hsaco` is compiler-generated for the
    // `raw_neighbors` kernel here. The subsequent launch remains independently unsafe.
    // This example requires that output to target this device and contain no init/fini kernels.
    let module =
        unsafe { context.load_module_from_file_unchecked(hsaco_dir.join("raw_neighbors.hsaco")) }?;

    // SAFETY: `raw_neighbors` expects two f32 slice ABIs; the buffers are
    // distinct N-element allocations that live through sync, and the kernel
    // guards both neighbor reads.
    unsafe {
        launch! {
            kernel: raw_neighbors,
            stream: stream,
            module: module,
            config: LaunchConfig::for_num_elems(N as u32),
            args: [slice(x_dev), slice_mut(out_dev)]
        }
    }?;

    let out_host = out_dev.to_host_vec(&stream)?;
    for i in 0..N {
        let expected = if i == 0 || i + 1 == N {
            0.0
        } else {
            0.25 * x_host[i - 1] + 0.75 * x_host[i + 1]
        };
        assert!(
            (out_host[i] - expected).abs() < 1e-5,
            "mismatch at {i}: got {}, expected {expected}",
            out_host[i]
        );
    }

    println!("raw_neighbors passed for {N} elements");
    Ok(())
}
