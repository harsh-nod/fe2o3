use fe2o3_core::{DeviceBuffer, GpuContext, LaunchConfig};
use fe2o3_device::{DisjointSlice, kernel, thread};
use fe2o3_host::launch;
use std::path::PathBuf;

#[kernel]
pub fn raw_disjoint_inplace_shift(x: &[f32], mut out: DisjointSlice<f32>) {
    let idx = thread::index_1d();
    let source = idx.get();
    let target = source + 1;
    if source < x.len() {
        if let Some(value) = out.get_mut_at(target) {
            *value = *value + x[source];
        }
    }
}

fn main() -> fe2o3_core::Result<()> {
    const N: usize = 1024;

    let context = GpuContext::new(0)?;
    let stream = context.default_stream();

    let x_host: Vec<f32> = (0..(N - 1)).map(|i| i as f32 * 0.0625 - 1.0).collect();
    let out_initial: Vec<f32> = (0..N).map(|i| 10.0 + i as f32 * 0.25).collect();

    let x_dev = DeviceBuffer::from_host(&stream, &x_host)?;
    let out_dev = DeviceBuffer::from_host(&stream, &out_initial)?;

    let hsaco_dir = std::env::var_os("FE2O3_HSACO_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    // SAFETY: `raw_disjoint_inplace_shift.hsaco` is compiler-generated for the
    // identically named kernel here. The subsequent launch remains independently unsafe.
    // This example requires that output to target this device and contain no init/fini kernels.
    let module = unsafe {
        context.load_module_from_file_unchecked(hsaco_dir.join("raw_disjoint_inplace_shift.hsaco"))
    }?;

    // SAFETY: `raw_disjoint_inplace_shift` expects two f32 slice ABIs; x has
    // N - 1 elements, out has N, both live through sync, and each in-bounds
    // thread writes a unique i + 1.
    unsafe {
        launch! {
            kernel: raw_disjoint_inplace_shift,
            stream: stream,
            module: module,
            config: LaunchConfig::for_num_elems((N - 1) as u32),
            args: [slice(x_dev), slice_mut(out_dev)]
        }
    }?;

    let out_host = out_dev.to_host_vec(&stream)?;
    for i in 0..N {
        let expected = if i == 0 {
            out_initial[i]
        } else {
            out_initial[i] + x_host[i - 1]
        };
        assert!(
            (out_host[i] - expected).abs() < 1e-5,
            "mismatch at {i}: got {}, expected {expected}",
            out_host[i]
        );
    }

    println!("raw_disjoint_inplace_shift passed for {N} elements");
    Ok(())
}
