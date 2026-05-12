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
    let module = context.load_module_from_file(hsaco_dir.join("raw_const_minus.hsaco"))?;

    launch! {
        kernel: raw_const_minus,
        stream: stream,
        module: module,
        config: LaunchConfig::for_num_elems(N as u32),
        args: [slice(x_dev), slice_mut(out_dev)]
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
