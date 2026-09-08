//! Exact live qualification of the gfx942 DeviceLocal R57 N3 path.

#[cfg(not(feature = "hardware-qualification"))]
fn main() {
    eprintln!("enable the fe2o3-runtime `hardware-qualification` feature");
    std::process::exit(2);
}

#[cfg(feature = "hardware-qualification")]
mod enabled {
    use std::env;
    use std::error::Error;
    use std::time::Duration;

    use fe2o3_runtime::qualification_gfx942_r57_n3_v1::{
        GFX942_R57_N3_QUALIFICATION_BUFFER_ALIGNMENT_V1,
        GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1, GFX942_R57_N3_QUALIFICATION_GEOMETRY_V1,
        GFX942_R57_N3_QUALIFICATION_KERNEL_V1, Gfx942R57N3QualificationArgumentsV1,
        Gfx942R57N3QualificationAuthorityObservationV1, admit_gfx942_r57_n3_qualification_v1,
    };
    use fe2o3_runtime::{
        KfdRuntimeBackendErrorKindV1, KfdRuntimeBackendV1, KfdRuntimeLaunchDataPathV1,
        KfdRuntimeLaunchPerformanceV1, RuntimeAllocationIdV1, RuntimeContextV1, RuntimeErrorV1,
        RuntimeMemoryKindV1, RuntimeModuleIdV1, RuntimePollV1, RuntimeStreamIdV1,
        TypedRuntimeKernelV1,
    };
    use sha2::{Digest, Sha256};

    const USAGE: &str = "usage: gfx942-runtime-r57-n3-qualification UNIQUE_ID_OR_AUTO";
    const COMPLETION_TIMEOUT: Duration = Duration::from_secs(30);

