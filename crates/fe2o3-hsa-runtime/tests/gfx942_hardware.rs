use fe2o3_amd_target::FeatureState;
use fe2o3_artifacts::DigestAlgorithm;
use fe2o3_core::{DeviceBuffer, GpuContext};
use fe2o3_host::{
    HsaLaunchGeometryV1, ReviewedHsaExecutableLifecycleAdapterV1,
    ReviewedHsaImplicitKernargAdapterV1,
};
use fe2o3_hsa_runtime::ReviewedHsaRuntimeAdapterV1;

#[cfg(feature = "hardware-test-hooks")]
use fe2o3_device::KernelMarkerV1;
#[cfg(feature = "hardware-test-hooks")]
use fe2o3_host::{
    AuthenticatedWorkerV2ExecutableV1, CompilerGeneratedKernelContractV1,
    CompilerGeneratedKernelProfileV1, GeneratedWorkerV2VecAddExecutorV1, ObservedContext,
    WorkerV2PrerequisiteAuthenticatorV1, WorkerV2PrerequisiteDecisionV1,
    WorkerV2PrerequisiteRequestV1, WorkerV2SafetyPropertiesV1,
};

#[cfg(feature = "hardware-test-hooks")]
fn generated_hardware_vecadd() {}

#[cfg(feature = "hardware-test-hooks")]
static GENERATED_HARDWARE_VECADD_REGISTRATION: (u16, &str, &str, fn()) =
    (1, "vecadd", "vecadd", generated_hardware_vecadd);

#[cfg(feature = "hardware-test-hooks")]
struct GeneratedHardwareVecAddKernel;

#[cfg(feature = "hardware-test-hooks")]
unsafe impl KernelMarkerV1 for GeneratedHardwareVecAddKernel {
    type Function = fn();
    type Registration = (u16, &'static str, &'static str, fn());

    const LOGICAL_NAME: &'static str = "vecadd";
    const EXPORT_NAME: &'static str = "vecadd";
    const FUNCTION: Self::Function = generated_hardware_vecadd;
    const REGISTRATION: &'static Self::Registration = &GENERATED_HARDWARE_VECADD_REGISTRATION;
}

#[cfg(feature = "hardware-test-hooks")]
unsafe impl CompilerGeneratedKernelContractV1 for GeneratedHardwareVecAddKernel {
    const PROFILE: CompilerGeneratedKernelProfileV1 =
        CompilerGeneratedKernelProfileV1::TypedVecAddF32RustcLayoutV2;
    const KERNEL_BINDING_ID_V1: [u8; 32] = [0x6b; 32];

    fn artifact_container_bytes() -> &'static [u8] {
        &[]
    }
}

#[cfg(feature = "hardware-test-hooks")]
struct HardwarePrerequisiteAuthenticator;

#[cfg(feature = "hardware-test-hooks")]
unsafe impl WorkerV2PrerequisiteAuthenticatorV1<GeneratedHardwareVecAddKernel>
    for HardwarePrerequisiteAuthenticator
{
    type Error = core::convert::Infallible;

    unsafe fn authenticate(
        &mut self,
        request: &WorkerV2PrerequisiteRequestV1<'_, GeneratedHardwareVecAddKernel>,
    ) -> Result<WorkerV2PrerequisiteDecisionV1, Self::Error> {
        let artifact = request.artifact_identity();
        Ok(WorkerV2PrerequisiteDecisionV1::new(
            request.challenge_identity().clone(),
            request.finalized_digest(),
            artifact.kernel_id(),
            artifact.executable_digest(),
            request.target(),
            request.code_object_version(),
            artifact.name().as_str(),
            artifact.symbol().as_str(),
            artifact.abi().clone(),
            artifact.launch().clone(),
            GeneratedHardwareVecAddKernel::KERNEL_BINDING_ID_V1,
            DigestAlgorithm::Sha256.calculate(b"hardware-test-compiler"),
            DigestAlgorithm::Sha256.calculate(b"hardware-test-verus"),
            DigestAlgorithm::Sha256.calculate(b"hardware-test-proof-executable"),
            DigestAlgorithm::Sha256.calculate(b"hardware-test-rust-layout"),
            DigestAlgorithm::Sha256.calculate(b"hardware-test-rust-effects"),
            WorkerV2SafetyPropertiesV1::required(),
        ))
    }
}

