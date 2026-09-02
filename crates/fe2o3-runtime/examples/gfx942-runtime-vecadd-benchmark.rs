//! Exact-fixture direct-KFD qualification and timing harness.

#[cfg(not(feature = "hardware-qualification"))]
fn main() {
    eprintln!("enable the fe2o3-runtime `hardware-qualification` feature");
    std::process::exit(2);
}

#[cfg(feature = "hardware-qualification")]
mod enabled {
    use std::env;
    use std::error::Error;
    use std::time::{Duration, Instant};

    use fe2o3_runtime::qualification_gfx942_vecadd_v1::{
        GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1,
        GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1, GFX942_VECADD_QUALIFICATION_ELEMENTS_V1,
        GFX942_VECADD_QUALIFICATION_GEOMETRY_V1, GFX942_VECADD_QUALIFICATION_KERNEL_V1,
        Gfx942VecaddQualificationArgumentsV1, admit_gfx942_vecadd_qualification_v1,
    };
    use fe2o3_runtime::{
        KfdRuntimeBackendV1, KfdRuntimeLaunchPerformanceV1, RuntimeAllocationIdV1,
        RuntimeContextV1, RuntimeMemoryKindV1, RuntimeModuleIdV1, RuntimePollV1, RuntimeStreamIdV1,
        TypedRuntimeKernelV1,
    };

    const USAGE: &str =
        "usage: gfx942-runtime-vecadd-benchmark UNIQUE_ID WARMUPS SAMPLES LAUNCHES_PER_SAMPLE";
    const COMPLETION_TIMEOUT: Duration = Duration::from_secs(30);

    fn parse_unique_id(text: &str) -> Result<u64, String> {
        let value = text
            .strip_prefix("0x")
            .map_or_else(|| text.parse::<u64>(), |hex| u64::from_str_radix(hex, 16));
        value
            .map_err(|error| format!("invalid unique ID `{text}`: {error}"))
            .and_then(|value| {
                (value != 0)
                    .then_some(value)
                    .ok_or_else(|| "unique ID must be nonzero".to_owned())
            })
    }

    fn parse_positive(name: &str, text: &str) -> Result<usize, String> {
        text.parse::<usize>()
            .map_err(|error| format!("invalid {name} `{text}`: {error}"))
            .and_then(|value| {
                (value != 0)
                    .then_some(value)
                    .ok_or_else(|| format!("{name} must be nonzero"))
            })
    }

    fn backend_error(error: impl core::fmt::Debug) -> String {
        format!("{error:?}")
    }

    fn validate_output(observed: &[u8], expected: &[u8]) -> Result<(), String> {
        if observed.len() != expected.len() {
            return Err(format!(
                "output length mismatch: expected {}, observed {}",
                expected.len(),
                observed.len()
            ));
        }
        if let Some((index, (observed, expected))) = observed
            .chunks_exact(size_of::<f32>())
            .zip(expected.chunks_exact(size_of::<f32>()))
            .enumerate()
            .find(|(_, (observed, expected))| observed != expected)
        {
            let observed = f32::from_bits(u32::from_le_bytes(
                observed.try_into().expect("one observed f32"),
            ));
            let expected = f32::from_bits(u32::from_le_bytes(
                expected.try_into().expect("one expected f32"),
            ));
            return Err(format!(
                "output mismatch at element {index}: expected {expected:?}, observed {observed:?}"
            ));
        }
        Ok(())
    }

    fn percentile(sorted: &[f64], percentile: usize) -> f64 {
        sorted[(sorted.len() - 1) * percentile / 100]
    }

