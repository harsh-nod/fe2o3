//! Matched R26 persistent-HBM transform benchmark for the exact gfx942 fixture.

#[cfg(not(feature = "hardware-qualification"))]
fn main() {
    eprintln!("enable the fe2o3-runtime `hardware-qualification` feature");
    std::process::exit(2);
}

#[cfg(feature = "hardware-qualification")]
mod enabled {
    use std::error::Error;
    use std::fmt::{Debug, Write as _};
    use std::time::{Duration, Instant};

    use fe2o3_runtime::qualification_gfx942_inplace_transform_v1::{
        GFX942_INPLACE_TRANSFORM_INPUT_A_SHA256_V1, GFX942_INPLACE_TRANSFORM_INPUT_B_SHA256_V1,
        GFX942_INPLACE_TRANSFORM_OUTPUT_A_SHA256_V1, GFX942_INPLACE_TRANSFORM_OUTPUT_B_SHA256_V1,
        GFX942_INPLACE_TRANSFORM_QUALIFICATION_BUFFER_BYTES_V1,
        GFX942_INPLACE_TRANSFORM_QUALIFICATION_ELEMENTS_V1,
        GFX942_INPLACE_TRANSFORM_QUALIFICATION_GEOMETRY_V1,
        GFX942_INPLACE_TRANSFORM_QUALIFICATION_KERNEL_V1,
        Gfx942InplaceTransformQualificationArgumentsV1, Gfx942InplaceTransformQualificationInputV1,
        admit_gfx942_inplace_transform_qualification_v1,
        validate_gfx942_inplace_transform_output_v1,
    };
    use fe2o3_runtime::{
        KfdRuntimeBackendV1, KfdRuntimeLaunchDataPathV1, RuntimeAccessV1, RuntimeAllocationIdV1,
        RuntimeContextV1, RuntimeCopyV1, RuntimeMemoryKindV1, RuntimeMemoryRegionV1, RuntimePollV1,
        RuntimeStreamIdV1, RuntimeSubmissionV1, TypedRuntimeKernelV1,
    };

    const USAGE: &str = "usage: gfx942-runtime-r26-inplace-benchmark <unique-id>";
    const WARMUPS: usize = 10;
    const SAMPLES: usize = 30;
    const ITERATIONS_PER_SAMPLE: usize = 10;
    const COMPLETION_TIMEOUT: Duration = Duration::from_secs(30);
    const COMPLETION_WAIT_SLICE: Duration = Duration::from_micros(50);
    const DEVICE_ALIGNMENT: u64 = 4096;

    type BenchmarkResult<T> = Result<T, Box<dyn Error>>;

    fn facade_error(error: impl Debug) -> Box<dyn Error> {
        format!("{error:?}").into()
    }

    fn shutdown_context_v1(context: RuntimeContextV1<KfdRuntimeBackendV1>) -> BenchmarkResult<()> {
        let mut backend = match context.shutdown() {
            Ok(backend) => backend,
            Err(failure) => {
                let detail = format!("{failure:?}");
                let context = failure.into_context();
                std::mem::forget(context);
                return Err(format!(
                    "R26 context cleanup failed; exact custody is retained until process teardown: {detail}"
                )
                .into());
            }
        };
        if let Err(error) = backend.shutdown_native_v1() {
            let detail = format!("{error:?}");
            std::mem::forget(backend);
            return Err(format!(
                "R26 native cleanup failed; exact custody is retained until process teardown: {detail}"
            )
            .into());
        }
        Ok(())
    }

    fn combine_primary_and_cleanup_v1(
        primary: Box<dyn Error>,
        cleanup: BenchmarkResult<()>,
    ) -> Box<dyn Error> {
        match cleanup {
            Ok(()) => primary,
            Err(cleanup) => {
                format!("R26 operation failed: {primary}; cleanup also failed: {cleanup}").into()
            }
        }
    }

    fn parse_unique_id(value: &str) -> BenchmarkResult<u64> {
        let unique_id = if let Some(hex) = value.strip_prefix("0x") {
            u64::from_str_radix(hex, 16)?
        } else {
            value.parse()?
        };
        if unique_id == 0 {
            return Err("unique ID must be nonzero".into());
        }
        Ok(unique_id)
    }

    const fn region(
        allocation: RuntimeAllocationIdV1,
        access: RuntimeAccessV1,
    ) -> RuntimeMemoryRegionV1 {
        RuntimeMemoryRegionV1 {
            allocation,
            access,
            byte_offset: 0,
            byte_len: GFX942_INPLACE_TRANSFORM_QUALIFICATION_BUFFER_BYTES_V1 as u64,
        }
    }