#[test]
#[ignore = "requires a gfx942 GPU with matching HIP and HSA runtimes"]
fn observes_one_exact_gfx942_hip_hsa_device() -> Result<(), Box<dyn std::error::Error>> {
    let context = GpuContext::new(0)?;
    let adapter = ReviewedHsaRuntimeAdapterV1::new(context)?;
    assert_eq!(
        adapter.environment().physical_device().target().processor(),
        "gfx942"
    );
    assert_eq!(
        adapter.environment().physical_device().target().xnack(),
        Some(FeatureState::Disabled)
    );
    assert_eq!(adapter.environment().physical_device().hip_ordinal(), 0);
    assert_ne!(adapter.environment().agent().agent_handle(), 0);
    Ok(())
}

struct RuntimeKernarg {
    pointer: std::ptr::NonNull<u8>,
    layout: std::alloc::Layout,
}

impl RuntimeKernarg {
    fn new(size: u64, alignment: u64) -> Result<Self, Box<dyn std::error::Error>> {
        let size = usize::try_from(size)?;
        let alignment = usize::try_from(alignment)?;
        let layout = std::alloc::Layout::from_size_align(size, alignment)?;
        // SAFETY: `layout` is valid. This owner deallocates the result once.
        let pointer = std::ptr::NonNull::new(unsafe { std::alloc::alloc_zeroed(layout) })
            .ok_or("failed to allocate runtime-aligned kernarg storage")?;
        Ok(Self { pointer, layout })
    }

    fn bytes_mut(&mut self) -> &mut [u8] {
        // SAFETY: the allocation is live and exactly `layout.size()` bytes.
        unsafe { std::slice::from_raw_parts_mut(self.pointer.as_ptr(), self.layout.size()) }
    }
}

impl Drop for RuntimeKernarg {
    fn drop(&mut self) {
        // SAFETY: this owner deallocates the exact live allocation once.
        unsafe { std::alloc::dealloc(self.pointer.as_ptr(), self.layout) };
    }
}

