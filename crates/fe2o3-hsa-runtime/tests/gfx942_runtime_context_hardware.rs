//! Exact-fixture `RuntimeContextV1` qualification for the reviewed HSA backend.
//!
//! This lane is deliberately ignored and feature-gated. It admits only the
//! repository-owned gfx942 vecadd fixture and an explicitly isolated visible
//! device zero. Passing it is bounded runtime evidence, not general module
//! authority or Worker V3 authentication.

#[cfg(feature = "hardware-qualification")]
mod qualification {
    use std::error::Error;
    use std::time::{Duration, Instant};

    use fe2o3_amd_target::AmdTargetId;
    use fe2o3_core::GpuContext;
    use fe2o3_hsa_runtime::ReviewedHsaRuntimeBackendV1;
    use fe2o3_runtime::qualification_gfx942_vecadd_v1::{
        AdmittedGfx942VecaddQualificationV1, GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1,
        GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1, GFX942_VECADD_QUALIFICATION_ELEMENTS_V1,
        GFX942_VECADD_QUALIFICATION_TARGET_V1, Gfx942VecaddQualificationArgumentsV1,
        admit_gfx942_vecadd_qualification_v1,
    };
    use fe2o3_runtime::{
        RuntimeAllocationIdV1, RuntimeContextV1, RuntimeEventIdV1, RuntimeMemoryKindV1,
        RuntimePollV1, RuntimeStreamIdV1, TypedRuntimeKernelV1,
    };

    type BoxError = Box<dyn Error>;

    const RUN_ENV_V1: &str = "FE2O3_RUN_GFX942_RUNTIME_HSA_QUALIFICATION";
    const HIP_VISIBLE_ENV_V1: &str = "HIP_VISIBLE_DEVICES";
    const ROCR_VISIBLE_ENV_V1: &str = "ROCR_VISIBLE_DEVICES";
    const WAIT_TIMEOUT_V1: Duration = Duration::from_secs(30);
    const SOURCE_STREAMS_V1: usize = 6;
    const DEFAULT_WARMUPS_V1: usize = 10;
    const DEFAULT_SAMPLES_V1: usize = 30;
    const DEFAULT_LAUNCHES_PER_SAMPLE_V1: usize = 10;
    const WARMUPS_ENV_V1: &str = "FE2O3_RUNTIME_WARMUPS";
    const SAMPLES_ENV_V1: &str = "FE2O3_RUNTIME_SAMPLES";
    const LAUNCHES_ENV_V1: &str = "FE2O3_RUNTIME_LAUNCHES_PER_SAMPLE";

    fn require(condition: bool, message: impl Into<String>) -> Result<(), BoxError> {
        if condition {
            Ok(())
        } else {
            Err(message.into().into())
        }
    }

    fn require_opt_in_and_device_isolation_v1() -> Result<(), BoxError> {
        require(
            !cfg!(debug_assertions),
            "HSA timing qualification must be compiled with --release",
        )?;
        require(
            std::env::var(RUN_ENV_V1).as_deref() == Ok("1"),
            format!("set {RUN_ENV_V1}=1 to opt into exact HSA hardware qualification"),
        )?;
        let hip_visible = std::env::var(HIP_VISIBLE_ENV_V1)
            .map_err(|_| format!("{HIP_VISIBLE_ENV_V1} must select one decimal GPU index"))?;
        let rocr_visible = std::env::var(ROCR_VISIBLE_ENV_V1)
            .map_err(|_| format!("{ROCR_VISIBLE_ENV_V1} must select one decimal GPU index"))?;
        require(
            hip_visible == "0",
            format!("{HIP_VISIBLE_ENV_V1} must select ordinal zero after ROCr device isolation"),
        )?;
        require(
            !rocr_visible.is_empty()
                && rocr_visible.bytes().all(|byte| byte.is_ascii_digit())
                && rocr_visible.parse::<u32>().is_ok(),
            format!("{ROCR_VISIBLE_ENV_V1} must be one decimal physical GPU index"),
        )?;
        Ok(())
    }

    #[derive(Clone, Copy, Debug)]
    struct BenchmarkConfigurationV1 {
        warmups: usize,
        samples: usize,
        launches_per_sample: usize,
    }

    impl BenchmarkConfigurationV1 {
        fn from_environment_v1() -> Result<Self, BoxError> {
            let configuration = Self {
                warmups: parse_positive_environment_v1(WARMUPS_ENV_V1, DEFAULT_WARMUPS_V1)?,
                samples: parse_positive_environment_v1(SAMPLES_ENV_V1, DEFAULT_SAMPLES_V1)?,
                launches_per_sample: parse_positive_environment_v1(
                    LAUNCHES_ENV_V1,
                    DEFAULT_LAUNCHES_PER_SAMPLE_V1,
                )?,
            };
            let timed_launches = configuration
                .samples
                .checked_mul(configuration.launches_per_sample)
                .and_then(|value| value.checked_mul(2))
                .and_then(|value| value.checked_add(configuration.warmups))
                .ok_or("HSA benchmark launch count overflow")?;
            require(
                timed_launches > 64,
                "HSA qualification requires more than 64 sequential launches to cover ring wrap",
            )?;
            Ok(configuration)
        }
    }

