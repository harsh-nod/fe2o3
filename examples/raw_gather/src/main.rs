use fe2o3_core::{DeviceBuffer, GpuContext, LaunchConfig};
use fe2o3_device::{DisjointSlice, kernel, thread};
use fe2o3_host::launch;
use std::path::PathBuf;

#[kernel]
pub fn raw_gather(x: &[f32], mut out: DisjointSlice<f32>) {
    let idx = thread::index_1d();
    let source = idx.get() * 2 + 1;
    let Some(value) = out.get_mut(idx) else {
        return;
    };
    if source >= x.len() {
        fe2o3_device::trap();
        return;
    }
    *value = x[source];
}

fn main() -> fe2o3_core::Result<()> {
    const N: usize = 1024;

    let context = GpuContext::new(0)?;
    let stream = context.default_stream();

    let x_host: Vec<f32> = (0..N * 2).map(|i| i as f32 * 0.03125 - 4.0).collect();
    let x_dev = DeviceBuffer::from_host(&stream, &x_host)?;
    let out_dev = DeviceBuffer::<f32>::zeroed(&stream, N)?;

    let hsaco_dir = std::env::var_os("FE2O3_HSACO_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    // SAFETY: `raw_gather.hsaco` is compiler-generated for the `raw_gather`
    // kernel here. The subsequent launch remains independently unsafe.
    // This example requires that output to target this device and contain no init/fini kernels.
    let module =
        unsafe { context.load_module_from_file_unchecked(hsaco_dir.join("raw_gather.hsaco")) }?;

    // SAFETY: `raw_gather` expects f32 slice ABIs; `x_dev` has 2 * N readable
    // elements and `out_dev` has N disjoint writable elements through sync.
    unsafe {
        launch! {
            kernel: raw_gather,
            stream: stream,
            module: module,
            config: LaunchConfig::for_num_elems(N as u32),
            args: [slice(x_dev), slice_mut(out_dev)]
        }
    }?;

    let out_host = out_dev.to_host_vec(&stream)?;
    for i in 0..N {
        let expected = x_host[i * 2 + 1];
        assert!(
            (out_host[i] - expected).abs() < 1e-5,
            "mismatch at {i}: got {}, expected {expected}",
            out_host[i]
        );
    }

    println!("raw_gather passed for {N} elements");
    Ok(())
}