#[test]
#[ignore = "requires FE2O3_GFX942_VECADD_HSACO and a gfx942 GPU"]
fn directly_executes_finalized_cov6_vecadd() -> Result<(), Box<dyn std::error::Error>> {
    const LENGTH: usize = 1024;
    let path = std::env::var_os("FE2O3_GFX942_VECADD_HSACO")
        .ok_or("FE2O3_GFX942_VECADD_HSACO is not set")?;
    let bytes = std::fs::read(path)?;
    let digest = DigestAlgorithm::Sha256.calculate(&bytes);

    let context = GpuContext::new(0)?;
    let stream = context.default_stream();
    let a_host = (0..LENGTH).map(|value| value as f32).collect::<Vec<_>>();
    let b_host = (0..LENGTH)
        .map(|value| (value as f32) * 2.0)
        .collect::<Vec<_>>();
    let a = DeviceBuffer::from_host(&stream, &a_host)?;
    let b = DeviceBuffer::from_host(&stream, &b_host)?;
    let c = DeviceBuffer::<f32>::zeroed(&stream, LENGTH)?;

    let mut adapter = ReviewedHsaRuntimeAdapterV1::new(context)?;
    // SAFETY: this ignored integration harness supplies one immutable finalized
    // image and retains it, the adapter, and every allocation through cleanup.
    let (executable, _) = unsafe { adapter.load_executable(&bytes, digest) }?;
    // SAFETY: the fixture contract requires the exact exported vecadd symbol.
    let (kernel, resolution) = match unsafe { adapter.resolve_kernel(&executable, "vecadd") } {
        Ok(value) => value,
        Err(error) => {
            // SAFETY: no dispatch was submitted, so exact executable cleanup is safe.
            let _ = unsafe { adapter.unload_executable(executable) };
            return Err(error.into());
        }
    };
    if resolution.kernarg_segment_size() != 304 {
        drop(kernel);
        // SAFETY: no dispatch was submitted.
        let _ = unsafe { adapter.unload_executable(executable) };
        return Err("fixture does not expose the exact 304-byte COV6 kernarg".into());
    }

    let mut kernarg = RuntimeKernarg::new(
        resolution.kernarg_segment_size(),
        resolution.kernarg_segment_alignment(),
    )?;
    let kernarg = kernarg.bytes_mut();
    // SAFETY: the buffers are live HIP allocations on the correlated device.
    let pointers = unsafe {
        [
            a.raw_device_ptr().addr() as u64,
            b.raw_device_ptr().addr() as u64,
            c.raw_device_ptr().addr() as u64,
        ]
    };
    for (offset, value) in [
        (0, pointers[0]),
        (8, LENGTH as u64),
        (16, pointers[1]),
        (24, LENGTH as u64),
        (32, pointers[2]),
        (40, LENGTH as u64),
    ] {
        kernarg[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
    let geometry = HsaLaunchGeometryV1::new([4, 1, 1], [256, 1, 1], 0);
    // SAFETY: the explicit prefix is the exact three-slice f32 ABI, and the
    // adapter accepts only the reviewed 48+256 byte COV6 layout.
    if let Err(error) = unsafe {
        adapter.initialize_implicit_kernarg(&executable, &kernel, geometry, 48, 48, 256, kernarg)
    } {
        drop(kernel);
        // SAFETY: no dispatch was submitted.
        let _ = unsafe { adapter.unload_executable(executable) };
        return Err(error.into());
    }
    // SAFETY: all three allocations, exact ABI bytes, executable, and kernel
    // remain live until the adapter's synchronous completion observation.
    unsafe { adapter.launch_and_wait(&executable, &kernel, geometry, kernarg) }?;
    drop(kernel);
    // SAFETY: synchronous completion was observed and the kernel token is gone.
    unsafe { adapter.unload_executable(executable) }?;

    let output = c.to_host_vec(&stream)?;
    for index in 0..LENGTH {
        assert_eq!(output[index], a_host[index] + b_host[index]);
    }
    Ok(())
}

#[test]
#[cfg(feature = "hardware-test-hooks")]
#[ignore = "requires FE2O3_GFX942_VECADD_HSACO and a gfx942 GPU"]
fn safely_executes_generated_worker_v2_vecadd_end_to_end() {
    const LENGTH: usize = 1024;
    let path = std::env::var_os("FE2O3_GFX942_VECADD_HSACO")
        .expect("FE2O3_GFX942_VECADD_HSACO is not set");
    let bytes = std::fs::read(path).unwrap();
    let context = GpuContext::new(0).unwrap();
    let observed = ObservedContext::observe(&context).unwrap();
    let (admission, _directory) = fe2o3_host::__hardware_test::admitted_hardware_for_lifecycle_test(
        0xa6,
        bytes,
        GeneratedHardwareVecAddKernel::LOGICAL_NAME,
        GeneratedHardwareVecAddKernel::EXPORT_NAME,
        GeneratedHardwareVecAddKernel::KERNEL_BINDING_ID_V1,
        &observed,
    );
    let authenticated =
        AuthenticatedWorkerV2ExecutableV1::<GeneratedHardwareVecAddKernel>::authenticate(
            admission,
            &mut HardwarePrerequisiteAuthenticator,
        )
        .unwrap();
    let adapter = ReviewedHsaRuntimeAdapterV1::new(context.clone()).unwrap();
    let loaded = authenticated
        .authorize_hsa_load(adapter)
        .unwrap()
        .load()
        .unwrap();
    let mut executor = GeneratedWorkerV2VecAddExecutorV1::bind(loaded, &context).unwrap();

    let stream = context.default_stream();
    let a_host = (0..LENGTH).map(|value| value as f32).collect::<Vec<_>>();
    let b_host = (0..LENGTH)
        .map(|value| (value as f32) * 2.0)
        .collect::<Vec<_>>();
    let a = DeviceBuffer::from_host(&stream, &a_host).unwrap();
    let b = DeviceBuffer::from_host(&stream, &b_host).unwrap();
    let mut c = DeviceBuffer::<f32>::zeroed(&stream, LENGTH).unwrap();
    executor
        .prepare(&a, &b, &mut c)
        .unwrap()
        .dispatch()
        .unwrap();

    let output = c.to_host_vec(&stream).unwrap();
    for index in 0..LENGTH {
        assert_eq!(output[index], a_host[index] + b_host[index]);
    }
    executor.unload().unwrap();
}

#[test]
#[cfg(feature = "hardware-test-hooks")]
#[ignore = "requires FE2O3_GFX942_VECADD_HSACO and a disposable gfx942 process"]
fn faulting_dispatch_terminates_within_completion_budget() -> Result<(), Box<dyn std::error::Error>>
{
    const CHILD_MARKER: &str = "FE2O3_HSA_FAULT_PROBE_CHILD";
    const PHASE_VARIABLE: &str = "FE2O3_HSA_TEST_POST_SUBMIT_PHASE";
    const POST_SUBMIT_RECORD: &[u8] = b"fe2o3-hsa-post-submit-wait-v1\n";
    const QUEUE_ERROR_RECORD: &[u8] = b"fe2o3-hsa-unquiesced-queue-error-v1\n";
    const DEADLINE_RECORD: &[u8] = b"fe2o3-hsa-unquiesced-deadline-v1\n";
    if std::env::var_os(CHILD_MARKER).is_some() {
        run_faulting_dispatch_child()?;
        return Err("faulting dispatch unexpectedly returned to caller".into());
    }

    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let phase_path = std::env::temp_dir().join(format!(
        "fe2o3-hsa-fault-phase-{}-{unique}",
        std::process::id()
    ));
    let mut child = std::process::Command::new(std::env::current_exe()?)
        .arg("--ignored")
        .arg("--exact")
        .arg("faulting_dispatch_terminates_within_completion_budget")
        .arg("--nocapture")
        .env(CHILD_MARKER, "1")
        .env(PHASE_VARIABLE, &phase_path)
        .spawn()?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            child.kill()?;
            let _ = child.wait();
            let phase = std::fs::read(&phase_path).ok();
            let _ = std::fs::remove_file(&phase_path);
            if phase
                .as_deref()
                .is_some_and(|phase| phase.starts_with(POST_SUBMIT_RECORD))
            {
                return Err("post-submit fault handling exceeded the 20 second bound".into());
            }
            return Err("faulting HSA dispatch exceeded the 20 second process bound".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        let phase = std::fs::read(&phase_path);
        let _ = std::fs::remove_file(&phase_path);
        let valid_phase = phase.as_deref().ok().is_some_and(|phase| {
            phase == [POST_SUBMIT_RECORD, QUEUE_ERROR_RECORD].concat()
                || phase == [POST_SUBMIT_RECORD, DEADLINE_RECORD].concat()
        });
        if !valid_phase {
            return Err(format!(
                "fault child terminated without exact post-submit wait evidence: {phase:?}"
            )
            .into());
        }
        if status.signal() != Some(6) {
            return Err(format!("fault probe did not terminate with SIGABRT: {status}").into());
        }
    }
    Ok(())
}

#[cfg(feature = "hardware-test-hooks")]
fn run_faulting_dispatch_child() -> Result<(), Box<dyn std::error::Error>> {
    const LENGTH: usize = 1024;
    let path = std::env::var_os("FE2O3_GFX942_VECADD_HSACO")
        .ok_or("FE2O3_GFX942_VECADD_HSACO is not set")?;
    let bytes = std::fs::read(path)?;
    let digest = DigestAlgorithm::Sha256.calculate(&bytes);
    let context = GpuContext::new(0)?;
    let stream = context.default_stream();
    let input = vec![1.0_f32; LENGTH];
    let a = DeviceBuffer::from_host(&stream, &input)?;
    let b = DeviceBuffer::from_host(&stream, &input)?;
    let mut adapter = ReviewedHsaRuntimeAdapterV1::new(context)?;
    // SAFETY: the child retains exact finalized bytes until process termination.
    let (executable, _) = unsafe { adapter.load_executable(&bytes, digest) }?;
    // SAFETY: the fixture contract requires the exact exported vecadd symbol.
    let (kernel, resolution) = unsafe { adapter.resolve_kernel(&executable, "vecadd") }?;
    if resolution.kernarg_segment_size() != 304 {
        return Err("fault fixture does not expose the exact 304-byte COV6 kernarg".into());
    }

    let mut kernarg = RuntimeKernarg::new(
        resolution.kernarg_segment_size(),
        resolution.kernarg_segment_alignment(),
    )?;
    let kernarg = kernarg.bytes_mut();
    // SAFETY: both input buffers remain live. The intentionally invalid output
    // address exercises the post-publication terminal policy in this child.
    let pointers = unsafe {
        [
            a.raw_device_ptr().addr() as u64,
            b.raw_device_ptr().addr() as u64,
            1_u64,
        ]
    };
    for (offset, value) in [
        (0, pointers[0]),
        (8, LENGTH as u64),
        (16, pointers[1]),
        (24, LENGTH as u64),
        (32, pointers[2]),
        (40, LENGTH as u64),
    ] {
        kernarg[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
    let geometry = HsaLaunchGeometryV1::new([4, 1, 1], [256, 1, 1], 0);
    // SAFETY: this deliberately creates a fault only in a monitored subprocess.
    unsafe {
        adapter.initialize_implicit_kernarg(
            &executable,
            &kernel,
            geometry,
            48,
            48,
            256,
            kernarg,
        )?;
        adapter.launch_and_wait(&executable, &kernel, geometry, kernarg)?;
    }
    Ok(())
}