    fn parse_unique_id(text: &str) -> Result<u64, String> {
        if text == "auto" {
            return fe2o3_kfd::topology::discover_default_topology()
                .map_err(|error| format!("KFD topology discovery: {error}"))?
                .topology()
                .gpu_nodes()
                .iter()
                .filter(|node| node.target().name() == "gfx942")
                .filter(|node| node.capacity().wavefront_size() == 64)
                .map(|node| node.unique_id())
                .filter(|unique_id| *unique_id != 0)
                .min()
                .ok_or_else(|| "no nonzero gfx942 Wave64 KFD device was observed".to_owned());
        }
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

    fn backend_error(error: impl core::fmt::Debug) -> String {
        format!("{error:?}")
    }

    fn exact_bytes(label: &str, observed: &[u8], expected: &[u8]) -> Result<(), String> {
        if observed == expected {
            return Ok(());
        }
        let mismatch = observed
            .iter()
            .zip(expected)
            .position(|(observed, expected)| observed != expected)
            .unwrap_or(observed.len().min(expected.len()));
        Err(format!(
            "{label} readback mismatch at byte {mismatch}: observed_len={} expected_len={}",
            observed.len(),
            expected.len()
        ))
    }

    fn exact_r57_performance(
        label: &str,
        performance: KfdRuntimeLaunchPerformanceV1,
    ) -> Result<(), String> {
        if performance.data_path() != KfdRuntimeLaunchDataPathV1::PersistentDeviceReused
            || performance.user_data_materializations() != 0
            || performance.persistent_control_reused()
        {
            return Err(format!(
                "{label} used unexpected R57 path: data_path={:?} materializations={} control_reused={}",
                performance.data_path(),
                performance.user_data_materializations(),
                performance.persistent_control_reused()
            ));
        }
        Ok(())
    }

    struct QualifiedRunV1 {
        context: RuntimeContextV1<KfdRuntimeBackendV1>,
        authority: Gfx942R57N3QualificationAuthorityObservationV1,
        stream: RuntimeStreamIdV1,
        module: RuntimeModuleIdV1,
        kernel: TypedRuntimeKernelV1<Gfx942R57N3QualificationArgumentsV1>,
        allocations: [RuntimeAllocationIdV1; 4],
        initial: [Vec<u8>; 4],
        expected: [Vec<u8>; 2],
    }

    impl QualifiedRunV1 {
        fn open(device_unique_id: u64) -> Result<Self, String> {
            let admitted =
                admit_gfx942_r57_n3_qualification_v1().map_err(|error| error.to_string())?;
            let [a, b, c_initial, d_initial, expected_c, expected_d] = admitted
                .host_buffers()
                .map_err(|error| error.to_string())?
                .into_parts();
            let (backend, authority) =
                KfdRuntimeBackendV1::open_gfx942_r57_n3_qualification_v1(device_unique_id)
                    .map_err(backend_error)?;
            let mut context = RuntimeContextV1::open(backend).map_err(backend_error)?;
            if context.devices().len() != 1 {
                return Err(format!(
                    "qualification requires one admitted device, observed {}",
                    context.devices().len()
                ));
            }
            let device = context.devices()[0].id();
            if context.devices()[0].target() != "gfx942:xnack-" {
                return Err(format!(
                    "qualification requires gfx942:xnack-, observed {}",
                    context.devices()[0].target()
                ));
            }
            let stream = context.create_stream(device).map_err(backend_error)?;
            let module = context
                .load_module(device, admitted.hsaco())
                .map_err(backend_error)?;
            let kernel = context
                .resolve_kernel::<Gfx942R57N3QualificationArgumentsV1>(
                    module,
                    GFX942_R57_N3_QUALIFICATION_KERNEL_V1,
                )
                .map_err(backend_error)?;
            let mut roster = Vec::new();
            roster
                .try_reserve_exact(4)
                .map_err(|_| "qualification allocation roster capacity".to_owned())?;
            for _ in 0..4 {
                roster.push(
                    context
                        .allocate(
                            device,
                            RuntimeMemoryKindV1::DeviceLocal,
                            GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1 as u64,
                            GFX942_R57_N3_QUALIFICATION_BUFFER_ALIGNMENT_V1,
                        )
                        .map_err(backend_error)?,
                );
            }
            let allocations: [RuntimeAllocationIdV1; 4] = roster
                .try_into()
                .map_err(|_| "qualification allocation roster cardinality".to_owned())?;
            context
                .write_allocation(allocations[0], 0, &a)
                .map_err(backend_error)?;
            context
                .write_allocation(allocations[1], 0, &b)
                .map_err(backend_error)?;
            Ok(Self {
                context,
                authority,
                stream,
                module,
                kernel,
                allocations,
                initial: [a, b, c_initial, d_initial],
                expected: [expected_c, expected_d],
            })
        }

        fn qualify(mut self) -> Result<[String; 4], String> {
            let first_arguments = Gfx942R57N3QualificationArgumentsV1::new(
                self.allocations[0],
                self.allocations[1],
                self.allocations[2],
            )
            .map_err(|error| error.to_string())?;
            match self.context.launch(
                self.stream,
                &self.kernel,
                &first_arguments,
                GFX942_R57_N3_QUALIFICATION_GEOMETRY_V1,
                &[],
            ) {
                Err(RuntimeErrorV1::BackendRejected(error))
                    if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
                        && error.detail().contains("exact R/R/W admission") => {}
                Err(error) => {
                    return Err(format!(
                        "uninitialized C produced the wrong rejection: {error:?}"
                    ));
                }
                Ok(_) => return Err("uninitialized C launch unexpectedly succeeded".to_owned()),
            }
            if self.authority.authorization_calls_v1() != 0 {
                return Err("uninitialized C reached final launch authority".to_owned());
            }

            self.context
                .write_allocation(self.allocations[2], 0, &self.initial[2])
                .map_err(backend_error)?;
            self.context
                .write_allocation(self.allocations[3], 0, &self.initial[3])
                .map_err(backend_error)?;

            let mut first = self
                .context
                .launch(
                    self.stream,
                    &self.kernel,
                    &first_arguments,
                    GFX942_R57_N3_QUALIFICATION_GEOMETRY_V1,
                    &[],
                )
                .map_err(backend_error)?;
            if self
                .context
                .wait(&mut first, COMPLETION_TIMEOUT)
                .map_err(backend_error)?
                != RuntimePollV1::Succeeded
            {
                return Err("A+B -> C did not succeed before the deadline".to_owned());
            }
            exact_r57_performance(
                "A+B -> C",
                self.context
                    .backend()
                    .last_launch_performance_v1()
                    .ok_or_else(|| "A+B -> C has no performance observation".to_owned())?,
            )?;

            let second_arguments = Gfx942R57N3QualificationArgumentsV1::new(
                self.allocations[2],
                self.allocations[1],
                self.allocations[3],
            )
            .map_err(|error| error.to_string())?;
            let mut second = self
                .context
                .launch(
                    self.stream,
                    &self.kernel,
                    &second_arguments,
                    GFX942_R57_N3_QUALIFICATION_GEOMETRY_V1,
                    &[],
                )
                .map_err(backend_error)?;
            if self
                .context
                .wait(&mut second, COMPLETION_TIMEOUT)
                .map_err(backend_error)?
                != RuntimePollV1::Succeeded
            {
                return Err("C+B -> D did not succeed before the deadline".to_owned());
            }
            exact_r57_performance(
                "C+B -> D",
                self.context
                    .backend()
                    .last_launch_performance_v1()
                    .ok_or_else(|| "C+B -> D has no performance observation".to_owned())?,
            )?;
            if self.authority.authorization_calls_v1() != 2 {
                return Err(format!(
                    "expected two final authority calls, observed {}",
                    self.authority.authorization_calls_v1()
                ));
            }

            let mut observed: [Vec<u8>; 4] =
                core::array::from_fn(|_| vec![0; GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1]);
            for (allocation, bytes) in self.allocations.into_iter().zip(&mut observed) {
                self.context
                    .read_allocation(allocation, 0, bytes)
                    .map_err(backend_error)?;
            }
            exact_bytes("A", &observed[0], &self.initial[0])?;
            exact_bytes("B", &observed[1], &self.initial[1])?;
            exact_bytes("C", &observed[2], &self.expected[0])?;
            exact_bytes("D", &observed[3], &self.expected[1])?;
            let digests = observed.map(|bytes| hex(Sha256::digest(bytes).into()));

            self.context
                .release_submission(first)
                .map_err(backend_error)?;
            self.context
                .release_submission(second)
                .map_err(backend_error)?;
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
            backend.shutdown_native_v1().map_err(backend_error)?;
            Ok(digests)
        }
    }

    pub fn run() -> Result<(), Box<dyn Error>> {
        let mut arguments = env::args().skip(1);
        let unique_id = parse_unique_id(&arguments.next().ok_or(USAGE)?)?;
        if arguments.next().is_some() {
            return Err(USAGE.into());
        }
        let [a, b, c, d] = QualifiedRunV1::open(unique_id)?.qualify()?;
        println!(
            "PASS schema=fe2o3.runtime.gfx942-r57-n3-qualification.v1 target=gfx942:xnack- expected_prepublication_rejections=1 authority_calls=2 launches=2 data_path=PersistentDeviceReused user_data_materializations=0 persistent_control_reused=false readbacks=4 a_sha256={a} b_sha256={b} c_sha256={c} d_sha256={d} cleanup=complete"
        );
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
}

#[cfg(feature = "hardware-qualification")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    enabled::run()
}
