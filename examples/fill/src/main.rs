use fe2o3_core::{DeviceBuffer, GpuContext, LaunchConfig};
use fe2o3_device::{DisjointSlice, kernel, thread};
use fe2o3_host::launch;
use std::path::PathBuf;

#[kernel]
pub fn fill(mut out: DisjointSlice<f32>) {
    let idx = thread::index_1d();
    if let Some(value) = out.get_mut(idx) {
        *value = 42.5;
    }
}

fn main() -> fe2o3_core::Result<()> {
    const N: usize = 1024;
    const EXPECTED: f32 = 42.5;

    let context = GpuContext::new(0)?;
    let stream = context.default_stream();

    let out_dev = DeviceBuffer::<f32>::zeroed(&stream, N)?;

    let hsaco_dir = std::env::var_os("FE2O3_HSACO_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let module = context.load_module_from_file(hsaco_dir.join("fill.hsaco"))?;

    // SAFETY: `fill` expects one writable f32 slice ABI; `out_dev` contains N
    // elements, one per launched thread, and remains alive until sync.
    unsafe {
        launch! {
            kernel: fill,
            stream: stream,
            module: module,
            config: LaunchConfig::for_num_elems(N as u32),
            args: [slice_mut(out_dev)]
        }
    }?;

    let out_host = out_dev.to_host_vec(&stream)?;
    for (i, value) in out_host.iter().copied().enumerate() {
        assert!(
            (value - EXPECTED).abs() < 1e-5,
            "mismatch at {i}: got {value}, expected {EXPECTED}"
        );
    }

    println!("fill passed for {N} elements");
    Ok(())
}
