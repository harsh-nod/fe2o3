use fe2o3_core::{DeviceBuffer, GpuContext, LaunchConfig};
use fe2o3_device::{DisjointSlice, kernel, thread};
use fe2o3_host::launch;
use std::path::PathBuf;

#[kernel]
pub fn stencil(x: &[f32], mut out: DisjointSlice<f32>) {
    let idx = thread::index_1d();
    let left = idx.offset_signed(-1);
    let center = idx.get();
    let right = idx.offset(1);
    if left < x.len() && right < x.len() {
        if let Some(value) = out.get_mut(idx) {
            *value = 0.25 * x[left] + 0.5 * x[center] + 0.25 * x[right];
        }
    }
}

fn main() -> fe2o3_core::Result<()> {
    const N: usize = 1024;

    let context = GpuContext::new(0)?;
    let stream = context.default_stream();

    let x_host: Vec<f32> = (0..N).map(|i| (i as f32 * 0.125).sin()).collect();
    let x_dev = DeviceBuffer::from_host(&stream, &x_host)?;
    let out_dev = DeviceBuffer::<f32>::zeroed(&stream, N)?;

    let hsaco_dir = std::env::var_os("FE2O3_HSACO_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let module = context.load_module_from_file(hsaco_dir.join("stencil.hsaco"))?;

    // SAFETY: `stencil` expects two f32 slice ABIs; the buffers are distinct
    // N-element allocations that live through sync, and the kernel guards both
    // neighboring reads.
    unsafe {
        launch! {
            kernel: stencil,
            stream: stream,
            module: module,
            config: LaunchConfig::for_num_elems(N as u32),
            args: [slice(x_dev), slice_mut(out_dev)]
        }
    }?;

    let out_host = out_dev.to_host_vec(&stream)?;
    for i in 0..N {
        let expected = if i == 0 || i + 1 == N {
            0.0
        } else {
            0.25 * x_host[i - 1] + 0.5 * x_host[i] + 0.25 * x_host[i + 1]
        };
        assert!(
            (out_host[i] - expected).abs() < 1e-5,
            "mismatch at {i}: got {}, expected {expected}",
            out_host[i]
        );
    }

    println!("stencil passed for {N} elements");
    Ok(())
}
