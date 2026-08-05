use fe2o3_core::{DeviceBuffer, GpuContext, LaunchConfig};
use fe2o3_device::{DisjointSlice, kernel, thread};
use fe2o3_host::launch;
use std::path::PathBuf;

#[kernel]
pub fn shift(x: &[f32], mut out: DisjointSlice<f32>) {
    let source = thread::index_1d().offset(1);
    let idx = thread::index_1d();
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

    let x_host: Vec<f32> = (0..=N).map(|i| i as f32 * 0.25 - 8.0).collect();
    let x_dev = DeviceBuffer::from_host(&stream, &x_host)?;
    let out_dev = DeviceBuffer::<f32>::zeroed(&stream, N)?;

    let hsaco_dir = std::env::var_os("FE2O3_HSACO_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    // SAFETY: `shift.hsaco` is compiler-generated for the `shift` kernel in
    // this exact example. The subsequent launch remains independently unsafe.
    // This example requires that output to target this device and contain no init/fini kernels.
    let module = unsafe { context.load_module_from_file_unchecked(hsaco_dir.join("shift.hsaco")) }?;

    // SAFETY: `shift` expects two f32 slice ABIs; x has N + 1 readable
    // elements, out has N disjoint writable elements, and both live through sync.
    unsafe {
        launch! {
            kernel: shift,
            stream: stream,
            module: module,
            config: LaunchConfig::for_num_elems(N as u32),
            args: [slice(x_dev), slice_mut(out_dev)]
        }
    }?;

    let out_host = out_dev.to_host_vec(&stream)?;
    for i in 0..N {
        let expected = x_host[i + 1];
        assert!(
            (out_host[i] - expected).abs() < 1e-5,
            "mismatch at {i}: got {}, expected {expected}",
            out_host[i]
        );
    }

    println!("shift passed for {N} elements");
    Ok(())
}
