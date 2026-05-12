use fe2o3_core::{DeviceBuffer, GpuContext, LaunchConfig};
use fe2o3_device::{kernel, thread};
use fe2o3_host::launch;
use std::path::PathBuf;

#[kernel]
pub fn raw_output_shift(x: &[f32], out: &mut [f32]) {
    let idx = thread::index_1d();
    let source = idx.get();
    let target = source + 1;
    if source < x.len() && target < out.len() {
        out[target] = x[source] * 2.0;
    }
}

fn main() -> fe2o3_core::Result<()> {
    const N: usize = 1024;

    let context = GpuContext::new(0)?;
    let stream = context.default_stream();

    let x_host: Vec<f32> = (0..(N - 1)).map(|i| i as f32 * 0.125 - 2.0).collect();
    let x_dev = DeviceBuffer::from_host(&stream, &x_host)?;
    let out_dev = DeviceBuffer::<f32>::zeroed(&stream, N)?;

    let hsaco_dir = std::env::var_os("FE2O3_HSACO_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let module = context.load_module_from_file(hsaco_dir.join("raw_output_shift.hsaco"))?;

    launch! {
        kernel: raw_output_shift,
        stream: stream,
        module: module,
        config: LaunchConfig::for_num_elems((N - 1) as u32),
        args: [slice(x_dev), slice_mut(out_dev)]
    }?;

    let out_host = out_dev.to_host_vec(&stream)?;
    for i in 0..N {
        let expected = if i == 0 { 0.0 } else { x_host[i - 1] * 2.0 };
        assert!(
            (out_host[i] - expected).abs() < 1e-5,
            "mismatch at {i}: got {}, expected {expected}",
            out_host[i]
        );
    }

    println!("raw_output_shift passed for {N} elements");
    Ok(())
}