    fn report(metric: &str, samples: &[f64], launches_per_sample: usize) {
        let mut sorted = samples.to_vec();
        sorted.sort_by(f64::total_cmp);
        let mean = sorted.iter().sum::<f64>() / sorted.len() as f64;
        println!(
            "backend=kfd metric={} n={} samples={} launches_per_sample={} min_us={:.3} p50_us={:.3} mean_us={:.3} p90_us={:.3} max_us={:.3}",
            metric,
            GFX942_VECADD_QUALIFICATION_ELEMENTS_V1,
            sorted.len(),
            launches_per_sample,
            sorted[0],
            percentile(&sorted, 50),
            mean,
            percentile(&sorted, 90),
            sorted[sorted.len() - 1],
        );
    }

    #[derive(Clone, Copy, Debug, Default)]
    struct IterationTimingV1 {
        total: Duration,
        output_reset: Duration,
        synchronized_launch_wait: Duration,
        facade_readback: Duration,
        backend: KfdRuntimeLaunchPerformanceV1,
    }

    #[derive(Debug, Default)]
    struct SampleTimingV1 {
        total: Duration,
        output_reset: Duration,
        synchronized_launch_wait: Duration,
        facade_readback: Duration,
        preparation: Duration,
        bound_snapshot: Duration,
        authority: Duration,
        native_binding: Duration,
        publication: Duration,
        publish_to_completion: Duration,
        completed_readback: Duration,
        recycle: Duration,
    }

    impl SampleTimingV1 {
        fn add(&mut self, timing: IterationTimingV1) {
            self.total += timing.total;
            self.output_reset += timing.output_reset;
            self.synchronized_launch_wait += timing.synchronized_launch_wait;
            self.facade_readback += timing.facade_readback;
            self.preparation += timing.backend.preparation();
            self.bound_snapshot += timing.backend.bound_snapshot();
            self.authority += timing.backend.authority();
            self.native_binding += timing.backend.native_binding();
            self.publication += timing.backend.publication();
            self.publish_to_completion += timing.backend.publish_to_completion();
            self.completed_readback += timing.backend.completed_readback();
            self.recycle += timing.backend.recycle();
        }
    }

    fn per_launch_us(duration: Duration, launches_per_sample: usize) -> f64 {
        duration.as_secs_f64() * 1_000_000.0 / launches_per_sample as f64
    }

    struct QualifiedRunV1 {
        context: RuntimeContextV1<KfdRuntimeBackendV1>,
        stream: RuntimeStreamIdV1,
        module: RuntimeModuleIdV1,
        kernel: TypedRuntimeKernelV1<Gfx942VecaddQualificationArgumentsV1>,
        allocations: [RuntimeAllocationIdV1; 3],
        arguments: Gfx942VecaddQualificationArgumentsV1,
        initial_output: Vec<u8>,
        expected_output: Vec<u8>,
        observed_output: Vec<u8>,
    }

