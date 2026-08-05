use fe2o3_core::{DeviceBuffer, GpuContext, LaunchConfig};
use fe2o3_device::{DisjointSlice, kernel, thread};
use fe2o3_host::launch;
use std::path::PathBuf;

const N: usize = 1024;
const LAST: usize = N - 1;

#[kernel]
pub fn raw_const_minus(x: &[f32], mut out: DisjointSlice<f32>) {
    let idx = thread::index_1d();
    let source = LAST - idx.get();
    if source < x.len() {
        if let Some(value) = out.get_mut(idx) {
            *value = x[source];
        }
    }
}

fn main() -> fe2o3_core::Result<()> {
    let context = GpuContext::new(0)?;
    let stream = context.default_stream();

    let x_host: Vec<f32> = (0..N).map(|i| i as f32 * 0.03125 - 4.0).collect();
    let x_dev = DeviceBuffer::from_host(&stream, &x_host)?;
    let out_dev = DeviceBuffer::<f32>::zeroed(&stream, N)?;

    let hsaco_dir = std::env::var_os("FE2O3_HSACO_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    // SAFETY: `raw_const_minus.hsaco` is compiler-generated for the
    // `raw_const_minus` kernel here. The subsequent launch remains independently unsafe.
    // This example requires that output to target this device and contain no init/fini kernels.
    let module = unsafe {
        context.load_module_from_file_unchecked(hsaco_dir.join("raw_const_minus.hsaco"))
    }?;

    // SAFETY: `raw_const_minus` expects two f32 slice ABIs; the distinct
    // allocations have N elements and live through sync, and the N launched
    // threads produce in-bounds indices.
    unsafe {
        launch! {
            kernel: raw_const_minus,
            stream: stream,
            module: module,
            config: LaunchConfig::for_num_elems(N as u32),
            args: [slice(x_dev), slice_mut(out_dev)]
        }
    }?;

    let out_host = out_dev.to_host_vec(&stream)?;
    for i in 0..N {
        let expected = x_host[LAST - i];
        assert!(
            (out_host[i] - expected).abs() < 1e-5,
            "mismatch at {i}: got {}, expected {expected}",
            out_host[i]
        );
    }

    println!("raw_const_minus passed for {N} elements");
    Ok(())
}
