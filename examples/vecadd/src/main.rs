use fe2o3_core::{DeviceBuffer, GpuContext};
use fe2o3_device::{DisjointSlice, kernel, thread};

include!("vecadd_body.rs");

macro_rules! production_f32_add {
    ($lhs:expr, $rhs:expr) => {{ $lhs + $rhs }};
}

#[kernel(typed)]
pub fn vecadd(a: &[f32], b: &[f32], mut c: DisjointSlice<f32>) {
    vecadd_kernel_body!(thread, (), production_f32_add, a, b, c);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    const N: usize = 1024;

    let context = GpuContext::new(0)?;
    let stream = context.default_stream();

    let a_host: Vec<f32> = (0..N).map(|i| i as f32).collect();
    let b_host: Vec<f32> = (0..N).map(|i| (i * 2) as f32).collect();

    let a_dev = DeviceBuffer::from_host(&stream, &a_host)?;
    let b_dev = DeviceBuffer::from_host(&stream, &b_host)?;
    let mut c_dev = DeviceBuffer::<f32>::zeroed(&stream, N)?;

    let kernel = vecadd_gpu::Kernel::load(&context)?;
    kernel
        .prepare(&a_dev, &b_dev, &mut c_dev)?
        .launch(&stream)?;

    let c_host = c_dev.to_host_vec(&stream)?;
    for i in 0..N {
        let expected = a_host[i] + b_host[i];
        assert!(
            (c_host[i] - expected).abs() < 1e-5,
            "mismatch at {i}: expected {expected}, got {}",
            c_host[i]
        );
    }

    println!("vecadd passed for {N} elements");
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fe2o3_core::{DeviceBuffer, GpuContext, Stream};

    const KERNEL_SOURCE: &str = include_str!("main.rs");
    const SHARED_BODY: &str = include_str!("vecadd_body.rs");

    #[allow(dead_code)]
    fn generated_public_api_typechecks<'loaded, 'allocation>(
        context: &Arc<GpuContext>,
        kernel: &'loaded super::vecadd_gpu::Kernel,
        a: &'allocation DeviceBuffer<f32>,
        b: &'allocation DeviceBuffer<f32>,
        c: &'allocation mut DeviceBuffer<f32>,
        prepared: super::vecadd_gpu::Prepared<'loaded, 'allocation>,
        stream: &Stream,
    ) {
        let _: Result<super::vecadd_gpu::Kernel, _> = super::vecadd_gpu::Kernel::load(context);
        let _: Result<super::vecadd_gpu::Prepared<'loaded, 'allocation>, _> =
            kernel.prepare(a, b, c);
        let _: Result<(), _> = prepared.launch(stream);
    }

    #[test]
    fn real_kernel_expands_the_shared_body() {
        assert!(KERNEL_SOURCE.contains("include!(\"vecadd_body.rs\")"));
        assert!(
            KERNEL_SOURCE.contains("vecadd_kernel_body!(thread, (), production_f32_add, a, b, c)")
        );
        assert!(KERNEL_SOURCE.contains("macro_rules! production_f32_add"));
        assert!(KERNEL_SOURCE.contains("$lhs + $rhs"));
    }

    #[test]
    fn example_uses_only_the_generated_typed_launch_api() {
        let production_source = KERNEL_SOURCE
            .split("#[cfg(test)]")
            .next()
            .expect("example has production source");

        for required in [
            "#[kernel(typed)]",
            "vecadd_gpu::Kernel::load(&context)",
            ".prepare(&a_dev, &b_dev, &mut c_dev)",
            ".launch(&stream)",
        ] {
            assert!(production_source.contains(required), "missing `{required}`");
        }

        for forbidden in [
            "#[kernel]",
            "FE2O3_HSACO_DIR",
            "load_module_from_file",
            "launch!",
            "LaunchConfig",
            "unsafe",
        ] {
            assert!(
                !production_source.contains(forbidden),
                "typed example retained `{forbidden}`"
            );
        }
    }

    #[test]
    fn shared_body_retains_the_verified_memory_shape() {
        for operation in [
            "let idx = $thread::index_1d",
            "let i = idx.get()",
            "if let Some(out) = $output.get_mut(idx)",
            "*out = $add!($a[i], $b[i])",
        ] {
            assert!(SHARED_BODY.contains(operation), "missing `{operation}`");
        }

        let guard = SHARED_BODY.find("if let Some(out)").unwrap();
        let first_input_access = SHARED_BODY.find("$a[i]").unwrap();
        assert!(guard < first_input_access, "input access escaped the guard");
    }
}
