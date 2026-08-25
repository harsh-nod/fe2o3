#[cfg(feature = "qualification-embedded-vecadd-test-only")]
use fe2o3_core::{DeviceBuffer, Event, GpuContext};
use fe2o3_device::{DisjointSlice, kernel, thread};

include!("vecadd_body.rs");

macro_rules! production_f32_add {
    ($lhs:expr, $rhs:expr) => {{ $lhs + $rhs }};
}

#[cfg_attr(
    not(feature = "qualification-embedded-vecadd-test-only"),
    kernel(
        typed,
        namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
    )
)]
#[cfg_attr(
    feature = "qualification-embedded-vecadd-test-only",
    kernel(
        typed,
        qualification_worker_v2,
        namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
    )
)]
pub fn vecadd(a: &[f32], b: &[f32], mut c: DisjointSlice<f32>) {
    vecadd_kernel_body!(thread, (), production_f32_add, a, b, c);
}

#[cfg(not(feature = "qualification-embedded-vecadd-test-only"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "the production Worker V3 application verifier is not wired for fe2o3-vecadd",
    )
    .into())
}

#[cfg(feature = "qualification-embedded-vecadd-test-only")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    const DEFAULT_N: usize = 1024;

    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let (n, warmups, samples, launches_per_sample) = match arguments.as_slice() {
        [] => (DEFAULT_N, 0, 0, 1),
        [mode, n, warmups, samples, launches] if mode == "--benchmark" => (
            n.parse()?,
            warmups.parse()?,
            samples.parse()?,
            launches.parse()?,
        ),
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "usage: fe2o3-vecadd [--benchmark N WARMUPS SAMPLES LAUNCHES_PER_SAMPLE]",
            )
            .into());
        }
    };
    if n == 0 || n > u32::MAX as usize || (samples != 0 && launches_per_sample == 0) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "VecAdd requires 1 <= N <= u32::MAX and a nonzero timed batch",
        )
        .into());
    }
    let context = GpuContext::new(0)?;
    let stream = context.default_stream();

    let a_host: Vec<f32> = (0..n).map(|i| i as f32).collect();
    let b_host: Vec<f32> = (0..n).map(|i| i as f32 * 2.0).collect();

    let a_dev = DeviceBuffer::from_host(&stream, &a_host)?;
    let b_dev = DeviceBuffer::from_host(&stream, &b_host)?;
    let mut c_dev = DeviceBuffer::<f32>::zeroed(&stream, n)?;

    let kernel = vecadd_gpu::Kernel::load(&context)?;
    for _ in 0..warmups {
        kernel
            .prepare(&a_dev, &b_dev, &mut c_dev)?
            .launch(&stream)?;
    }

    if samples == 0 {
        kernel
            .prepare(&a_dev, &b_dev, &mut c_dev)?
            .launch(&stream)?;
    } else {
        stream.synchronize()?;
        let mut start = Event::new(&context)?;
        let mut stop = Event::new(&context)?;
        let mut microseconds = Vec::with_capacity(samples);
        for _ in 0..samples {
            start.record(&stream)?;
            for _ in 0..launches_per_sample {
                kernel
                    .prepare(&a_dev, &b_dev, &mut c_dev)?
                    .launch(&stream)?;
            }
            stop.record(&stream)?;
            stop.synchronize()?;
            microseconds
                .push(stop.elapsed_time_ms_since(&start)? * 1_000.0 / launches_per_sample as f32);
        }
        microseconds.sort_by(f32::total_cmp);
        let median_us = percentile(&microseconds, 50);
        let bytes_per_launch = (3 * n * std::mem::size_of::<f32>()) as f64;
        let bandwidth_gb_s = bytes_per_launch / f64::from(median_us) / 1_000.0;
        println!(
            "fe2o3 vecadd dispatch path: n={n} samples={samples} batch={launches_per_sample} event_interval_median_us={median_us:.3} p10_us={:.3} p90_us={:.3} effective_bandwidth_gb_s={bandwidth_gb_s:.2}",
            percentile(&microseconds, 10),
            percentile(&microseconds, 90),
        );
    }

    let c_host = c_dev.to_host_vec(&stream)?;
    for i in 0..n {
        let expected = a_host[i] + b_host[i];
        assert!(
            (c_host[i] - expected).abs() < 1e-5,
            "mismatch at {i}: expected {expected}, got {}",
            c_host[i]
        );
    }

    println!("vecadd passed for {n} elements");
    Ok(())
}

#[cfg(feature = "qualification-embedded-vecadd-test-only")]
fn percentile(sorted: &[f32], percentile: usize) -> f32 {
    sorted[(sorted.len() - 1) * percentile / 100]
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "qualification-embedded-vecadd-test-only")]
    use std::sync::Arc;

    use fe2o3_core::DeviceBuffer;
    #[cfg(feature = "qualification-embedded-vecadd-test-only")]
    use fe2o3_core::{GpuContext, Stream};

    const KERNEL_SOURCE: &str = include_str!("main.rs");
    const SHARED_BODY: &str = include_str!("vecadd_body.rs");

    #[cfg(not(feature = "qualification-embedded-vecadd-test-only"))]
    #[allow(dead_code)]
    fn generated_v3_arguments_typecheck<'allocation>(
        observed: &fe2o3_host::ObservedContext,
        a: &'allocation DeviceBuffer<f32>,
        b: &'allocation DeviceBuffer<f32>,
        c: &'allocation mut DeviceBuffer<f32>,
    ) {
        let a = fe2o3_host::__generated::GeneratedReadDeviceSlice::new(observed, a).unwrap();
        let b = fe2o3_host::__generated::GeneratedReadDeviceSlice::new(observed, b).unwrap();
        let c = fe2o3_host::__generated::GeneratedReadWriteDeviceSlice::new(observed, c).unwrap();
        let _arguments: super::vecadd_gpu::Arguments<'allocation> =
            super::vecadd_gpu::Arguments::new(a, b, c);
    }

    #[cfg(feature = "qualification-embedded-vecadd-test-only")]
    #[allow(dead_code)]
    fn qualification_embedded_api_typechecks<'loaded, 'allocation>(
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
    fn example_separates_production_v3_from_the_embedded_qualification_oracle() {
        let production_source = KERNEL_SOURCE
            .split("#[cfg(test)]")
            .next()
            .expect("example has production source");

        for required in [
            "not(feature = \"qualification-embedded-vecadd-test-only\")",
            "feature = \"qualification-embedded-vecadd-test-only\"",
            "qualification_worker_v2",
            "production Worker V3 application verifier",
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
            "FE2O3_CODEGEN_PIPELINE",
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
