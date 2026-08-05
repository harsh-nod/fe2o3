use fe2o3_core::{DeviceBuffer, GpuContext, LaunchConfig};
use fe2o3_device::{DisjointSlice, kernel, thread};
use fe2o3_host::launch;
use std::path::PathBuf;

#[kernel]
pub fn previous(x: &[f32], mut out: DisjointSlice<f32>) {
    let idx = thread::index_1d();
    let source = idx.offset_signed(-1);
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

    let x_host: Vec<f32> = (0..N).map(|i| i as f32 * 0.5 + 3.0).collect();
    let x_dev = DeviceBuffer::from_host(&stream, &x_host)?;
    let out_dev = DeviceBuffer::<f32>::zeroed(&stream, N)?;

    let hsaco_dir = std::env::var_os("FE2O3_HSACO_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let module = context.load_module_from_file(hsaco_dir.join("previous.hsaco"))?;

    // SAFETY: `previous` expects two f32 slice ABIs; `x_dev` and `out_dev` are
    // distinct N-element allocations that live through sync, and the kernel
    // guards its shifted read.
    unsafe {
        launch! {
            kernel: previous,
            stream: stream,
            module: module,
            config: LaunchConfig::for_num_elems(N as u32),
            args: [slice(x_dev), slice_mut(out_dev)]
        }
    }?;

    let out_host = out_dev.to_host_vec(&stream)?;
    for i in 0..N {
        let expected = if i == 0 { 0.0 } else { x_host[i - 1] };
        assert!(
            (out_host[i] - expected).abs() < 1e-5,
            "mismatch at {i}: got {}, expected {expected}",
            out_host[i]
        );
    }

    println!("previous passed for {N} elements");
    Ok(())
}
