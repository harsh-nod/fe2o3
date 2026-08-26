use super::*;
use crate::{
    HsaAgentIdentityV1, HsaPhysicalDeviceIdentityV1, HsaRuntimeIdentityV1,
    ReviewedHsaExecutableLifecycleAdapterV1,
};
use fe2o3_artifacts::DigestAlgorithm;
use std::{
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

const RUNTIME_INSTANCE: [u8; 16] = [0x31; 16];
const DEVICE_UUID: [u8; 16] = [0x42; 16];
const AGENT_HANDLE: u64 = 0x5354;
const EXECUTABLE_OBJECT: [u8; 32] = [0x65; 32];
const KERNEL_OBJECT: [u8; 32] = [0x76; 32];
const DISPATCH_IDENTITY: [u8; 16] = [0x87; 16];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Fault {
    None,
    EnvironmentAdapter,
    EnvironmentTarget,
    EnvironmentOrdinal,
    LoadAdapter,
    LoadDigest,
    LoadLength,
    LoadRuntime,
    LoadAgent,
    ResolveAdapter,
    ResolutionExecutable,
    ResolutionSymbol,
    ResolutionSize,
    ResolutionAlignment,
    ResourceAdapter,
    ResourceExecutable,
    ResourceKernel,
    ResourceGroup,
    ResourcePrivate,
    ImplicitAdapter,
    ImplicitExecutable,
    ImplicitKernel,
    ImplicitGeometry,
    ImplicitSpan,
    ImplicitIncomplete,
    ExplicitMutation,
    DispatchAdapter,
    DispatchTimeout,
    DispatchExecutable,
    DispatchKernel,
    DispatchGeometry,
    DispatchIncomplete,
}

#[derive(Debug)]
struct FakeError(&'static str);

impl fmt::Display for FakeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Error for FakeError {}

struct FakeRetained {
    bytes: Vec<u8>,
    output: ContentIdentityV1,
    explicit: [u8; EXPLICIT_KERNARG_BYTES],
    drops: Arc<AtomicUsize>,
}

impl FakeRetained {
    fn new(drops: Arc<AtomicUsize>) -> Self {
        let bytes = b"exact-finalized-MoeTop2-v1".to_vec();
        let output = ContentIdentityV1::calculate(&bytes);
        Self {
            bytes,
            output,
            explicit: std::array::from_fn(|index| index as u8),
            drops,
        }
    }
}

impl Drop for FakeRetained {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

impl RetainedMoeTop2V1 for FakeRetained {
    fn target_v1(&self) -> &str {
        TARGET
    }

    fn ordinal_v1(&self) -> i32 {
        0
    }

    fn explicit_kernarg_v1(&self) -> &[u8; EXPLICIT_KERNARG_BYTES] {
        &self.explicit
    }

    fn with_finalized_bytes_v1<T, F: FnOnce(&[u8], ContentIdentityV1) -> T>(&self, load: F) -> T {
        load(&self.bytes, self.output)
    }
}

#[derive(Debug)]
struct FakeExecutable {
    object: HsaExecutableObjectIdentityV1,
}

#[derive(Debug)]
struct FakeKernel {
    object: HsaKernelObjectIdentityV1,
    events: Arc<Mutex<Vec<&'static str>>>,
}

impl Drop for FakeKernel {
    fn drop(&mut self) {
        self.events.lock().unwrap().push("kernel_drop");
    }
}

#[derive(Clone)]
struct FakeAdapter {
    fault: Fault,
    events: Arc<Mutex<Vec<&'static str>>>,
    unloads: Arc<AtomicUsize>,
    aligned: Arc<AtomicBool>,
    explicit_preserved: Arc<AtomicBool>,
    hidden_initialized: Arc<AtomicBool>,
}

struct FakeState {
    adapter: FakeAdapter,
    events: Arc<Mutex<Vec<&'static str>>>,
    unloads: Arc<AtomicUsize>,
    aligned: Arc<AtomicBool>,
    explicit_preserved: Arc<AtomicBool>,
    hidden_initialized: Arc<AtomicBool>,
}

fn fake(fault: Fault) -> FakeState {
    let events = Arc::new(Mutex::new(Vec::new()));
    let unloads = Arc::new(AtomicUsize::new(0));
    let aligned = Arc::new(AtomicBool::new(false));
    let explicit_preserved = Arc::new(AtomicBool::new(false));
    let hidden_initialized = Arc::new(AtomicBool::new(false));
    FakeState {
        adapter: FakeAdapter {
            fault,
            events: events.clone(),
            unloads: unloads.clone(),
            aligned: aligned.clone(),
            explicit_preserved: explicit_preserved.clone(),
            hidden_initialized: hidden_initialized.clone(),
        },
        events,
        unloads,
        aligned,
        explicit_preserved,
        hidden_initialized,
    }
}

fn executable_object() -> HsaExecutableObjectIdentityV1 {
    HsaExecutableObjectIdentityV1::new(EXECUTABLE_OBJECT).unwrap()
}

fn kernel_object() -> HsaKernelObjectIdentityV1 {
    HsaKernelObjectIdentityV1::new(KERNEL_OBJECT).unwrap()
}

fn environment(target: &str, ordinal: i32) -> HsaEnvironmentObservationV1 {
    let target = AmdTargetId::parse(target).unwrap();
    let runtime = HsaRuntimeIdentityV1::new(
        "fake-hsa",
        "1.0",
        DigestAlgorithm::Sha256.calculate(b"fake-runtime"),
        RUNTIME_INSTANCE,
    )
    .unwrap();
    let physical = HsaPhysicalDeviceIdentityV1::new(DEVICE_UUID, 7, ordinal, target).unwrap();
    let agent =
        HsaAgentIdentityV1::new(RUNTIME_INSTANCE, AGENT_HANDLE, DEVICE_UUID, target).unwrap();
    HsaEnvironmentObservationV1::new(runtime, physical, agent).unwrap()
}

unsafe impl ReviewedHsaExecutableLifecycleAdapterV1 for FakeAdapter {
    type Executable = FakeExecutable;
    type Kernel = FakeKernel;
    type Error = FakeError;

    unsafe fn observe_environment(&mut self) -> Result<HsaEnvironmentObservationV1, Self::Error> {
        self.events.lock().unwrap().push("environment");
        match self.fault {
            Fault::EnvironmentAdapter => Err(FakeError("environment")),
            Fault::EnvironmentTarget => Ok(environment("gfx1100", 0)),
            Fault::EnvironmentOrdinal => Ok(environment(TARGET, 1)),
            _ => Ok(environment(TARGET, 0)),
        }
    }

    unsafe fn load_executable(
        &mut self,
        bytes: &[u8],
        finalized_digest: PayloadDigest,
    ) -> Result<(Self::Executable, HsaCodeObjectLoadObservationV1), Self::Error> {
        self.events.lock().unwrap().push("load");
        if self.fault == Fault::LoadAdapter {
            return Err(FakeError("load"));
        }
        let digest = if self.fault == Fault::LoadDigest {
            DigestAlgorithm::Sha256.calculate(b"substituted")
        } else {
            finalized_digest
        };
        let byte_len = bytes.len() as u64 + u64::from(self.fault == Fault::LoadLength);
        let runtime = if self.fault == Fault::LoadRuntime {
            [0x99; 16]
        } else {
            RUNTIME_INSTANCE
        };
        let agent = if self.fault == Fault::LoadAgent {
            AGENT_HANDLE + 1
        } else {
            AGENT_HANDLE
        };
        let object = executable_object();
        Ok((
            FakeExecutable { object },
            HsaCodeObjectLoadObservationV1::new(digest, byte_len, runtime, agent, object),
        ))
    }

    unsafe fn resolve_kernel(
        &mut self,
        executable: &Self::Executable,
        _export_symbol: &str,
    ) -> Result<(Self::Kernel, HsaKernelResolutionObservationV1), Self::Error> {
        self.events.lock().unwrap().push("resolve");
        if self.fault == Fault::ResolveAdapter {
            return Err(FakeError("resolve"));
        }
        let executable_observation = if self.fault == Fault::ResolutionExecutable {
            HsaExecutableObjectIdentityV1::new([0xa1; 32]).unwrap()
        } else {
            executable.object
        };
        let symbol = if self.fault == Fault::ResolutionSymbol {
            "substituted"
        } else {
            EXPORT_SYMBOL
        };
        let size = if self.fault == Fault::ResolutionSize {
            COMPLETE_KERNARG_BYTES as u64 - 8
        } else {
            COMPLETE_KERNARG_BYTES as u64
        };
        let alignment = if self.fault == Fault::ResolutionAlignment {
            8
        } else {
            HSA_KERNARG_ALIGNMENT
        };
        let object = kernel_object();
        Ok((
            FakeKernel {
                object,
                events: self.events.clone(),
            },
            HsaKernelResolutionObservationV1::new(
                executable_observation,
                object,
                symbol,
                size,
                alignment,
                0,
                0,
            )
            .unwrap(),
        ))
    }

    unsafe fn launch_and_wait(
        &mut self,
        executable: &Self::Executable,
        kernel: &Self::Kernel,
        geometry: HsaLaunchGeometryV1,
        kernarg: &mut [u8],
    ) -> Result<HsaDispatchObservationV1, Self::Error> {
        self.events.lock().unwrap().push("dispatch");
        if matches!(self.fault, Fault::DispatchAdapter | Fault::DispatchTimeout) {
            return Err(FakeError(if self.fault == Fault::DispatchTimeout {
                "bounded timeout before publication"
            } else {
                "dispatch"
            }));
        }
        self.aligned.store(
            (kernarg.as_ptr() as usize).is_multiple_of(HSA_KERNARG_ALIGNMENT as usize),
            Ordering::SeqCst,
        );
        self.explicit_preserved.store(
            kernarg[..EXPLICIT_KERNARG_BYTES]
                .iter()
                .copied()
                .eq((0..EXPLICIT_KERNARG_BYTES).map(|index| index as u8)),
            Ordering::SeqCst,
        );
        self.hidden_initialized.store(
            kernarg[EXPLICIT_KERNARG_BYTES..]
                .iter()
                .all(|byte| *byte == 0xa5),
            Ordering::SeqCst,
        );
        let executable_observation = if self.fault == Fault::DispatchExecutable {
            HsaExecutableObjectIdentityV1::new([0xa2; 32]).unwrap()
        } else {
            executable.object
        };
        let kernel_observation = if self.fault == Fault::DispatchKernel {
            HsaKernelObjectIdentityV1::new([0xa3; 32]).unwrap()
        } else {
            kernel.object
        };
        let geometry_observation = if self.fault == Fault::DispatchGeometry {
            HsaLaunchGeometryV1::new([2, 1, 1], WORKGROUP, 0)
        } else {
            geometry
        };
        HsaDispatchObservationV1::new(
            DISPATCH_IDENTITY,
            executable_observation,
            kernel_observation,
            geometry_observation,
            self.fault != Fault::DispatchIncomplete,
        )
        .map_err(|_| FakeError("dispatch observation"))
    }

    unsafe fn unload_executable(
        &mut self,
        executable: Self::Executable,
    ) -> Result<HsaUnloadObservationV1, Self::Error> {
        self.events.lock().unwrap().push("unload");
        self.unloads.fetch_add(1, Ordering::SeqCst);
        Ok(HsaUnloadObservationV1::new(
            executable.object,
            RUNTIME_INSTANCE,
            AGENT_HANDLE,
            true,
        ))
    }
}

unsafe impl ReviewedHsaImplicitKernargAdapterV1 for FakeAdapter {
    unsafe fn initialize_implicit_kernarg(
        &mut self,
        executable: &Self::Executable,
        kernel: &Self::Kernel,
        geometry: HsaLaunchGeometryV1,
        explicit_byte_len: usize,
        implicit_byte_offset: usize,
        implicit_byte_len: usize,
        kernarg: &mut [u8],
    ) -> Result<HsaImplicitKernargInitializationObservationV1, Self::Error> {
        self.events.lock().unwrap().push("implicit");
        if self.fault == Fault::ImplicitAdapter {
            return Err(FakeError("implicit"));
        }
        kernarg[implicit_byte_offset..implicit_byte_offset + implicit_byte_len].fill(0xa5);
        if self.fault == Fault::ExplicitMutation {
            kernarg[0] ^= 1;
        }
        let executable_observation = if self.fault == Fault::ImplicitExecutable {
            HsaExecutableObjectIdentityV1::new([0xb1; 32]).unwrap()
        } else {
            executable.object
        };
        let kernel_observation = if self.fault == Fault::ImplicitKernel {
            HsaKernelObjectIdentityV1::new([0xb2; 32]).unwrap()
        } else {
            kernel.object
        };
        let geometry_observation = if self.fault == Fault::ImplicitGeometry {
            HsaLaunchGeometryV1::new([2, 1, 1], WORKGROUP, 0)
        } else {
            geometry
        };
        Ok(HsaImplicitKernargInitializationObservationV1::new(
            executable_observation,
            kernel_observation,
            geometry_observation,
            (explicit_byte_len - usize::from(self.fault == Fault::ImplicitSpan)) as u64,
            implicit_byte_offset as u64,
            implicit_byte_len as u64,
            self.fault != Fault::ImplicitIncomplete,
        ))
    }
}

unsafe impl ReviewedMoeTop2V1RuntimeAdapterV1 for FakeAdapter {
    unsafe fn context_identity_v1(&mut self) -> ContextIdentity {
        unreachable!("private helper starts after context validation")
    }

    unsafe fn observe_moe_top2_v1_kernel_resources(
        &mut self,
        executable: &Self::Executable,
        kernel: &Self::Kernel,
    ) -> Result<MoeTop2V1KernelResourceObservationV1, Self::Error> {
        self.events.lock().unwrap().push("resources");
        if self.fault == Fault::ResourceAdapter {
            return Err(FakeError("resources"));
        }
        let executable_observation = if self.fault == Fault::ResourceExecutable {
            HsaExecutableObjectIdentityV1::new([0xc1; 32]).unwrap()
        } else {
            executable.object
        };
        let kernel_observation = if self.fault == Fault::ResourceKernel {
            HsaKernelObjectIdentityV1::new([0xc2; 32]).unwrap()
        } else {
            kernel.object
        };
        Ok(MoeTop2V1KernelResourceObservationV1::new(
            executable_observation,
            kernel_observation,
            u32::from(self.fault == Fault::ResourceGroup),
            u32::from(self.fault == Fault::ResourcePrivate),
        ))
    }
}

fn load(
    state: &FakeState,
    drops: Arc<AtomicUsize>,
) -> Result<LoadedState<FakeRetained, FakeAdapter>, MoeTop2V1LoadErrorV1<FakeError>> {
    load_after_context_match(FakeRetained::new(drops), state.adapter.clone())
}

fn events(state: &FakeState) -> Vec<&'static str> {
    state.events.lock().unwrap().clone()
}

#[test]
fn successful_exact_lifecycle_preserves_abi_and_releases_leases_at_quiescence() {
    let state = fake(Fault::None);
    let drops = Arc::new(AtomicUsize::new(0));
    let loaded = load(&state, drops.clone()).unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(
        events(&state),
        ["environment", "load", "resolve", "resources"]
    );

    let quiescent = loaded.dispatch_and_wait().unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert!(state.aligned.load(Ordering::SeqCst));
    assert!(state.explicit_preserved.load(Ordering::SeqCst));
    assert!(state.hidden_initialized.load(Ordering::SeqCst));
    let mut completed = quiescent.release_retained();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    let executable = completed.executable.take().unwrap();
    let mut adapter = completed.adapter.take().unwrap();
    let unload = terminal_unload(
        &mut adapter,
        executable,
        &completed.environment,
        &completed.load,
    );
    assert!(unload.released());
    drop(completed);
    assert_eq!(state.unloads.load(Ordering::SeqCst), 1);
    assert_eq!(
        events(&state),
        [
            "environment",
            "load",
            "resolve",
            "resources",
            "implicit",
            "dispatch",
            "kernel_drop",
            "unload"
        ]
    );
}

#[test]
fn every_load_and_resource_substitution_fails_closed_with_exact_cleanup() {
    for fault in [
        Fault::EnvironmentAdapter,
        Fault::EnvironmentTarget,
        Fault::EnvironmentOrdinal,
        Fault::LoadAdapter,
        Fault::LoadDigest,
        Fault::LoadLength,
        Fault::LoadRuntime,
        Fault::LoadAgent,
        Fault::ResolveAdapter,
        Fault::ResolutionExecutable,
        Fault::ResolutionSymbol,
        Fault::ResolutionSize,
        Fault::ResolutionAlignment,
        Fault::ResourceAdapter,
        Fault::ResourceExecutable,
        Fault::ResourceKernel,
        Fault::ResourceGroup,
        Fault::ResourcePrivate,
    ] {
        let state = fake(fault);
        let drops = Arc::new(AtomicUsize::new(0));
        assert!(load(&state, drops.clone()).is_err(), "accepted {fault:?}");
        let expected_unloads = usize::from(!matches!(
            fault,
            Fault::EnvironmentAdapter
                | Fault::EnvironmentTarget
                | Fault::EnvironmentOrdinal
                | Fault::LoadAdapter
        ));
        assert_eq!(
            state.unloads.load(Ordering::SeqCst),
            expected_unloads,
            "{fault:?}"
        );
        assert_eq!(drops.load(Ordering::SeqCst), 1, "{fault:?}");
    }
}

#[test]
fn implicit_dispatch_timeout_and_observation_failures_cleanup_once() {
    for fault in [
        Fault::ImplicitAdapter,
        Fault::ImplicitExecutable,
        Fault::ImplicitKernel,
        Fault::ImplicitGeometry,
        Fault::ImplicitSpan,
        Fault::ImplicitIncomplete,
        Fault::ExplicitMutation,
        Fault::DispatchAdapter,
        Fault::DispatchTimeout,
        Fault::DispatchExecutable,
        Fault::DispatchKernel,
        Fault::DispatchGeometry,
        Fault::DispatchIncomplete,
    ] {
        let state = fake(fault);
        let drops = Arc::new(AtomicUsize::new(0));
        let loaded = load(&state, drops.clone()).unwrap();
        assert!(loaded.dispatch_and_wait().is_err(), "accepted {fault:?}");
        assert_eq!(state.unloads.load(Ordering::SeqCst), 1, "{fault:?}");
        assert_eq!(drops.load(Ordering::SeqCst), 1, "{fault:?}");
        let observed = events(&state);
        assert!(
            observed.iter().position(|event| *event == "kernel_drop")
                < observed.iter().position(|event| *event == "unload"),
            "{fault:?}: {observed:?}"
        );
    }
}

#[test]
fn dropping_loaded_or_completed_state_unloads_exactly_once() {
    let loaded_state = fake(Fault::None);
    let loaded_drops = Arc::new(AtomicUsize::new(0));
    drop(load(&loaded_state, loaded_drops.clone()).unwrap());
    assert_eq!(loaded_state.unloads.load(Ordering::SeqCst), 1);
    assert_eq!(loaded_drops.load(Ordering::SeqCst), 1);

    let completed_state = fake(Fault::None);
    let completed_drops = Arc::new(AtomicUsize::new(0));
    let completed = load(&completed_state, completed_drops.clone())
        .unwrap()
        .dispatch_and_wait()
        .unwrap()
        .release_retained();
    drop(completed);
    assert_eq!(completed_state.unloads.load(Ordering::SeqCst), 1);
    assert_eq!(completed_drops.load(Ordering::SeqCst), 1);
}

#[test]
fn load_error_names_the_hsa_context() {
    let error: MoeTop2V1LoadErrorV1<FakeError> = MoeTop2V1LoadErrorV1::ContextIdentity;
    assert_eq!(error.to_string(), "exact HSA context identity mismatch");
}