    impl QualifiedRunV1 {
        fn open(device_unique_id: u64) -> Result<Self, String> {
            let admitted =
                admit_gfx942_vecadd_qualification_v1().map_err(|error| error.to_string())?;
            let host = admitted.host_buffers().map_err(|error| error.to_string())?;
            let (left, right, initial_output, expected_output) = host.into_parts();
            let backend =
                KfdRuntimeBackendV1::open_gfx942_vecadd_qualification_v1(device_unique_id)
                    .map_err(|error| error.to_string())?;
            let mut context = RuntimeContextV1::open(backend).map_err(backend_error)?;
            if context.devices().len() != 1 {
                return Err(format!(
                    "qualification requires one admitted device, observed {}",
                    context.devices().len()
                ));
            }
            let device = context
                .devices()
                .iter()
                .find(|device| device.target() == "gfx942:xnack-")
                .ok_or_else(|| "the admitted gfx942:xnack- device was not enumerated".to_owned())?;
            if device.target() != "gfx942:xnack-" {
                return Err(format!(
                    "qualification requires gfx942:xnack-, observed {}",
                    device.target()
                ));
            }
            let device = device.id();
            let stream = context.create_stream(device).map_err(backend_error)?;
            let module = context
                .load_module(device, admitted.hsaco())
                .map_err(backend_error)?;
            let kernel = context
                .resolve_kernel::<Gfx942VecaddQualificationArgumentsV1>(
                    module,
                    GFX942_VECADD_QUALIFICATION_KERNEL_V1,
                )
                .map_err(backend_error)?;
            let mut allocation_roster = Vec::new();
            allocation_roster
                .try_reserve_exact(3)
                .map_err(|_| "qualification allocation roster capacity".to_owned())?;
            for _ in 0..3 {
                allocation_roster.push(
                    context
                        .allocate(
                            device,
                            RuntimeMemoryKindV1::HostVisible,
                            GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1 as u64,
                            GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1,
                        )
                        .map_err(backend_error)?,
                );
            }
            let allocations: [RuntimeAllocationIdV1; 3] = allocation_roster
                .try_into()
                .map_err(|_| "qualification allocation roster cardinality".to_owned())?;
            for (allocation, bytes) in allocations.into_iter().zip([
                left.as_slice(),
                right.as_slice(),
                initial_output.as_slice(),
            ]) {
                context
                    .write_allocation(allocation, 0, bytes)
                    .map_err(backend_error)?;
            }
            let arguments = Gfx942VecaddQualificationArgumentsV1::new(
                allocations[0],
                allocations[1],
                allocations[2],
            )
            .map_err(|error| error.to_string())?;
            Ok(Self {
                context,
                stream,
                module,
                kernel,
                allocations,
                arguments,
                initial_output,
                expected_output,
                observed_output: vec![0; GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1],
            })
        }

        fn iteration(&mut self, record_event: bool) -> Result<IterationTimingV1, String> {
            let total_started = Instant::now();
            let reset_started = Instant::now();
            self.context
                .write_allocation(self.allocations[2], 0, &self.initial_output)
                .map_err(backend_error)?;
            let output_reset = reset_started.elapsed();
            let synchronized_started = Instant::now();
            let mut submission = self
                .context
                .launch(
                    self.stream,
                    &self.kernel,
                    &self.arguments,
                    GFX942_VECADD_QUALIFICATION_GEOMETRY_V1,
                    &[],
                )
                .map_err(backend_error)?;
            let event = record_event
                .then(|| self.context.record_event(&submission))
                .transpose()
                .map_err(backend_error)?;
            let status = self
                .context
                .wait(&mut submission, COMPLETION_TIMEOUT)
                .map_err(backend_error)?;
            if status != RuntimePollV1::Succeeded {
                return Err(format!(
                    "KFD dispatch did not complete successfully before the deadline: {status:?}"
                ));
            }
            if let Some(event) = event {
                self.context.release_event(event).map_err(backend_error)?;
            }
            self.context
                .release_submission(submission)
                .map_err(backend_error)?;
            let synchronized_launch_wait = synchronized_started.elapsed();
            let readback_started = Instant::now();
            self.context
                .read_allocation(self.allocations[2], 0, &mut self.observed_output)
                .map_err(backend_error)?;
            let facade_readback = readback_started.elapsed();
            let backend = self
                .context
                .backend()
                .last_launch_performance_v1()
                .ok_or_else(|| "KFD launch completed without phase timings".to_owned())?;
            Ok(IterationTimingV1 {
                total: total_started.elapsed(),
                output_reset,
                synchronized_launch_wait,
                facade_readback,
                backend,
            })
        }

        fn validate(&self) -> Result<(), String> {
            validate_output(&self.observed_output, &self.expected_output)
        }

        fn shutdown(mut self) -> Result<(), String> {
            for allocation in self.allocations.into_iter().rev() {
                self.context
                    .release_allocation(allocation)
                    .map_err(backend_error)?;
            }
            self.context
                .unload_module(self.module)
                .map_err(backend_error)?;
            self.context
                .destroy_stream(self.stream)
                .map_err(backend_error)?;
            let mut backend = self.context.shutdown().map_err(backend_error)?;
            backend.shutdown_native_v1().map_err(backend_error)
        }
    }