    fn parse_positive_environment_v1(name: &str, default: usize) -> Result<usize, BoxError> {
        match std::env::var(name) {
            Ok(text) => text
                .parse::<usize>()
                .map_err(|error| format!("invalid {name}={text:?}: {error}").into())
                .and_then(|value| {
                    require(value != 0, format!("{name} must be nonzero"))?;
                    Ok(value)
                }),
            Err(std::env::VarError::NotPresent) => Ok(default),
            Err(error) => Err(format!("invalid {name}: {error}").into()),
        }
    }

    struct HsaQualificationRunV1 {
        context: RuntimeContextV1<ReviewedHsaRuntimeBackendV1>,
        streams: Vec<RuntimeStreamIdV1>,
        module: fe2o3_runtime::RuntimeModuleIdV1,
        kernel: TypedRuntimeKernelV1<Gfx942VecaddQualificationArgumentsV1>,
        left: RuntimeAllocationIdV1,
        right: RuntimeAllocationIdV1,
        outputs: Vec<RuntimeAllocationIdV1>,
        initial_output: Vec<u8>,
        expected_output: Vec<u8>,
        observed_output: Vec<u8>,
        // Declared last so unwind/drop destroys the runtime context first.
        _admission: AdmittedGfx942VecaddQualificationV1,
    }

    impl HsaQualificationRunV1 {
        fn open() -> Result<Self, BoxError> {
            require_opt_in_and_device_isolation_v1()?;
            let admission = admit_gfx942_vecadd_qualification_v1()?;
            let gpu_context = GpuContext::new(0)?;
            // SAFETY: the exact repository-owned fixture is manually reviewed
            // as trusted code. `admission` re-hashes and structurally validates
            // that fixture's module, typed ABI, and metadata-declared
            // read/read/write effects, and is retained for the complete backend
            // lifetime.
            // Device visibility is fixed to one correlated gfx942:xnack- device.
            let backend = unsafe { ReviewedHsaRuntimeBackendV1::new(gpu_context) }?;
            let mut context = RuntimeContextV1::open(backend)?;
            let [device] = context.devices() else {
                return Err("reviewed HSA qualification requires exactly one device".into());
            };
            let artifact_target = AmdTargetId::parse(GFX942_VECADD_QUALIFICATION_TARGET_V1)
                .map_err(|error| format!("invalid fixture target: {error:?}"))?;
            let observed_target = AmdTargetId::parse(device.target())
                .map_err(|error| format!("invalid HSA device target: {error:?}"))?;
            require(
                artifact_target.is_compatible_with_observed(&observed_target),
                format!(
                    "HSA device target {} is incompatible with fixture target {}",
                    device.target(),
                    GFX942_VECADD_QUALIFICATION_TARGET_V1,
                ),
            )?;
            let device = device.id();
            let mut streams = Vec::new();
            streams.try_reserve_exact(SOURCE_STREAMS_V1 + 2)?;
            for _ in 0..SOURCE_STREAMS_V1 + 2 {
                streams.push(context.create_stream(device)?);
            }
            let module = context.load_module(device, admission.hsaco())?;
            let kernel = context.resolve_kernel::<Gfx942VecaddQualificationArgumentsV1>(
                module,
                admission.kernel_name(),
            )?;
            let host = admission.host_buffers()?;
            let (left_bytes, right_bytes, initial_output, expected_output) = host.into_parts();
            let left = Self::allocate_v1(&mut context, device)?;
            let right = Self::allocate_v1(&mut context, device)?;
            let mut outputs = Vec::new();
            outputs.try_reserve_exact(SOURCE_STREAMS_V1 + 1)?;
            for _ in 0..SOURCE_STREAMS_V1 + 1 {
                outputs.push(Self::allocate_v1(&mut context, device)?);
            }
            context.write_allocation(left, 0, &left_bytes)?;
            context.write_allocation(right, 0, &right_bytes)?;
            for output in &outputs {
                context.write_allocation(*output, 0, &initial_output)?;
            }
            Ok(Self {
                context,
                streams,
                module,
                kernel,
                left,
                right,
                outputs,
                initial_output,
                expected_output,
                observed_output: vec![0; GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1],
                _admission: admission,
            })
        }