    fn wait_for_copy_v1(
        context: &mut RuntimeContextV1<KfdRuntimeBackendV1>,
        stream: RuntimeStreamIdV1,
        submission: &mut RuntimeSubmissionV1<RuntimeCopyV1>,
        deadline: Instant,
    ) -> BenchmarkResult<()> {
        context.flush_stream(stream).map_err(facade_error)?;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("R26 copy completion timed out".into());
            }
            match context
                .wait(submission, remaining.min(COMPLETION_WAIT_SLICE))
                .map_err(facade_error)?
            {
                RuntimePollV1::Succeeded => return Ok(()),
                RuntimePollV1::Pending => context.flush_stream(stream).map_err(facade_error)?,
                RuntimePollV1::Failed { code } => {
                    return Err(format!("R26 copy failed with code {code}").into());
                }
            }
        }
    }

    fn run_copy_v1(
        context: &mut RuntimeContextV1<KfdRuntimeBackendV1>,
        stream: RuntimeStreamIdV1,
        source: RuntimeMemoryRegionV1,
        destination: RuntimeMemoryRegionV1,
    ) -> BenchmarkResult<(u128, RuntimeSubmissionV1<RuntimeCopyV1>)> {
        let started = Instant::now();
        let deadline = started
            .checked_add(COMPLETION_TIMEOUT)
            .ok_or("R26 copy deadline overflow")?;
        let mut submission = context
            .copy_async(stream, source, destination, &[])
            .map_err(facade_error)?;
        wait_for_copy_v1(context, stream, &mut submission, deadline)?;
        let elapsed = started.elapsed().as_nanos();
        if elapsed == 0 {
            return Err("R26 copy duration was zero".into());
        }
        Ok((elapsed, submission))
    }

    #[derive(Clone, Copy, Debug)]
    struct IterationTimingV1 {
        h2d: u128,
        compute: u128,
        d2h: u128,
        e2e: u128,
        promotion: u128,
    }

    #[derive(Clone, Copy, Debug, Default)]
    struct SampleAccumulatorV1 {
        h2d: u128,
        compute: u128,
        d2h: u128,
        e2e: u128,
        promotion: u128,
    }

    impl SampleAccumulatorV1 {
        fn add(&mut self, timing: IterationTimingV1) -> BenchmarkResult<()> {
            self.h2d = self
                .h2d
                .checked_add(timing.h2d)
                .ok_or("H2D timing overflow")?;
            self.compute = self
                .compute
                .checked_add(timing.compute)
                .ok_or("compute timing overflow")?;
            self.d2h = self
                .d2h
                .checked_add(timing.d2h)
                .ok_or("D2H timing overflow")?;
            self.e2e = self
                .e2e
                .checked_add(timing.e2e)
                .ok_or("E2E timing overflow")?;
            self.promotion = self
                .promotion
                .checked_add(timing.promotion)
                .ok_or("promotion timing overflow")?;
            Ok(())
        }

        fn average(self) -> BenchmarkResult<IterationTimingV1> {
            let divisor = ITERATIONS_PER_SAMPLE as u128;
            let timing = IterationTimingV1 {
                h2d: self.h2d / divisor,
                compute: self.compute / divisor,
                d2h: self.d2h / divisor,
                e2e: self.e2e / divisor,
                promotion: self.promotion / divisor,
            };
            if [
                timing.h2d,
                timing.compute,
                timing.d2h,
                timing.e2e,
                timing.promotion,
            ]
            .contains(&0)
            {
                return Err("R26 sample contains a zero average duration".into());
            }
            Ok(timing)
        }
    }

    struct QualifiedRunV1 {
        context: RuntimeContextV1<KfdRuntimeBackendV1>,
        stream: RuntimeStreamIdV1,
        kernel: TypedRuntimeKernelV1<Gfx942InplaceTransformQualificationArgumentsV1>,
        upload: RuntimeAllocationIdV1,
        device_buffer: RuntimeAllocationIdV1,
        download: RuntimeAllocationIdV1,
        arguments: Gfx942InplaceTransformQualificationArgumentsV1,
        inputs: [Vec<u8>; 2],
        observed: Vec<u8>,
        last_promotion_ordinal: Option<u64>,
    }

    impl QualifiedRunV1 {
        fn open(unique_id: u64) -> BenchmarkResult<Self> {
            let admitted = admit_gfx942_inplace_transform_qualification_v1()?;
            let (inputs, _) = admitted.host_buffers()?.into_parts();
            let backend =
                KfdRuntimeBackendV1::open_gfx942_inplace_transform_qualification_v1(unique_id)?;
            let mut context = RuntimeContextV1::open(backend).map_err(facade_error)?;
            let setup = (|| -> BenchmarkResult<_> {
                if context.devices().len() != 1 || context.devices()[0].target() != "gfx942:xnack-"
                {
                    return Err("R26 KFD backend did not enumerate one gfx942:xnack- device".into());
                }
                let device = context.devices()[0].id();
                let capabilities = context.execution_capabilities(device)?;
                if !capabilities.native_async_copy || !capabilities.memory_pool {
                    return Err(
                        "R26 KFD backend lacks persistent SDMA or memory-pool support".into(),
                    );
                }
                let stream = context.create_stream(device).map_err(facade_error)?;
                let module = context
                    .load_module(device, admitted.hsaco())
                    .map_err(facade_error)?;
                let kernel = context
                    .resolve_kernel::<Gfx942InplaceTransformQualificationArgumentsV1>(
                        module,
                        GFX942_INPLACE_TRANSFORM_QUALIFICATION_KERNEL_V1,
                    )
                    .map_err(facade_error)?;
                let byte_len = GFX942_INPLACE_TRANSFORM_QUALIFICATION_BUFFER_BYTES_V1 as u64;
                let upload = context
                    .allocate(
                        device,
                        RuntimeMemoryKindV1::HostVisible,
                        byte_len,
                        DEVICE_ALIGNMENT,
                    )
                    .map_err(facade_error)?;
                let device_buffer = context
                    .allocate(
                        device,
                        RuntimeMemoryKindV1::DeviceLocal,
                        byte_len,
                        DEVICE_ALIGNMENT,
                    )
                    .map_err(facade_error)?;
                let download = context
                    .allocate(
                        device,
                        RuntimeMemoryKindV1::HostVisible,
                        byte_len,
                        DEVICE_ALIGNMENT,
                    )
                    .map_err(facade_error)?;
                Ok((stream, kernel, upload, device_buffer, download))
            })();
            let (stream, kernel, upload, device_buffer, download) = match setup {
                Ok(setup) => setup,
                Err(error) => {
                    return Err(combine_primary_and_cleanup_v1(
                        error,
                        shutdown_context_v1(context),
                    ));
                }
            };
            Ok(Self {
                context,
                stream,
                kernel,
                upload,
                device_buffer,
                download,
                arguments: Gfx942InplaceTransformQualificationArgumentsV1::new(device_buffer),
                inputs,
                observed: vec![0; GFX942_INPLACE_TRANSFORM_QUALIFICATION_BUFFER_BYTES_V1],
                last_promotion_ordinal: None,
            })
        }

        fn run_compute_v1(
            &mut self,
        ) -> BenchmarkResult<(
            u128,
            RuntimeSubmissionV1<Gfx942InplaceTransformQualificationArgumentsV1>,
        )> {
            let started = Instant::now();
            let deadline = started
                .checked_add(COMPLETION_TIMEOUT)
                .ok_or("R26 compute deadline overflow")?;
            let mut submission = self
                .context
                .launch(
                    self.stream,
                    &self.kernel,
                    &self.arguments,
                    GFX942_INPLACE_TRANSFORM_QUALIFICATION_GEOMETRY_V1,
                    &[],
                )
                .map_err(facade_error)?;
            self.context
                .flush_stream(self.stream)
                .map_err(facade_error)?;
            loop {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err("R26 compute completion timed out".into());
                }
                match self
                    .context
                    .wait(&mut submission, remaining.min(COMPLETION_WAIT_SLICE))
                    .map_err(facade_error)?
                {
                    RuntimePollV1::Succeeded => break,
                    RuntimePollV1::Pending => self
                        .context
                        .flush_stream(self.stream)
                        .map_err(facade_error)?,
                    RuntimePollV1::Failed { code } => {
                        return Err(format!("R26 compute failed with code {code}").into());
                    }
                }
            }
            let elapsed = started.elapsed().as_nanos();
            if elapsed == 0 {
                return Err("R26 compute duration was zero".into());
            }
            Ok((elapsed, submission))
        }

        fn observe_promotion_v1(&mut self) -> BenchmarkResult<u128> {
            let performance = self
                .context
                .backend()
                .last_launch_performance_v1()
                .ok_or("R26 completed launch has no performance observation")?;
            if performance.data_path() != KfdRuntimeLaunchDataPathV1::PersistentDeviceReused
                || performance.user_data_materializations() != 0
            {
                return Err("R26 launch did not reuse persistent HBM exactly".into());
            }
            let promotion = performance
                .ready_promotion()
                .ok_or("R26 launch did not consume an H2D-ready promotion")?;
            if promotion.authenticated_bytes()
                != GFX942_INPLACE_TRANSFORM_QUALIFICATION_BUFFER_BYTES_V1 as u64
                || promotion.content_ordinal() != 0
            {
                return Err("R26 ready-promotion coordinates changed".into());
            }
            if let Some(previous) = self.last_promotion_ordinal
                && promotion.ordinal() != previous.checked_add(1).ok_or("promotion overflow")?
            {
                return Err("R26 ready-promotion ordinal is not contiguous".into());
            }
            self.last_promotion_ordinal = Some(promotion.ordinal());
            let authentication = promotion.authentication().as_nanos();
            if authentication == 0 {
                return Err("R26 promotion duration was zero".into());
            }
            Ok(authentication)
        }

        fn iteration(&mut self, global_iteration: u64) -> BenchmarkResult<IterationTimingV1> {
            let input =
                Gfx942InplaceTransformQualificationInputV1::for_global_iteration(global_iteration);
            let input_bytes = match input {
                Gfx942InplaceTransformQualificationInputV1::A => &self.inputs[0],
                Gfx942InplaceTransformQualificationInputV1::B => &self.inputs[1],
            };
            self.context
                .write_allocation(self.upload, 0, input_bytes)
                .map_err(facade_error)?;
            let e2e_started = Instant::now();
            let (h2d, h2d_submission) = run_copy_v1(
                &mut self.context,
                self.stream,
                region(self.upload, RuntimeAccessV1::Read),
                region(self.device_buffer, RuntimeAccessV1::Write),
            )?;
            let (compute, compute_submission) = self.run_compute_v1()?;
            let (d2h, d2h_submission) = run_copy_v1(
                &mut self.context,
                self.stream,
                region(self.device_buffer, RuntimeAccessV1::Read),
                region(self.download, RuntimeAccessV1::Write),
            )?;
            let e2e = e2e_started.elapsed().as_nanos();
            let promotion = self.observe_promotion_v1()?;
            self.context
                .read_allocation(self.download, 0, &mut self.observed)
                .map_err(facade_error)?;
            validate_gfx942_inplace_transform_output_v1(input, &self.observed)?;
            self.context
                .release_submission(d2h_submission)
                .map_err(facade_error)?;
            self.context
                .release_submission(compute_submission)
                .map_err(facade_error)?;
            self.context
                .release_submission(h2d_submission)
                .map_err(facade_error)?;
            if e2e == 0 {
                return Err("R26 E2E duration was zero".into());
            }
            Ok(IterationTimingV1 {
                h2d,
                compute,
                d2h,
                e2e,
                promotion,
            })
        }

        fn shutdown(self) -> BenchmarkResult<()> {
            shutdown_context_v1(self.context)
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct SummaryV1 {
        min: u128,
        mean: u128,
        max: u128,
        p50: u128,
        p95: u128,
    }

    fn summarize(values: &[u128]) -> BenchmarkResult<SummaryV1> {
        if values.len() != SAMPLES || values.contains(&0) {
            return Err("R26 series is incomplete or contains zero".into());
        }
        let mut ordered = values.to_vec();
        ordered.sort_unstable();
        let sum = ordered
            .iter()
            .try_fold(0_u128, |sum, value| sum.checked_add(*value))
            .ok_or("R26 summary sum overflow")?;
        let nearest_rank = |numerator: usize, denominator: usize| {
            let rank = ordered
                .len()
                .checked_mul(numerator)
                .and_then(|value| value.checked_add(denominator - 1))
                .expect("fixed percentile rank is representable")
                / denominator;
            ordered[rank - 1]
        };
        Ok(SummaryV1 {
            min: ordered[0],
            mean: sum / ordered.len() as u128,
            max: *ordered.last().expect("validated nonempty series"),
            p50: nearest_rank(1, 2),
            p95: nearest_rank(19, 20),
        })
    }

    fn raw(values: &[u128]) -> String {
        values
            .iter()
            .map(u128::to_string)
            .collect::<Vec<_>>()
            .join(",")
    }

    fn append_phase(row: &mut String, phase: &str, values: &[u128]) -> BenchmarkResult<()> {
        let summary = summarize(values)?;
        write!(
            row,
            " {phase}_samples_ns={} {phase}_min_ns={} {phase}_mean_ns={} {phase}_max_ns={} {phase}_p50_ns={} {phase}_p95_ns={}",
            raw(values),
            summary.min,
            summary.mean,
            summary.max,
            summary.p50,
            summary.p95,
        )?;
        Ok(())
    }

    fn hex(bytes: [u8; 32]) -> String {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut output = String::with_capacity(64);
        for byte in bytes {
            output.push(DIGITS[(byte >> 4) as usize] as char);
            output.push(DIGITS[(byte & 0x0f) as usize] as char);
        }
        output
    }

    pub fn run() -> BenchmarkResult<()> {
        let arguments = std::env::args().skip(1).collect::<Vec<_>>();
        if arguments.len() != 1 {
            return Err(USAGE.into());
        }
        let unique_id = parse_unique_id(&arguments[0])?;
        let mut run = QualifiedRunV1::open(unique_id)?;
        let measurement = (|| -> BenchmarkResult<_> {
            let mut global_iteration = 0_u64;
            for _ in 0..WARMUPS {
                run.iteration(global_iteration)?;
                global_iteration = global_iteration
                    .checked_add(1)
                    .ok_or("R26 iteration overflow")?;
            }

            let mut h2d = Vec::with_capacity(SAMPLES);
            let mut compute = Vec::with_capacity(SAMPLES);
            let mut d2h = Vec::with_capacity(SAMPLES);
            let mut e2e = Vec::with_capacity(SAMPLES);
            let mut promotion = Vec::with_capacity(SAMPLES);
            for _ in 0..SAMPLES {
                let mut sample = SampleAccumulatorV1::default();
                for _ in 0..ITERATIONS_PER_SAMPLE {
                    sample.add(run.iteration(global_iteration)?)?;
                    global_iteration = global_iteration
                        .checked_add(1)
                        .ok_or("R26 iteration overflow")?;
                }
                let sample = sample.average()?;
                h2d.push(sample.h2d);
                compute.push(sample.compute);
                d2h.push(sample.d2h);
                e2e.push(sample.e2e);
                promotion.push(sample.promotion);
            }
            Ok((h2d, compute, d2h, e2e, promotion))
        })();
        let cleanup = run.shutdown();
        let (h2d, compute, d2h, e2e, promotion) = match measurement {
            Ok(series) => {
                cleanup?;
                series
            }
            Err(primary) => return Err(combine_primary_and_cleanup_v1(primary, cleanup)),
        };

        let validated_iterations = WARMUPS
            .checked_add(
                SAMPLES
                    .checked_mul(ITERATIONS_PER_SAMPLE)
                    .ok_or("R26 measured iteration overflow")?,
            )
            .ok_or("R26 total iteration overflow")?;
        let pattern_a_iterations = validated_iterations.div_ceil(2);
        let pattern_b_iterations = validated_iterations / 2;
        let mut row = format!(
            "backend=kfd schema=fe2o3.r26-inplace-benchmark.v1 device_index=0 unique_id={unique_id:016x} uuid=GPU-{unique_id:016x} target=gfx942:xnack- xnack=disabled kernel={GFX942_INPLACE_TRANSFORM_QUALIFICATION_KERNEL_V1} bytes={GFX942_INPLACE_TRANSFORM_QUALIFICATION_BUFFER_BYTES_V1} elements={GFX942_INPLACE_TRANSFORM_QUALIFICATION_ELEMENTS_V1} workgroup=256 warmups={WARMUPS} samples={SAMPLES} iterations_per_sample={ITERATIONS_PER_SAMPLE} sample_value=integer-average-ns-over-10-iterations trimming=none input_pattern=alternating-full-a-b pattern_start=a validation=every-element-every-iteration validated_iterations={validated_iterations} pattern_a_iterations={pattern_a_iterations} pattern_b_iterations={pattern_b_iterations} timing=host-monotonic interphase_control=e2e-h2d-compute-d2h promotion=full-h2d-to-compute-ready data_path=persistent-device-reused user_data_materializations=0 input_a_sha256={} output_a_sha256={} input_b_sha256={} output_b_sha256={}",
            hex(GFX942_INPLACE_TRANSFORM_INPUT_A_SHA256_V1),
            hex(GFX942_INPLACE_TRANSFORM_OUTPUT_A_SHA256_V1),
            hex(GFX942_INPLACE_TRANSFORM_INPUT_B_SHA256_V1),
            hex(GFX942_INPLACE_TRANSFORM_OUTPUT_B_SHA256_V1),
        );
        append_phase(&mut row, "h2d", &h2d)?;
        append_phase(&mut row, "compute", &compute)?;
        append_phase(&mut row, "d2h", &d2h)?;
        append_phase(&mut row, "e2e", &e2e)?;
        append_phase(&mut row, "promotion", &promotion)?;
        println!("{row}");
        Ok(())
    }
}

#[cfg(feature = "hardware-qualification")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    enabled::run()
}