    pub fn run() -> Result<(), Box<dyn Error>> {
        let mut arguments = env::args().skip(1);
        let device_unique_id = parse_unique_id(&arguments.next().ok_or(USAGE)?)?;
        let warmups = parse_positive("WARMUPS", &arguments.next().ok_or(USAGE)?)?;
        let samples = parse_positive("SAMPLES", &arguments.next().ok_or(USAGE)?)?;
        let launches_per_sample =
            parse_positive("LAUNCHES_PER_SAMPLE", &arguments.next().ok_or(USAGE)?)?;
        if arguments.next().is_some() {
            return Err(USAGE.into());
        }

        let mut run = QualifiedRunV1::open(device_unique_id)?;
        run.iteration(true)?;
        run.validate()?;
        println!("backend=kfd event_lifecycle=record_wait_release status=passed");
        for _ in 0..warmups {
            run.iteration(false)?;
        }
        run.validate()?;

        let mut total = Vec::new();
        let mut output_reset = Vec::new();
        let mut synchronized_launch_wait = Vec::new();
        let mut facade_readback = Vec::new();
        let mut preparation = Vec::new();
        let mut bound_snapshot = Vec::new();
        let mut authority = Vec::new();
        let mut native_binding = Vec::new();
        let mut publication = Vec::new();
        let mut publish_to_completion = Vec::new();
        let mut completed_readback = Vec::new();
        let mut recycle = Vec::new();
        for _ in 0..samples {
            let mut sample = SampleTimingV1::default();
            for _ in 0..launches_per_sample {
                sample.add(run.iteration(false)?);
            }
            total.push(per_launch_us(sample.total, launches_per_sample));
            output_reset.push(per_launch_us(sample.output_reset, launches_per_sample));
            synchronized_launch_wait.push(per_launch_us(
                sample.synchronized_launch_wait,
                launches_per_sample,
            ));
            facade_readback.push(per_launch_us(sample.facade_readback, launches_per_sample));
            preparation.push(per_launch_us(sample.preparation, launches_per_sample));
            bound_snapshot.push(per_launch_us(sample.bound_snapshot, launches_per_sample));
            authority.push(per_launch_us(sample.authority, launches_per_sample));
            native_binding.push(per_launch_us(sample.native_binding, launches_per_sample));
            publication.push(per_launch_us(sample.publication, launches_per_sample));
            publish_to_completion.push(per_launch_us(
                sample.publish_to_completion,
                launches_per_sample,
            ));
            completed_readback.push(per_launch_us(
                sample.completed_readback,
                launches_per_sample,
            ));
            recycle.push(per_launch_us(sample.recycle, launches_per_sample));
            run.validate()?;
        }
        report(
            "qualified_persistent_submit_wait_readback",
            &total,
            launches_per_sample,
        );
        report("host_output_reset", &output_reset, launches_per_sample);
        report(
            "synchronized_launch_wait",
            &synchronized_launch_wait,
            launches_per_sample,
        );
        report("facade_readback", &facade_readback, launches_per_sample);
        report("phase_preparation", &preparation, launches_per_sample);
        report("phase_bound_snapshot", &bound_snapshot, launches_per_sample);
        report("phase_authority", &authority, launches_per_sample);
        report("phase_native_binding", &native_binding, launches_per_sample);
        report("phase_publication", &publication, launches_per_sample);
        report(
            "phase_publish_to_completion",
            &publish_to_completion,
            launches_per_sample,
        );
        report(
            "phase_completed_readback",
            &completed_readback,
            launches_per_sample,
        );
        report("phase_recycle", &recycle, launches_per_sample);
        println!(
            "backend=kfd validation=exact status=passed n={}",
            GFX942_VECADD_QUALIFICATION_ELEMENTS_V1
        );
        run.shutdown()?;
        println!("backend=kfd teardown=explicit status=passed");
        Ok(())
    }
}

#[cfg(feature = "hardware-qualification")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    enabled::run()
}