        fn allocate_v1(
            context: &mut RuntimeContextV1<ReviewedHsaRuntimeBackendV1>,
            device: fe2o3_runtime::RuntimeDeviceIdV1,
        ) -> Result<RuntimeAllocationIdV1, BoxError> {
            Ok(context.allocate(
                device,
                RuntimeMemoryKindV1::HostVisible,
                GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1 as u64,
                GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1,
            )?)
        }

        fn arguments_v1(
            &self,
            output: RuntimeAllocationIdV1,
        ) -> Result<Gfx942VecaddQualificationArgumentsV1, BoxError> {
            Ok(Gfx942VecaddQualificationArgumentsV1::new(
                self.left, self.right, output,
            )?)
        }

        fn reset_output_v1(&mut self, output: RuntimeAllocationIdV1) -> Result<(), BoxError> {
            Ok(self
                .context
                .write_allocation(output, 0, &self.initial_output)?)
        }

        fn launch_wait_release_v1(
            &mut self,
            stream: RuntimeStreamIdV1,
            output: RuntimeAllocationIdV1,
            dependencies: &[RuntimeEventIdV1],
        ) -> Result<(), BoxError> {
            let arguments = self.arguments_v1(output)?;
            let mut submission = self.context.launch(
                stream,
                &self.kernel,
                &arguments,
                self._admission.geometry(),
                dependencies,
            )?;
            require(
                self.context.wait(&mut submission, WAIT_TIMEOUT_V1)? == RuntimePollV1::Succeeded,
                "reviewed HSA dispatch did not succeed before its deadline",
            )?;
            self.context
                .release_submission(submission)
                .map_err(|failure| failure.error().to_string().into())
        }

        fn launch_wait_release_read_v1(
            &mut self,
            stream: RuntimeStreamIdV1,
            output: RuntimeAllocationIdV1,
        ) -> Result<(), BoxError> {
            self.reset_output_v1(output)?;
            self.launch_wait_release_v1(stream, output, &[])?;
            self.context
                .read_allocation(output, 0, &mut self.observed_output)?;
            self.validate_observed_v1()
        }

        fn validate_observed_v1(&self) -> Result<(), BoxError> {
            if let Some((index, (observed, expected))) = self
                .observed_output
                .chunks_exact(size_of::<f32>())
                .zip(self.expected_output.chunks_exact(size_of::<f32>()))
                .enumerate()
                .find(|(_, (observed, expected))| observed != expected)
            {
                return Err(format!(
                    "HSA vecadd mismatch at element {index}: expected {:#010x}, observed {:#010x}",
                    u32::from_le_bytes(expected.try_into().expect("one expected f32")),
                    u32::from_le_bytes(observed.try_into().expect("one observed f32")),
                )
                .into());
            }
            Ok(())
        }

        fn six_source_event_fan_in_v1(&mut self) -> Result<(), BoxError> {
            let mut sources = Vec::new();
            let mut events = Vec::new();
            sources.try_reserve_exact(SOURCE_STREAMS_V1)?;
            events.try_reserve_exact(SOURCE_STREAMS_V1)?;
            for index in 0..SOURCE_STREAMS_V1 {
                self.reset_output_v1(self.outputs[index])?;
                let arguments = self.arguments_v1(self.outputs[index])?;
                let submission = self.context.launch(
                    self.streams[index],
                    &self.kernel,
                    &arguments,
                    self._admission.geometry(),
                    &[],
                )?;
                events.push(self.context.record_event(&submission)?);
                sources.push(submission);
            }
            let dependent_output = self.outputs[SOURCE_STREAMS_V1];
            self.reset_output_v1(dependent_output)?;
            let dependent_arguments = self.arguments_v1(dependent_output)?;
            let mut dependent = self.context.launch(
                self.streams[SOURCE_STREAMS_V1],
                &self.kernel,
                &dependent_arguments,
                self._admission.geometry(),
                &events,
            )?;
            require(
                self.context.wait(&mut dependent, WAIT_TIMEOUT_V1)? == RuntimePollV1::Succeeded,
                "six-event dependent HSA dispatch did not succeed",
            )?;
            for source in &mut sources {
                require(
                    self.context.wait(source, WAIT_TIMEOUT_V1)? == RuntimePollV1::Succeeded,
                    "source HSA dispatch did not succeed",
                )?;
            }
            self.context
                .release_submission(dependent)
                .map_err(|failure| -> BoxError { failure.error().to_string().into() })?;
            for event in events {
                self.context.release_event(event)?;
            }
            for source in sources {
                self.context
                    .release_submission(source)
                    .map_err(|failure| -> BoxError { failure.error().to_string().into() })?;
            }
            for output in self.outputs.clone() {
                self.context
                    .read_allocation(output, 0, &mut self.observed_output)?;
                self.validate_observed_v1()?;
            }
            Ok(())
        }

