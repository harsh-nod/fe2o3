//! Exact-fixture qualification of bounded ordinary host dispatch pipelining.

#[cfg(not(feature = "hardware-qualification"))]
fn main() {
    eprintln!("enable the fe2o3-runtime `hardware-qualification` feature");
    std::process::exit(2);
}

#[cfg(feature = "hardware-qualification")]
mod enabled {
    use std::env;
    use std::error::Error;
    use std::fs::{File, OpenOptions};
    use std::io::{Read, Write};
    use std::os::unix::fs::OpenOptionsExt;
    use std::path::Path;
    use std::time::{Duration, Instant};

    use fe2o3_profiler_protocol::{
        KfdRuntimeProfileEventKindV1, KfdRuntimeProfileV1, ProfileIdentityV1,
        encode_kfd_runtime_profile_v1,
    };
    use fe2o3_runtime::qualification_gfx942_vecadd_v1::{
        GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1,
        GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1, GFX942_VECADD_QUALIFICATION_GEOMETRY_V1,
        GFX942_VECADD_QUALIFICATION_HSACO_SHA256_V1, GFX942_VECADD_QUALIFICATION_KERNEL_V1,
        Gfx942VecaddQualificationArgumentsV1, admit_gfx942_vecadd_qualification_v1,
    };
    use fe2o3_runtime::{
        KfdRuntimeBackendV1, KfdRuntimeLaunchDataPathV1, KfdRuntimeProfilerConfigV1,
        RuntimeAllocationIdV1, RuntimeCompletionStatusV1, RuntimeContextV1, RuntimeMemoryKindV1,
        RuntimeModuleIdV1, RuntimePollV1, RuntimeStreamIdV1, RuntimeSubmissionV1,
        TypedRuntimeKernelV1,
    };
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};

    const DEPTH: usize = 64;
    const COMPLETION_TIMEOUT: Duration = Duration::from_secs(10);
    const OUTPUT_SHA256: &str = "79fd0768604fe9de0ced87297f7d653343e998926b59e2cce7df5e38194c52b3";
    const USAGE: &str = "usage: gfx942-runtime-r60-ordinary-pipeline UNIQUE_ID qualify PROFILE_OUTPUT\n\
        or: gfx942-runtime-r60-ordinary-pipeline UNIQUE_ID benchmark SOURCE_COMMIT RUN_ID";

    fn failure(error: impl core::fmt::Debug) -> String {
        let message = format!("{error:?}");
        // Live native custody can require abort-on-drop before main sees this error.
        eprintln!("R60 operation failed before cleanup: {message}");
        message
    }

    fn unique_id(text: &str) -> Result<u64, String> {
        let value = text
            .strip_prefix("0x")
            .map_or_else(|| text.parse::<u64>(), |hex| u64::from_str_radix(hex, 16))
            .map_err(failure)?;
        if value == 0 {
            return Err("device unique ID must be nonzero".to_owned());
        }
        Ok(value)
    }

    struct Run {
        context: RuntimeContextV1<KfdRuntimeBackendV1>,
        stream: RuntimeStreamIdV1,
        module: RuntimeModuleIdV1,
        kernel: TypedRuntimeKernelV1<Gfx942VecaddQualificationArgumentsV1>,
        allocations: [RuntimeAllocationIdV1; 3],
        arguments: Gfx942VecaddQualificationArgumentsV1,
        inputs: [Vec<u8>; 2],
        initial: Vec<u8>,
        expected: Vec<u8>,
        observed: Vec<u8>,
    }

    impl Run {
        fn open(device_unique_id: u64, profiling: bool) -> Result<Self, String> {
            let admitted = admit_gfx942_vecadd_qualification_v1().map_err(failure)?;
            let (left, right, initial, expected) =
                admitted.host_buffers().map_err(failure)?.into_parts();
            if hex(&Sha256::digest(&expected)) != OUTPUT_SHA256 {
                return Err("qualification output identity changed".to_owned());
            }
            let mut backend =
                KfdRuntimeBackendV1::open_gfx942_vecadd_qualification_v1(device_unique_id)
                    .map_err(failure)?;
            if profiling {
                let mut scope = [0; 32];
                File::open("/dev/urandom")
                    .and_then(|mut file| file.read_exact(&mut scope))
                    .map_err(failure)?;
                backend
                    .enable_profiler_v1(
                        KfdRuntimeProfilerConfigV1::new(scope, 1024).map_err(failure)?,
                    )
                    .map_err(failure)?;
            }
            let mut context = RuntimeContextV1::open(backend).map_err(failure)?;
            if context.devices().len() != 1 || context.devices()[0].target() != "gfx942:xnack-" {
                return Err("qualification requires one admitted gfx942:xnack- device".to_owned());
            }
            let device = context.devices()[0].id();
            let stream = context.create_stream(device).map_err(failure)?;
            let module = context
                .load_module(device, admitted.hsaco())
                .map_err(failure)?;
            let kernel = context
                .resolve_kernel::<Gfx942VecaddQualificationArgumentsV1>(
                    module,
                    GFX942_VECADD_QUALIFICATION_KERNEL_V1,
                )
                .map_err(failure)?;
            let mut allocations = Vec::with_capacity(3);
            for bytes in [&left, &right, &initial] {
                let allocation = context
                    .allocate(
                        device,
                        RuntimeMemoryKindV1::HostVisible,
                        GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1 as u64,
                        GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1,
                    )
                    .map_err(failure)?;
                context
                    .write_allocation(allocation, 0, bytes)
                    .map_err(failure)?;
                allocations.push(allocation);
            }
            let allocations: [RuntimeAllocationIdV1; 3] = allocations
                .try_into()
                .map_err(|_| "qualification allocation count".to_owned())?;
            let arguments = Gfx942VecaddQualificationArgumentsV1::new(
                allocations[0],
                allocations[1],
                allocations[2],
            )
            .map_err(failure)?;
            Ok(Self {
                context,
                stream,
                module,
                kernel,
                allocations,
                arguments,
                inputs: [left, right],
                initial,
                expected,
                observed: vec![0; GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1],
            })
        }

        fn issue_batch(
            &mut self,
            submissions: &mut Vec<RuntimeSubmissionV1<Gfx942VecaddQualificationArgumentsV1>>,
        ) -> Result<(), String> {
            if !submissions.is_empty() || submissions.capacity() < DEPTH {
                return Err("submission storage must be empty and preallocated".to_owned());
            }
            for index in 0..DEPTH {
                submissions.push(
                    self.context
                        .launch(
                            self.stream,
                            &self.kernel,
                            &self.arguments,
                            GFX942_VECADD_QUALIFICATION_GEOMETRY_V1,
                            &[],
                        )
                        .map_err(|error| failure(format!("launch {index}: {error:?}")))?,
                );
                // Flush also covers launches needing explicit host publication progress.
                self.context
                    .flush_stream(self.stream)
                    .map_err(|error| failure(format!("flush {index}: {error:?}")))?;
            }
            Ok(())
        }

        fn qualify(mut self) -> Result<KfdRuntimeProfileV1, String> {
            let mut submissions = Vec::with_capacity(DEPTH);
            self.issue_batch(&mut submissions)?;
            for submission in &submissions {
                if self.context.query_submission(submission).map_err(failure)?
                    != RuntimeCompletionStatusV1::Pending
                {
                    return Err("facade observed completion during publication prefix".to_owned());
                }
            }
            let tail = submissions.last_mut().ok_or("empty publication batch")?;
            if self
                .context
                .wait(tail, COMPLETION_TIMEOUT)
                .map_err(failure)?
                != RuntimePollV1::Succeeded
            {
                return Err(failure("pipeline tail did not succeed before the deadline"));
            }
            let performance = self
                .context
                .backend()
                .last_launch_performance_v1()
                .ok_or("missing tail launch performance")?;
            if performance.data_path() != KfdRuntimeLaunchDataPathV1::ResidentReused
                || performance.user_data_materializations() != 0
                || performance.persistent_control_reused()
            {
                return Err(failure(format!(
                    "unexpected ordinary pipeline data path: {performance:?}"
                )));
            }
            for submission in &mut submissions {
                if self.context.poll(submission).map_err(failure)? != RuntimePollV1::Succeeded {
                    return Err(failure("submission did not succeed after tail completion"));
                }
            }
            self.context
                .read_allocation(self.allocations[2], 0, &mut self.observed)
                .map_err(failure)?;
            let numerical_result = if self.observed == self.expected {
                Ok(())
            } else {
                Err("pipeline output failed full-byte comparison".to_owned())
            };
            for submission in submissions.into_iter().rev() {
                self.context
                    .release_submission(submission)
                    .map_err(failure)?;
            }
            let profile = self
                .shutdown(true)?
                .ok_or("missing qualification profile")?;
            numerical_result?;
            profile.validate().map_err(failure)?;
            if !profile.coverage.complete_runtime_operation_history
                || profile.coverage.dropped_events != 0
            {
                return Err("incomplete qualification profile".to_owned());
            }
            validate_publication_prefix(profile.events.iter().map(|entry| &entry.event))?;
            Ok(profile)
        }

        fn shutdown(mut self, profiling: bool) -> Result<Option<KfdRuntimeProfileV1>, String> {
            for allocation in self.allocations.into_iter().rev() {
                self.context
                    .release_allocation(allocation)
                    .map_err(failure)?;
            }
            self.context.unload_module(self.module).map_err(failure)?;
            self.context.destroy_stream(self.stream).map_err(failure)?;
            let mut backend = self.context.shutdown().map_err(failure)?;
            backend.shutdown_native_v1().map_err(failure)?;
            if profiling {
                backend.finish_profiler_v1().map(Some).map_err(failure)
            } else {
                Ok(None)
            }
        }

        fn benchmark(
            mut self,
            device_unique_id: u64,
            source_commit: &str,
            run_id: &str,
        ) -> Result<Vec<Value>, String> {
            let mut records = Vec::with_capacity(42);
            records.push(json!({
                "record": "config", "schema": "fe2o3.r60-pipeline-benchmark.v1", "backend": "kfd",
                "run_id": run_id, "source_commit": source_commit, "unique_id": format!("{device_unique_id:016x}"),
                "target": "gfx942:xnack-", "hsaco_sha256": hex(&GFX942_VECADD_QUALIFICATION_HSACO_SHA256_V1),
                "workload": "trusted-gfx942-vecadd-v1", "elements": 1048576, "bytes_per_buffer": 4194304,
                "grid": [1048576, 1, 1], "workgroup": [256, 1, 1], "access": ["read", "read", "write"],
                "memory": "host-visible-coherent", "launches_per_batch": DEPTH, "warmups": 10, "samples": 30,
                "ordering": "same-stream-ordered", "issue_api_calls_per_batch": DEPTH * 2, "host_waits_during_issue": 0,
                "reset_timed": false, "explicit_allocation_api_timed": false, "validation": "byte-exact-every-batch",
                "clock": "steady-monotonic-ns", "wait_timeout_ns": 10000000000_u64,
            }));
            let mut submissions = Vec::with_capacity(DEPTH);
            for ordinal in 0..40 {
                self.context
                    .write_allocation(self.allocations[2], 0, &self.initial)
                    .map_err(failure)?;
                let started = Instant::now();
                self.issue_batch(&mut submissions)?;
                let issued = Instant::now();
                let tail = submissions.last_mut().ok_or("empty benchmark batch")?;
                let remaining = COMPLETION_TIMEOUT.saturating_sub(issued.duration_since(started));
                if self.context.wait(tail, remaining).map_err(failure)? != RuntimePollV1::Succeeded
                {
                    return Err(failure(
                        "benchmark tail did not succeed before the deadline",
                    ));
                }
                let completed = Instant::now();
                if completed.duration_since(started) > COMPLETION_TIMEOUT {
                    return Err(failure("benchmark batch exceeded the deadline"));
                }
                for submission in &mut submissions {
                    if self.context.poll(submission).map_err(failure)? != RuntimePollV1::Succeeded {
                        return Err(
                            "benchmark submission did not succeed after its tail".to_owned()
                        );
                    }
                }
                for (allocation, expected) in self.allocations.into_iter().zip([
                    self.inputs[0].as_slice(),
                    self.inputs[1].as_slice(),
                    self.expected.as_slice(),
                ]) {
                    self.context
                        .read_allocation(allocation, 0, &mut self.observed)
                        .map_err(failure)?;
                    if self.observed != expected {
                        return Err("benchmark failed full-byte input/output comparison".to_owned());
                    }
                }
                let output_sha256 = hex(&Sha256::digest(&self.observed));
                for submission in submissions.drain(..).rev() {
                    self.context
                        .release_submission(submission)
                        .map_err(failure)?;
                }
                records.push(json!({
                    "record": "batch", "phase": if ordinal < 10 { "warmup" } else { "sample" },
                    "index": if ordinal < 10 { ordinal } else { ordinal - 10 },
                    "issued_launches": DEPTH, "completed_launches": DEPTH,
                    "issue_batch_ns": nanos(issued.duration_since(started))?,
                    "tail_wait_batch_ns": nanos(completed.duration_since(issued))?,
                    "total_batch_ns": nanos(completed.duration_since(started))?,
                    "output_sha256": output_sha256,
                }));
            }
            self.shutdown(false)?;
            records.push(json!({"record": "complete", "validated_batches": 40}));
            Ok(records)
        }
    }

    fn hex(bytes: &[u8]) -> String {
        use std::fmt::Write;
        let mut text = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            write!(text, "{byte:02x}").expect("writing to a string");
        }
        text
    }

    fn require_hex(text: &str, length: usize) -> Result<(), String> {
        if text.len() != length
            || !text
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || text.bytes().all(|byte| byte == b'0')
        {
            return Err(format!(
                "expected {length} lowercase nonzero hexadecimal digits"
            ));
        }
        Ok(())
    }

    fn nanos(duration: Duration) -> Result<u64, String> {
        u64::try_from(duration.as_nanos()).map_err(failure)
    }

    fn validate_publication_prefix<'a>(
        events: impl IntoIterator<Item = &'a KfdRuntimeProfileEventKindV1>,
    ) -> Result<(), String> {
        let mut published: Vec<ProfileIdentityV1> = Vec::with_capacity(DEPTH);
        let mut completed = 0;
        for event in events {
            match event {
                KfdRuntimeProfileEventKindV1::DispatchPublished { dispatch, .. } => {
                    if completed != 0 || published.len() == DEPTH || published.contains(dispatch) {
                        return Err(
                            "publication prefix is serialized, oversized, or duplicated".to_owned()
                        );
                    }
                    published.push(*dispatch);
                }
                KfdRuntimeProfileEventKindV1::DispatchCompleted {
                    dispatch,
                    host_timing,
                } => {
                    if published.len() != DEPTH || published.get(completed) != Some(dispatch) {
                        return Err(
                            "completion preceded the full publication prefix or violated its order"
                                .to_owned(),
                        );
                    }
                    if completed != 0 && host_timing.native_binding_ns != 0 {
                        return Err("successor rematerialized native bindings".to_owned());
                    }
                    completed += 1;
                }
                _ => {}
            }
        }
        if published.len() != DEPTH || completed != DEPTH {
            return Err("incomplete publication/completion roster".to_owned());
        }
        Ok(())
    }

    pub fn run() -> Result<(), Box<dyn Error>> {
        let args: Vec<_> = env::args().skip(1).collect();
        if args.len() == 4 && args[1] == "benchmark" {
            let device_unique_id = unique_id(&args[0])?;
            require_hex(&args[2], 40)?;
            require_hex(&args[3], 64)?;
            let records = Run::open(device_unique_id, false)?.benchmark(
                device_unique_id,
                &args[2],
                &args[3],
            )?;
            let stdout = std::io::stdout();
            let mut output = stdout.lock();
            for record in records {
                serde_json::to_writer(&mut output, &record)?;
                output.write_all(b"\n")?;
            }
            output.flush()?;
            return Ok(());
        }
        if args.len() != 3 || args[1] != "qualify" {
            return Err(USAGE.into());
        }
        let device_unique_id = unique_id(&args[0])?;
        let output = Path::new(&args[2]);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(output)?;
        let profile = Run::open(device_unique_id, true)?.qualify()?;
        let bytes = encode_kfd_runtime_profile_v1(&profile).map_err(failure)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        println!(
            "PASS schema=fe2o3.runtime.gfx942-r60-ordinary-pipeline-qualification.v1 launches=64 publication_prefix=64 completion_order=contiguous execution_order=wait-for-prior concurrent_kernel_execution=false data_path_last=ResidentReused user_data_materializations_last=0 readbacks=1 output_sha256={OUTPUT_SHA256} cleanup=complete"
        );
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use fe2o3_profiler_protocol::{
            KfdProfileHostTimingV1, KfdProfileLaunchV1, ProfileContentIdentityV1,
        };

        fn id(index: u8) -> ProfileIdentityV1 {
            ProfileIdentityV1::new([index + 1; 32]).unwrap()
        }

        #[test]
        fn exact_fixture_output_matches_the_benchmark_contract() {
            let admitted = admit_gfx942_vecadd_qualification_v1().unwrap();
            let (_, _, _, expected) = admitted.host_buffers().unwrap().into_parts();
            assert_eq!(hex(&Sha256::digest(&expected)), OUTPUT_SHA256);
        }

        fn events() -> Vec<KfdRuntimeProfileEventKindV1> {
            let mut events = Vec::new();
            for index in 0..DEPTH as u8 {
                events.push(KfdRuntimeProfileEventKindV1::DispatchPublished {
                    dispatch: id(index),
                    queue: id(100),
                    stream: id(101),
                    kernel: id(102),
                    dispatch_shape: ProfileContentIdentityV1::observed(b"shape").unwrap(),
                    launch: KfdProfileLaunchV1 {
                        grid: [1_048_576, 1, 1],
                        workgroup: [256, 1, 1],
                        dynamic_shared_bytes: 0,
                    },
                    bindings: Vec::new(),
                });
            }
            for index in 0..DEPTH as u8 {
                events.push(KfdRuntimeProfileEventKindV1::DispatchCompleted {
                    dispatch: id(index),
                    host_timing: KfdProfileHostTimingV1::default(),
                });
            }
            events
        }

        #[test]
        fn complete_publication_prefix_is_required() {
            assert!(validate_publication_prefix(&events()).is_ok());
            let mut serialized = events();
            let first_completion = serialized.remove(DEPTH);
            serialized.insert(1, first_completion);
            assert!(validate_publication_prefix(&serialized).is_err());
            assert!(validate_publication_prefix(&events()[..DEPTH * 2 - 1]).is_err());
        }

        #[test]
        fn substituted_duplicate_and_reordered_identities_are_rejected() {
            let mut duplicate = events();
            duplicate[1] = duplicate[0].clone();
            assert!(validate_publication_prefix(&duplicate).is_err());
            let mut reordered = events();
            reordered.swap(DEPTH, DEPTH + 1);
            assert!(validate_publication_prefix(&reordered).is_err());
            let mut foreign = events();
            if let KfdRuntimeProfileEventKindV1::DispatchCompleted { dispatch, .. } =
                &mut foreign[DEPTH]
            {
                *dispatch = id(90);
            }
            assert!(validate_publication_prefix(&foreign).is_err());
        }

        #[test]
        fn native_successor_rematerialization_is_rejected() {
            let mut materialized = events();
            if let KfdRuntimeProfileEventKindV1::DispatchCompleted { host_timing, .. } =
                &mut materialized[DEPTH + 1]
            {
                host_timing.native_binding_ns = 1;
            }
            assert!(validate_publication_prefix(&materialized).is_err());
        }
    }
}

#[cfg(feature = "hardware-qualification")]
fn main() {
    if let Err(error) = enabled::run() {
        eprintln!("R60 ordinary pipeline qualification failed: {error}");
        std::process::exit(1);
    }
}
