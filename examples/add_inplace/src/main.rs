use fe2o3_core::{DeviceBuffer, GpuContext, LaunchConfig};
use fe2o3_device::{DisjointSlice, kernel, thread};
use fe2o3_host::launch;
use std::path::PathBuf;

#[kernel]
pub fn add_inplace(delta: f32, mut values: DisjointSlice<f32>) {
    let idx = thread::index_1d();
    if let Some(value) = values.get_mut(idx) {
        *value = *value + delta;
    }
}

fn main() -> fe2o3_core::Result<()> {
    const N: usize = 1024;
    const DELTA: f32 = 3.25;

    let context = GpuContext::new(0)?;
    let stream = context.default_stream();

    let values_host: Vec<f32> = (0..N).map(|i| i as f32 * 0.125 - 4.0).collect();
    let values_dev = DeviceBuffer::from_host(&stream, &values_host)?;

    let hsaco_dir = std::env::var_os("FE2O3_HSACO_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    // SAFETY: `add_inplace.hsaco` is compiler-generated for the `add_inplace`
    // kernel here. The subsequent launch remains independently unsafe.
    // This example requires that output to target this device and contain no init/fini kernels.
    let module =
        unsafe { context.load_module_from_file_unchecked(hsaco_dir.join("add_inplace.hsaco")) }?;

    // SAFETY: `add_inplace` expects an f32 followed by an N-element writable
    // slice; `values_dev` has that layout and lives through synchronization.
    unsafe {
        launch! {
            kernel: add_inplace,
            stream: stream,
            module: module,
            config: LaunchConfig::for_num_elems(N as u32),
            args: [scalar(DELTA), slice_mut(values_dev)]
        }
    }?;

    let values_result = values_dev.to_host_vec(&stream)?;
    for i in 0..N {
        let expected = values_host[i] + DELTA;
        assert!(
            (values_result[i] - expected).abs() < 1e-5,
            "mismatch at {i}: got {}, expected {expected}",
            values_result[i]
        );
    }

    println!("add_inplace passed for {N} elements");
    Ok(())
}