        fn benchmark_v1(
            &mut self,
            configuration: BenchmarkConfigurationV1,
        ) -> Result<(), BoxError> {
            let stream = self.streams[SOURCE_STREAMS_V1 + 1];
            let output = self.outputs[0];
            for _ in 0..configuration.warmups {
                self.launch_wait_release_read_v1(stream, output)?;
            }

            let mut host_visible_readback = Vec::new();
            host_visible_readback.try_reserve_exact(configuration.samples)?;
            for _ in 0..configuration.samples {
                let mut elapsed = Duration::ZERO;
                for _ in 0..configuration.launches_per_sample {
                    self.reset_output_v1(output)?;
                    let started = Instant::now();
                    self.launch_wait_release_v1(stream, output, &[])?;
                    self.context
                        .read_allocation(output, 0, &mut self.observed_output)?;
                    elapsed += started.elapsed();
                }
                host_visible_readback.push(
                    elapsed.as_secs_f64() * 1_000_000.0 / configuration.launches_per_sample as f64,
                );
                self.validate_observed_v1()?;
            }
            report_v1(
                "host_visible_submit_wait_readback",
                host_visible_readback,
                configuration,
            );

            let mut synchronized = Vec::new();
            synchronized.try_reserve_exact(configuration.samples)?;
            for _ in 0..configuration.samples {
                let mut elapsed = Duration::ZERO;
                for _ in 0..configuration.launches_per_sample {
                    self.reset_output_v1(output)?;
                    let started = Instant::now();
                    self.launch_wait_release_v1(stream, output, &[])?;
                    elapsed += started.elapsed();
                }
                synchronized.push(
                    elapsed.as_secs_f64() * 1_000_000.0 / configuration.launches_per_sample as f64,
                );
                self.context
                    .read_allocation(output, 0, &mut self.observed_output)?;
                self.validate_observed_v1()?;
            }
            report_v1("synchronized_launch_wait", synchronized, configuration);
            Ok(())
        }

        fn shutdown(mut self) -> Result<(), BoxError> {
            for output in self.outputs.into_iter().rev() {
                self.context.release_allocation(output)?;
            }
            self.context.release_allocation(self.right)?;
            self.context.release_allocation(self.left)?;
            self.context.unload_module(self.module)?;
            for stream in self.streams.into_iter().rev() {
                self.context.destroy_stream(stream)?;
            }
            self.context.shutdown().map(|_| ()).map_err(|failure| {
                format!("HSA context shutdown failed: {:?}", failure.report()).into()
            })
        }
    }

    fn report_v1(metric: &str, mut samples: Vec<f64>, configuration: BenchmarkConfigurationV1) {
        samples.sort_by(f64::total_cmp);
        let percentile = |value: usize| samples[(samples.len() - 1) * value / 100];
        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
        println!(
            "backend=hsa metric={metric} n={} samples={} launches_per_sample={} min_us={:.3} p50_us={:.3} mean_us={:.3} p90_us={:.3} max_us={:.3}",
            GFX942_VECADD_QUALIFICATION_ELEMENTS_V1,
            samples.len(),
            configuration.launches_per_sample,
            samples[0],
            percentile(50),
            mean,
            percentile(90),
            samples[samples.len() - 1],
        );
    }

    fn run_v1() -> Result<(), BoxError> {
        let configuration = BenchmarkConfigurationV1::from_environment_v1()?;
        let mut run = HsaQualificationRunV1::open()?;
        run.launch_wait_release_read_v1(run.streams[0], run.outputs[0])?;
        run.six_source_event_fan_in_v1()?;
        run.benchmark_v1(configuration)?;
        println!(
            "backend=hsa validation=exact status=passed n={}",
            GFX942_VECADD_QUALIFICATION_ELEMENTS_V1
        );
        run.shutdown()?;
        println!("backend=hsa teardown=explicit status=passed");
        Ok(())
    }

    /// Runs exact gfx942 HSA facade execution, six-event fan-in, repeated ring
    /// wrap, deterministic result checks, stable timing, and explicit teardown.
    ///
    /// ```text
    /// HIP_VISIBLE_DEVICES=0 ROCR_VISIBLE_DEVICES=<physical-gpu-index> \
    /// FE2O3_RUN_GFX942_RUNTIME_HSA_QUALIFICATION=1 \
    /// cargo test --release --locked -p fe2o3-hsa-runtime \
    ///   --features hardware-qualification \
    ///   --test gfx942_runtime_context_hardware \
    ///   qualification::gfx942_runtime_context_exact_fixture_executes_dependencies_wraps_and_times \
    ///   -- --ignored --exact --nocapture --test-threads=1
    /// ```
    #[test]
    #[ignore = "requires explicit device-zero isolation and one gfx942:xnack- GPU"]
    fn gfx942_runtime_context_exact_fixture_executes_dependencies_wraps_and_times()
    -> Result<(), BoxError> {
        run_v1()
    }
}
