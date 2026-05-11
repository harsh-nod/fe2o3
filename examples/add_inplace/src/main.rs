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
    let module = context.load_module_from_file(hsaco_dir.join("add_inplace.hsaco"))?;

    launch! {
        kernel: add_inplace,
        stream: stream,
        module: module,
        config: LaunchConfig::for_num_elems(N as u32),
        args: [scalar(DELTA), slice_mut(values_dev)]
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
